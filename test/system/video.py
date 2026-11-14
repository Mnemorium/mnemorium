"""Black-box proof of the `video` capability.

A video is the typed row an uploaded or found video file backs. This module proves the measurements the row reports, the
absence of a name it never carries, how a caller reads it — the typed representation, its links, its file alias and the
`404` an unknown video answers — and the summary shape a film or an episode embeds.

The declarative contract — the media types, the status of each operation, the representation's fields, its `_links` and
the discrete `PROGRESSIVE`/`INTERLACED`/`MBAFF`/`PAFF` scan-type set — is validated by Schemathesis from the served
document. These cases prove only what a schema cannot: that the reported values are the ones the content carries, that a
video carries no name, that a file backs at most one video, that another media family creates none, and that the summary
a catalogue resource embeds is the declared subset and not the whole representation. Every route comes from the file's
`_links.video`, from the typed representation's own links or from the API index, so the suite keeps its single
`/api/v1` base literal in `harness.Sut.index`.

A codec is compared case-insensitively: the specification fixes that the row reports one, not how it cases it.
"""

from datetime import UTC, datetime
from itertools import count

import allure
from conftest import FIXTURE_DIRECTORY, linked_id, upload
from harness import Sut, link_of

VIDEO_CONTENT_TYPE = "video/mp4"
AUDIO_CONTENT_TYPE = "audio/flac"

# The fixture the module uploads: a real H.264 stream the container's probe reads.
VIDEO_FIXTURE = "clip.mp4"

# What the fixture's stream declares: a 16x16 progressive H.264 clip of two frames at 200 ms.
CODEC = "h264"
COLOR_ID = "YCbCr"
DURATION_MS = 200.0
FRAME_COUNT = 2
WIDTH = 16
HEIGHT = 16
SCAN_TYPE = "PROGRESSIVE"

VIDEO_FIELDS = {
    "codec",
    "color_id",
    "duration_ms",
    "file_id",
    "frame_count",
    "height",
    "id",
    "scan_type",
    "width",
    "_links",
}

# The summary a film or an episode embeds: the identifier, the backing file, the codec, the duration, the dimensions and
# the scan type, and none of the colour identifier, the frame count or any link.
SUMMARY_FIELDS = {"id", "file_id", "codec", "duration_ms", "width", "height", "scan_type"}

# Each generated video carries a per-call trailing `free` box, so two uploads never share a file through deduplication.
_VIDEO_NONCE = count(datetime.now(UTC).toordinal())


def video_bytes(fixture: str = VIDEO_FIXTURE) -> bytes:
    """Return a fixture's bytes with a unique trailing `free` box the probe ignores."""
    padding = (_VIDEO_NONCE.__next__() & 0xFFFF).to_bytes(4, "big")
    return (FIXTURE_DIRECTORY / fixture).read_bytes() + (8 + len(padding)).to_bytes(4, "big") + b"free" + padding


def upload_video(server: Sut, token: str, name: str, fixture: str = VIDEO_FIXTURE) -> dict:
    """Upload one video and return the file representation."""
    return upload(server, token, name, video_bytes(fixture), VIDEO_CONTENT_TYPE)


def typed_video(server: Sut, token: str, file: dict) -> dict:
    """Follow a file's `video` link and return the typed representation."""
    response = server.follow(file, "video", token=token)
    assert response.status_code == 200, f"video read failed: {response.status_code} {response.text}"
    return response.json()


def unknown_video(server: Sut, token: str, video: dict):
    """Request the video one identifier past `video`, which the instance has never created."""
    href = link_of(video, "self")
    base, _, _ = href.rpartition("/")
    return server.request("GET", f"{base}/{linked_id(href) + 1}", token=token)


def create_film(server: Sut, token: str, video_id: int) -> dict:
    """Create a film on a video and return the read representation."""
    created = server.request(
        "POST",
        server.href("movie"),
        token=token,
        json={"name": "The Fixture", "video_id": video_id},
    )
    assert created.status_code == 201, f"film creation failed: {created.status_code} {created.text}"
    response = server.request("GET", link_of(created.json(), "self"), token=token)
    assert response.status_code == 200, f"film read failed: {response.status_code} {response.text}"
    return response.json()


def create_episode(server: Sut, token: str, video_id: int) -> dict:
    """Create a show, its first season and its first episode, then return the episode representation."""
    created = server.request("POST", server.href("series"), token=token, json={"name": "The Fixture Show"})
    assert created.status_code == 201, f"show creation failed: {created.status_code} {created.text}"
    show = link_of(created.json(), "self")

    season = server.request("PUT", f"{show}/season/1", token=token, json={"name": "Season One"})
    assert season.status_code == 201, f"season creation failed: {season.status_code} {season.text}"

    episode = server.request(
        "PUT",
        f"{show}/season/1/episode/1",
        token=token,
        json={"name": "Pilot", "plot": "An opening.", "video_id": video_id},
    )
    assert episode.status_code == 201, f"episode creation failed: {episode.status_code} {episode.text}"

    response = server.request("GET", link_of(episode.json(), "self"), token=token)
    assert response.status_code == 200, f"episode read failed: {response.status_code} {response.text}"
    return response.json()


def assert_summary(representation: dict, file: dict, video: dict) -> None:
    """Assert the embedded video summary carries the declared subset and nothing else."""
    summary = representation["video"]

    assert set(summary) == SUMMARY_FIELDS
    assert summary["id"] == video["id"]
    assert summary["file_id"] == file["id"]
    assert summary["codec"].lower() == CODEC
    assert abs(summary["duration_ms"] - DURATION_MS) <= 1.0
    assert summary["width"] == WIDTH
    assert summary["height"] == HEIGHT
    assert summary["scan_type"] == SCAN_TYPE


@allure.epic("Video")
@allure.feature("REQ-VIDEO-001")
@allure.story("SCN-VIDEO-001-A")
def test_a_video_file_creates_one_video(server: Sut, root_token: str) -> None:
    """A video upload leaves exactly one video backing the uploaded file."""
    file = upload_video(server, root_token, "clip.mp4")

    video = typed_video(server, root_token, file)

    assert video["file_id"] == file["id"]
    assert link_of(file, "video").endswith(f"/{video['id']}")
    assert unknown_video(server, root_token, video).status_code == 404


@allure.epic("Video")
@allure.feature("REQ-VIDEO-001")
@allure.story("SCN-VIDEO-001-B")
def test_a_file_backs_one_video(server: Sut, root_token: str) -> None:
    """Registering the same stored file again creates no second video for it."""
    data = video_bytes()
    first = upload(server, root_token, "clip.mp4", data, VIDEO_CONTENT_TYPE)
    before = typed_video(server, root_token, first)

    again = upload(server, root_token, "clip.mp4", data, VIDEO_CONTENT_TYPE)
    after = typed_video(server, root_token, again)

    assert again["id"] == first["id"], "the same content must reuse its stored file"
    assert after["id"] == before["id"], "the second registration created a second video"
    assert unknown_video(server, root_token, after).status_code == 404


@allure.epic("Video")
@allure.feature("REQ-VIDEO-001")
@allure.story("SCN-VIDEO-001-C")
def test_image_and_audio_content_create_no_video(server: Sut, root_token: str) -> None:
    """Image and audio uploads leave the file without a video."""
    image = upload(
        server,
        root_token,
        "cover.png",
        (FIXTURE_DIRECTORY / "cover.png").read_bytes(),
        "image/png",
    )
    audio = upload(
        server,
        root_token,
        "track.flac",
        (FIXTURE_DIRECTORY / "track.flac").read_bytes(),
        AUDIO_CONTENT_TYPE,
    )

    assert "video" not in image["_links"]
    assert "video" not in audio["_links"]


@allure.epic("Video")
@allure.feature("REQ-VIDEO-002")
@allure.story("SCN-VIDEO-002-A")
def test_the_measurements_are_reported(server: Sut, root_token: str) -> None:
    """The representation carries the codec, colour identifier, duration, frame count and dimensions."""
    video = typed_video(server, root_token, upload_video(server, root_token, "clip.mp4"))

    assert video["codec"].lower() == CODEC
    assert video["color_id"] == COLOR_ID
    assert abs(video["duration_ms"] - DURATION_MS) <= 1.0
    assert video["frame_count"] == FRAME_COUNT
    assert (video["width"], video["height"]) == (WIDTH, HEIGHT)


@allure.epic("Video")
@allure.feature("REQ-VIDEO-002")
@allure.story("SCN-VIDEO-002-B")
def test_a_progressive_stream_reports_a_progressive_scan(server: Sut, root_token: str) -> None:
    """A stream whose content reports a progressive scan carries `PROGRESSIVE`."""
    video = typed_video(server, root_token, upload_video(server, root_token, "clip.mp4"))

    assert video["scan_type"] == SCAN_TYPE
    assert video["scan_type"] in {"PROGRESSIVE", "INTERLACED", "MBAFF", "PAFF"}


@allure.epic("Video")
@allure.feature("REQ-VIDEO-002")
@allure.story("SCN-VIDEO-002-C")
def test_a_video_carries_no_name(server: Sut, root_token: str) -> None:
    """The representation carries no name field, whatever the uploaded file was called."""
    video = typed_video(server, root_token, upload_video(server, root_token, "named-clip.mp4"))

    assert "name" not in video


@allure.epic("Video")
@allure.feature("REQ-VIDEO-003")
@allure.story("SCN-VIDEO-003-A")
def test_a_video_carries_its_representation(server: Sut, root_token: str) -> None:
    """The typed representation carries the video's fields and links its own resource, file and content."""
    file = upload_video(server, root_token, "clip.mp4")
    typed_href = link_of(file, "video")

    video = typed_video(server, root_token, file)

    assert set(video) == VIDEO_FIELDS
    assert link_of(video, "self") == typed_href
    assert link_of(video, "file") == link_of(file, "self")
    assert link_of(video, "content") == f"{typed_href}/content"


@allure.epic("Video")
@allure.feature("REQ-VIDEO-003")
@allure.story("SCN-VIDEO-003-B")
def test_an_unknown_video_is_not_found(server: Sut, root_token: str) -> None:
    """A video the instance never created answers `404`, like one the caller may not read."""
    video = typed_video(server, root_token, upload_video(server, root_token, "clip.mp4"))

    response = unknown_video(server, root_token, video)

    assert response.status_code == 404, f"unknown video answered {response.status_code}: {response.text}"


@allure.epic("Video")
@allure.feature("REQ-VIDEO-004")
@allure.story("SCN-VIDEO-004-A")
def test_the_file_alias_redirects_to_the_file(server: Sut, root_token: str) -> None:
    """The typed file alias answers `303` with the backing file resource in `Location`."""
    file = upload_video(server, root_token, "alias.mp4")
    typed_href = link_of(file, "video")

    response = server.request("GET", f"{typed_href}/file", token=root_token, allow_redirects=False)

    assert response.status_code == 303, f"alias answered {response.status_code}: {response.text}"
    assert response.headers["Location"] == link_of(file, "self")


@allure.epic("Video")
@allure.feature("REQ-VIDEO-005")
@allure.story("SCN-VIDEO-005-A")
def test_a_film_and_an_episode_embed_the_summary(server: Sut, root_token: str) -> None:
    """A film and an episode each carry the video summary and nothing the summary omits."""
    film_file = upload_video(server, root_token, "film.mp4")
    film_video = typed_video(server, root_token, film_file)
    episode_file = upload_video(server, root_token, "episode.mp4", "episode.mp4")
    episode_video = typed_video(server, root_token, episode_file)

    assert_summary(create_film(server, root_token, film_video["id"]), film_file, film_video)
    assert_summary(create_episode(server, root_token, episode_video["id"]), episode_file, episode_video)
