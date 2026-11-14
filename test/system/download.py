"""Black-box proof of the `download` capability.

The module uploads one image, one audio and one video through the real upload flow, then reads their content over HTTP:
authenticated with the session token only, decided per route, whole or in part, with the stored media type, the
validators, `HEAD` and the file route's attachment name. A caller who can discover an asset but lacks the route's action
is refused with `403`, while an undiscoverable or unknown asset answers `404`; neither reads bytes.

The declarative contract — the declared statuses, media types, headers and `_links` — is validated by Schemathesis from
the served document. These cases prove only what a schema cannot: which action a route decides, that the decision runs
on every request, that the credential is the header alone, and that the bytes and their attachment name are the stored
ones.
"""

import shutil
import sqlite3

import allure
from conftest import FIXTURE_DIRECTORY, provision, upload
from harness import Sut, link_of

IMAGE_CONTENT_TYPE = "image/png"
AUDIO_CONTENT_TYPE = "audio/flac"
VIDEO_CONTENT_TYPE = "video/mp4"

# A fixture large enough for the suffix and closed ranges the module requests.
LARGE = "track.flac"

VIEWER = "VIEWER"
PRIVATE = "PRIVATE"


def image_file(server: Sut, token: str, name: str = "cover.png") -> dict:
    """Upload an image and return its file representation."""
    return upload(server, token, name, (FIXTURE_DIRECTORY / "cover.png").read_bytes(), IMAGE_CONTENT_TYPE)


def audio_file(server: Sut, token: str, name: str = "track.flac") -> dict:
    """Upload an audio file and return its file representation."""
    return upload(server, token, name, (FIXTURE_DIRECTORY / LARGE).read_bytes(), AUDIO_CONTENT_TYPE)


def video_file(server: Sut, token: str, name: str = "clip.mp4") -> dict:
    """Upload a video and return its file representation."""
    return upload(server, token, name, (FIXTURE_DIRECTORY / "clip.mp4").read_bytes(), VIDEO_CONTENT_TYPE)


def typed(server: Sut, token: str, file: dict, rel: str) -> dict:
    """Follow a file's typed link and return the representation."""
    response = server.follow(file, rel, token=token)
    assert response.status_code == 200, f"{rel} read failed: {response.status_code} {response.text}"
    return response.json()


def content(server: Sut, token: str, representation: dict, *, headers: dict | None = None):
    """GET a representation's `content` link."""
    return server.request("GET", link_of(representation, "content"), token=token, headers=headers)


def filed_content(server: Sut, token: str, file: dict, *, headers: dict | None = None):
    """GET the file content route of a stored file."""
    return server.request("GET", f"{link_of(file, 'self')}/content", token=token, headers=headers)


def share_viewer(server: Sut, token: str, file: dict, user_id: int, *, effect: str = "ALLOW") -> None:
    """Replace the file's policy with one viewer grant (or deny) for `user_id`."""
    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/FILE/{file['id']}",
        token=token,
        json={
            "visibility": PRIVATE,
            "grants": [
                {
                    "principal": {"kind": "USER", "id": user_id},
                    "role": VIEWER,
                    "effect": effect,
                }
            ],
        },
    )
    assert response.status_code == 200, f"policy failed: {response.status_code} {response.text}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-001")
@allure.story("SCN-DOWNLOAD-001-A")
def test_a_bearer_request_reads_content(server: Sut, root_token: str) -> None:
    """A bearer request reads the bytes with the stored media type."""
    file = image_file(server, root_token)
    image = typed(server, root_token, file, "image")

    response = content(server, root_token, image)

    assert response.status_code == 200, f"content failed: {response.status_code} {response.text}"
    assert response.content == (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    assert response.headers["Content-Type"] == IMAGE_CONTENT_TYPE


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-001")
@allure.story("SCN-DOWNLOAD-001-B")
def test_a_missing_token_is_refused(server: Sut, root_token: str) -> None:
    """A headerless content request answers 401 and reads nothing."""
    image = typed(server, root_token, image_file(server, root_token), "image")

    response = server.request("GET", link_of(image, "content"))

    assert response.status_code == 401
    assert response.content != (FIXTURE_DIRECTORY / "cover.png").read_bytes()


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-001")
@allure.story("SCN-DOWNLOAD-001-C")
def test_a_query_value_is_not_a_credential(server: Sut, root_token: str) -> None:
    """An `exp`/`sig` query value without a header answers 401."""
    image = typed(server, root_token, image_file(server, root_token), "image")

    response = server.request("GET", f"{link_of(image, 'content')}?exp=4102444800&sig=tampered")

    assert response.status_code == 401


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-002")
@allure.story("SCN-DOWNLOAD-002-A")
def test_image_content_decides_view(server: Sut, root_token: str) -> None:
    """A viewer grant reads image content."""
    _user_id, user = provision(server, root_token, "alice")
    file = image_file(server, root_token)
    share_viewer(server, root_token, file, _user_id)
    image = typed(server, root_token, file, "image")

    response = content(server, user, image)

    assert response.status_code == 200, f"image content failed: {response.status_code} {response.text}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-002")
@allure.story("SCN-DOWNLOAD-002-B")
def test_audio_and_video_content_decide_stream(server: Sut, root_token: str) -> None:
    """A viewer grant read and stream audio and video content."""
    user_id, user = provision(server, root_token, "alice")
    audio = audio_file(server, root_token)
    video = video_file(server, root_token)
    share_viewer(server, root_token, audio, user_id)
    share_viewer(server, root_token, video, user_id)
    typed_audio = typed(server, root_token, audio, "audio")
    typed_video = typed(server, root_token, video, "video")

    audio_response = content(server, user, typed_audio)
    video_response = content(server, user, typed_video)

    assert audio_response.status_code == 200, f"audio failed: {audio_response.status_code}"
    assert video_response.status_code == 200, f"video failed: {video_response.status_code}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-002")
@allure.story("SCN-DOWNLOAD-002-C")
def test_file_content_decides_download(server: Sut, root_token: str) -> None:
    """A viewer grant without `Download` cannot read file content."""
    user_id, user = provision(server, root_token, "alice")
    file = image_file(server, root_token)
    share_viewer(server, root_token, file, user_id)

    response = filed_content(server, user, file)

    assert response.status_code == 403, f"file content allowed: {response.status_code}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-002")
@allure.story("SCN-DOWNLOAD-002-D")
def test_a_changed_decision_applies_to_the_next_request(server: Sut, root_token: str) -> None:
    """A replacement policy is consulted by the next content request."""
    user_id, user = provision(server, root_token, "alice")
    file = image_file(server, root_token)
    image = typed(server, root_token, file, "image")
    share_viewer(server, root_token, file, user_id)

    before = content(server, user, image)
    share_viewer(server, root_token, file, user_id, effect="DENY")
    after = content(server, user, image)

    assert before.status_code == 200
    assert after.status_code == 404


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-003")
@allure.story("SCN-DOWNLOAD-003-A")
def test_image_content_carries_the_stored_type(server: Sut, root_token: str) -> None:
    """The image content carries the bytes and type stored for its file."""
    image = typed(server, root_token, image_file(server, root_token), "image")

    response = content(server, root_token, image)

    assert response.headers["Content-Type"] == IMAGE_CONTENT_TYPE
    assert response.content == (FIXTURE_DIRECTORY / "cover.png").read_bytes()


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-003")
@allure.story("SCN-DOWNLOAD-003-B")
def test_audio_and_video_content_carry_the_stored_type(server: Sut, root_token: str) -> None:
    """Audio and video content carry their own stored type and bytes."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    video = typed(server, root_token, video_file(server, root_token), "video")

    audio_response = content(server, root_token, audio)
    video_response = content(server, root_token, video)

    assert audio_response.headers["Content-Type"] == AUDIO_CONTENT_TYPE
    assert audio_response.content == (FIXTURE_DIRECTORY / LARGE).read_bytes()
    assert video_response.headers["Content-Type"] == VIDEO_CONTENT_TYPE


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-003")
@allure.story("SCN-DOWNLOAD-003-C")
def test_file_content_carries_the_stored_type(server: Sut, root_token: str) -> None:
    """File content carries the stored type of the file."""
    file = image_file(server, root_token)

    response = filed_content(server, root_token, file)

    assert response.status_code == 200, f"file content failed: {response.status_code} {response.text}"
    assert response.headers["Content-Type"] == IMAGE_CONTENT_TYPE


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-004")
@allure.story("SCN-DOWNLOAD-004-B")
def test_a_head_request_answers_without_content(server: Sut, root_token: str) -> None:
    """`HEAD` answers the headers of `GET` and no body."""
    image = typed(server, root_token, image_file(server, root_token), "image")
    href = link_of(image, "content")

    whole = server.request("GET", href, token=root_token)
    head = server.request("HEAD", href, token=root_token)

    assert head.status_code == 200
    assert head.content == b""
    assert head.headers["Content-Length"] == whole.headers["Content-Length"]
    assert head.headers["Content-Type"] == whole.headers["Content-Type"]


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-A")
def test_a_closed_range(server: Sut, root_token: str) -> None:
    """A closed range answers 206 with exactly the requested bytes."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    data = (FIXTURE_DIRECTORY / LARGE).read_bytes()

    response = content(server, root_token, audio, headers={"Range": "bytes=0-99"})

    assert response.status_code == 206
    assert response.content == data[:100]
    assert response.headers["Content-Range"] == f"bytes 0-99/{len(data)}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-B")
def test_an_open_ended_range(server: Sut, root_token: str) -> None:
    """An open-ended range answers from its offset to the end."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    data = (FIXTURE_DIRECTORY / LARGE).read_bytes()

    response = content(server, root_token, audio, headers={"Range": "bytes=100-"})

    assert response.status_code == 206
    assert response.content == data[100:]


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-C")
def test_a_suffix_range(server: Sut, root_token: str) -> None:
    """A suffix range answers the final bytes."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    data = (FIXTURE_DIRECTORY / LARGE).read_bytes()

    response = content(server, root_token, audio, headers={"Range": "bytes=-500"})

    assert response.status_code == 206
    assert response.content == data[-500:]


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-D")
def test_an_unsatisfiable_range(server: Sut, root_token: str) -> None:
    """A range past the end answers 416 with the content size."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    size = len((FIXTURE_DIRECTORY / LARGE).read_bytes())

    response = content(server, root_token, audio, headers={"Range": f"bytes={size}-"})

    assert response.status_code == 416
    assert response.headers["Content-Range"] == f"bytes */{size}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-E")
def test_more_than_one_range(server: Sut, root_token: str) -> None:
    """Several ranges in one header answer the whole content."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    data = (FIXTURE_DIRECTORY / LARGE).read_bytes()

    response = content(server, root_token, audio, headers={"Range": "bytes=0-1,3-4"})

    assert response.status_code == 200
    assert response.content == data


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-006")
@allure.story("SCN-DOWNLOAD-006-A")
def test_a_content_response_carries_its_validators(server: Sut, root_token: str) -> None:
    """Content carries an `ETag` and a `Last-Modified`."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")

    response = content(server, root_token, audio)

    assert response.headers["ETag"].startswith('"')
    assert response.headers["Last-Modified"].endswith("GMT")


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-006")
@allure.story("SCN-DOWNLOAD-006-B")
def test_unchanged_content_answers_not_modified(server: Sut, root_token: str) -> None:
    """A matching `If-None-Match` answers 304 with no body."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    etag = content(server, root_token, audio).headers["ETag"]

    response = content(server, root_token, audio, headers={"If-None-Match": etag})

    assert response.status_code == 304
    assert response.content == b""


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-006")
@allure.story("SCN-DOWNLOAD-006-C")
def test_a_range_is_conditional_on_the_content(server: Sut, root_token: str) -> None:
    """A range whose `If-Range` matches answers 206."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    etag = content(server, root_token, audio).headers["ETag"]

    response = content(server, root_token, audio, headers={"Range": "bytes=0-9", "If-Range": etag})

    assert response.status_code == 206


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-006")
@allure.story("SCN-DOWNLOAD-006-D")
def test_a_stale_validator_answers_the_whole_content(server: Sut, root_token: str) -> None:
    """A range whose `If-Range` does not match answers 200."""
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    data = (FIXTURE_DIRECTORY / LARGE).read_bytes()

    response = content(
        server,
        root_token,
        audio,
        headers={"Range": "bytes=0-9", "If-Range": '"different"'},
    )

    assert response.status_code == 200
    assert response.content == data


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-007")
@allure.story("SCN-DOWNLOAD-007-A")
def test_a_typed_file_keeps_its_original_name(server: Sut, root_token: str) -> None:
    """The file route names the typed media row's original name."""
    file = image_file(server, root_token, "holiday.png")

    response = filed_content(server, root_token, file)

    assert 'filename="holiday.png"' in response.headers["Content-Disposition"]


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-007")
@allure.story("SCN-DOWNLOAD-007-B")
def test_a_bare_file_keeps_its_stored_name(server: Sut, root_token: str) -> None:
    """A file with no typed row names the final segment of its stored path."""
    file = image_file(server, root_token, "holiday.png")
    with sqlite3.connect(server.dir / "mnemorium.db") as connection:
        connection.execute("DELETE FROM image WHERE file_id = ?", (file["id"],))
    expected = file["path"].rsplit("/", 1)[-1]

    response = filed_content(server, root_token, file)

    assert response.status_code == 200, f"file content failed: {response.status_code} {response.text}"
    assert f'filename="{expected}"' in response.headers["Content-Disposition"]


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-007")
@allure.story("SCN-DOWNLOAD-007-C")
def test_a_display_route_is_not_an_attachment(server: Sut, root_token: str) -> None:
    """Image, audio and video content carry no disposition."""
    image = typed(server, root_token, image_file(server, root_token), "image")
    audio = typed(server, root_token, audio_file(server, root_token), "audio")
    video = typed(server, root_token, video_file(server, root_token), "video")

    for representation in (image, audio, video):
        response = content(server, root_token, representation)
        assert "Content-Disposition" not in response.headers


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-008")
@allure.story("SCN-DOWNLOAD-008-A")
def test_an_undiscoverable_asset_answers_not_found(server: Sut, root_token: str) -> None:
    """An unknown asset and an undiscoverable one both answer 404."""
    user_id, user = provision(server, root_token, "alice")
    file = image_file(server, root_token)
    image = typed(server, root_token, file, "image")
    unknown = link_of(image, "content").rsplit("/", 2)[0] + "/999999/content"

    undiscoverable = content(server, user, image)
    missing = server.request("GET", unknown, token=user)

    assert undiscoverable.status_code == 404
    assert missing.status_code == 404
    assert user_id > 0


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-008")
@allure.story("SCN-DOWNLOAD-008-B")
def test_a_visible_asset_whose_action_refuses_answers_forbidden(server: Sut, root_token: str) -> None:
    """A discoverable file without `Download` answers 403."""
    user_id, user = provision(server, root_token, "alice")
    file = image_file(server, root_token)
    share_viewer(server, root_token, file, user_id)

    response = filed_content(server, user, file)

    assert response.status_code == 403
    assert response.content != (FIXTURE_DIRECTORY / "cover.png").read_bytes()


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-008")
@allure.story("SCN-DOWNLOAD-008-C")
def test_an_absent_library_root_answers_unavailable(server: Sut, root_token: str) -> None:
    """A content request while the library root is absent answers 503."""
    image = typed(server, root_token, image_file(server, root_token), "image")
    server.stop()
    shutil.rmtree(server.dir / "media", ignore_errors=True)
    server.start(create_media=False)

    response = content(server, root_token, image)

    assert response.status_code == 503


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-008")
@allure.story("SCN-DOWNLOAD-008-D")
def test_a_registered_file_without_bytes_answers_server_error(server: Sut, root_token: str) -> None:
    """A registered file whose bytes are absent answers 500."""
    file = image_file(server, root_token)
    (server.dir / "media" / file["path"]).unlink()

    response = filed_content(server, root_token, file)

    assert response.status_code == 500
