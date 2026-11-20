"""Black-box proof of the `video` capability."""

import json
from datetime import UTC, datetime
from itertools import count

import allure
from conftest import SEED_MEDIA_DIRECTORY, upload
from harness import Sut
from mnemorium_client.api.asset import get_video

VIDEO_CONTENT_TYPE = "video/mp4"
AUDIO_CONTENT_TYPE = "audio/flac"

# The fixture the module uploads: a real H.264 stream the container's probe reads.
VIDEO_FIXTURE = "episode.mp4"

# What the fixture's stream declares: a 16x16 progressive H.264 clip of two frames at 200 ms.
CODEC = "h264"
COLOR_ID = "YCbCr"
DURATION_MS = 200.0
FRAME_COUNT = 2
WIDTH = 16
HEIGHT = 16
SCAN_TYPE = "PROGRESSIVE"

# Each generated video carries a per-call trailing `free` box, so two uploads never share a file through deduplication.
_VIDEO_NONCE = count(datetime.now(UTC).toordinal())


def video_bytes() -> bytes:
    """Return the fixture's bytes with a unique trailing `free` box the probe ignores."""
    padding = (_VIDEO_NONCE.__next__() & 0xFFFF).to_bytes(4, "big")
    return (
        (SEED_MEDIA_DIRECTORY / VIDEO_FIXTURE).read_bytes() + (8 + len(padding)).to_bytes(4, "big") + b"free" + padding
    )


@allure.epic("Video")
@allure.feature("REQ-VIDEO-001")
@allure.story("SCN-VIDEO-001-A")
def test_a_video_file_creates_one_video(server: Sut, root_token: str) -> None:
    """A video upload leaves one video backed by the uploaded file."""
    file = upload(server, root_token, "clip.mp4", video_bytes(), VIDEO_CONTENT_TYPE)

    video_id = int(file["_links"]["video"]["href"].rsplit("/", 1)[1])
    response = get_video.sync_detailed(video_id, client=server.client(root_token))
    assert response.status_code == 200, f"video read failed: {response.status_code} {response.content!r}"
    video = json.loads(response.content)

    assert video["file_id"] == file["id"]
    assert file["_links"]["video"]["href"].endswith(f"/{video['id']}")


@allure.epic("Video")
@allure.feature("REQ-VIDEO-001")
@allure.story("SCN-VIDEO-001-B")
def test_a_file_backs_one_video(server: Sut, root_token: str) -> None:
    """Uploading the same content twice reuses the stored file and its one video."""
    data = video_bytes()
    first = upload(server, root_token, "clip.mp4", data, VIDEO_CONTENT_TYPE)
    first_id = int(first["_links"]["video"]["href"].rsplit("/", 1)[1])
    first_response = get_video.sync_detailed(first_id, client=server.client(root_token))
    assert first_response.status_code == 200, (
        f"video read failed: {first_response.status_code} {first_response.content!r}"
    )
    first_video = json.loads(first_response.content)

    second = upload(server, root_token, "clip.mp4", data, VIDEO_CONTENT_TYPE)
    second_id = int(second["_links"]["video"]["href"].rsplit("/", 1)[1])
    second_response = get_video.sync_detailed(second_id, client=server.client(root_token))
    assert second_response.status_code == 200, (
        f"video read failed: {second_response.status_code} {second_response.content!r}"
    )
    second_video = json.loads(second_response.content)

    assert second["id"] == first["id"], "the same content must reuse its stored file"
    assert second_video["id"] == first_video["id"], "the second registration created a second video"


@allure.epic("Video")
@allure.feature("REQ-VIDEO-001")
@allure.story("SCN-VIDEO-001-C")
def test_image_and_audio_content_create_no_video(server: Sut, root_token: str) -> None:
    """Image and audio uploads leave the file without a video."""
    image = upload(
        server,
        root_token,
        "cover.png",
        (SEED_MEDIA_DIRECTORY / "library-cover.png").read_bytes(),
        "image/png",
    )
    audio = upload(
        server,
        root_token,
        "track.flac",
        (SEED_MEDIA_DIRECTORY / "track.flac").read_bytes(),
        AUDIO_CONTENT_TYPE,
    )

    assert "video" not in image["_links"]
    assert "video" not in audio["_links"]


@allure.epic("Video")
@allure.feature("REQ-VIDEO-002")
@allure.story("SCN-VIDEO-002-A")
def test_the_measurements_are_reported(server: Sut, root_token: str) -> None:
    """The representation carries the codec, colour identifier, duration, frame count and dimensions."""
    file = upload(server, root_token, "clip.mp4", video_bytes(), VIDEO_CONTENT_TYPE)

    video_id = int(file["_links"]["video"]["href"].rsplit("/", 1)[1])
    response = get_video.sync_detailed(video_id, client=server.client(root_token))
    assert response.status_code == 200, f"video read failed: {response.status_code} {response.content!r}"
    video = json.loads(response.content)

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
    file = upload(server, root_token, "clip.mp4", video_bytes(), VIDEO_CONTENT_TYPE)

    video_id = int(file["_links"]["video"]["href"].rsplit("/", 1)[1])
    response = get_video.sync_detailed(video_id, client=server.client(root_token))
    assert response.status_code == 200, f"video read failed: {response.status_code} {response.content!r}"
    video = json.loads(response.content)

    assert video["scan_type"] == SCAN_TYPE


@allure.epic("Video")
@allure.feature("REQ-VIDEO-002")
@allure.story("SCN-VIDEO-002-C")
def test_a_video_carries_no_name(server: Sut, root_token: str) -> None:
    """The representation carries no name field, whatever the uploaded file was called."""
    file = upload(server, root_token, "named-clip.mp4", video_bytes(), VIDEO_CONTENT_TYPE)

    video_id = int(file["_links"]["video"]["href"].rsplit("/", 1)[1])
    response = get_video.sync_detailed(video_id, client=server.client(root_token))
    assert response.status_code == 200, f"video read failed: {response.status_code} {response.content!r}"
    video = json.loads(response.content)

    assert "name" not in video
