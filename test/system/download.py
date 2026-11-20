"""Black-box proof of the `download` capability."""

import json
import shutil
from itertools import count

import allure
import pytest
from conftest import SEED_MEDIA_DIRECTORY, login, provision, start_instance, upload
from harness import ROOT_ADMIN_USERNAME, Sut
from mnemorium_client.api.asset import get_file

# The seeded slots this module reads, documented in `seed/sql/0005_download.sql`.
IMAGE_FILE = 16101
AUDIO_FILE = 16102
VIDEO_FILE = 16103
HOLIDAY_FILE = 16104
ODD_NAME_FILE = 16105
BARE_FILE = 16106
VIEW_IMAGE_FILE = 16107
STREAM_AUDIO_FILE = 16108
STREAM_VIDEO_FILE = 16109
REFUSAL_FILE = 16110

IMAGE_CONTENT_TYPE = "image/png"
AUDIO_CONTENT_TYPE = "audio/flac"
VIDEO_CONTENT_TYPE = "video/mp4"

IMAGE_BYTES = (SEED_MEDIA_DIRECTORY / "download-image.png").read_bytes()
AUDIO_BYTES = (SEED_MEDIA_DIRECTORY / "download-audio.flac").read_bytes()
VIDEO_BYTES = (SEED_MEDIA_DIRECTORY / "download-video.mp4").read_bytes()
BARE_BYTES = (SEED_MEDIA_DIRECTORY / "download-bare.png").read_bytes()
REFUSAL_BYTES = (SEED_MEDIA_DIRECTORY / "refusal.png").read_bytes()

VIEWER = "VIEWER"
PRIVATE = "PRIVATE"

_ACCOUNTS = count()


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-001")
@allure.story("SCN-DOWNLOAD-001-A")
@allure.feature("REQ-DOWNLOAD-003")
@allure.story("SCN-DOWNLOAD-003-A")
def test_a_bearer_request_reads_content(server: Sut, root_token: str) -> None:
    """A bearer request reads the bytes with the stored media type."""
    file_response = get_file.sync_detailed(IMAGE_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "image", token=root_token)
    assert typed_response.status_code == 200, (
        f"image read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    image = json.loads(typed_response.content)

    response = server.request("GET", image["_links"]["content"]["href"], token=root_token)

    assert response.status_code == 200, f"content failed: {response.status_code} {response.content!r}"
    assert response.content == IMAGE_BYTES
    assert response.headers["Content-Type"] == IMAGE_CONTENT_TYPE


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-002")
@allure.story("SCN-DOWNLOAD-002-A")
def test_image_content_decides_view(server: Sut, root_token: str) -> None:
    """A viewer grant reads image content."""
    user_id, user = provision(server, root_token, f"download{next(_ACCOUNTS)}")
    granted = server.request(
        "PUT",
        f"{server.href('permission')}/resource/FILE/{VIEW_IMAGE_FILE}",
        token=root_token,
        json={
            "visibility": PRIVATE,
            "grants": [
                {
                    "principal": {"kind": "USER", "id": user_id},
                    "role": VIEWER,
                    "effect": "ALLOW",
                }
            ],
        },
    )
    assert granted.status_code == 200, f"policy failed: {granted.status_code} {granted.content!r}"
    file_response = get_file.sync_detailed(VIEW_IMAGE_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "image", token=root_token)
    assert typed_response.status_code == 200, (
        f"image read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    image = json.loads(typed_response.content)

    response = server.request("GET", image["_links"]["content"]["href"], token=user)

    assert response.status_code == 200, f"image content failed: {response.status_code} {response.content!r}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-002")
@allure.story("SCN-DOWNLOAD-002-B")
def test_audio_content_decides_stream(server: Sut, root_token: str) -> None:
    """A viewer grant reads audio content."""
    user_id, user = provision(server, root_token, f"download{next(_ACCOUNTS)}")
    granted = server.request(
        "PUT",
        f"{server.href('permission')}/resource/FILE/{STREAM_AUDIO_FILE}",
        token=root_token,
        json={
            "visibility": PRIVATE,
            "grants": [
                {
                    "principal": {"kind": "USER", "id": user_id},
                    "role": VIEWER,
                    "effect": "ALLOW",
                }
            ],
        },
    )
    assert granted.status_code == 200, f"policy failed: {granted.status_code} {granted.content!r}"
    file_response = get_file.sync_detailed(STREAM_AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request("GET", audio["_links"]["content"]["href"], token=user)

    assert response.status_code == 200, f"audio content failed: {response.status_code} {response.content!r}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-002")
@allure.story("SCN-DOWNLOAD-002-B")
def test_video_content_decides_stream(server: Sut, root_token: str) -> None:
    """A viewer grant reads video content."""
    user_id, user = provision(server, root_token, f"download{next(_ACCOUNTS)}")
    granted = server.request(
        "PUT",
        f"{server.href('permission')}/resource/FILE/{STREAM_VIDEO_FILE}",
        token=root_token,
        json={
            "visibility": PRIVATE,
            "grants": [
                {
                    "principal": {"kind": "USER", "id": user_id},
                    "role": VIEWER,
                    "effect": "ALLOW",
                }
            ],
        },
    )
    assert granted.status_code == 200, f"policy failed: {granted.status_code} {granted.content!r}"
    file_response = get_file.sync_detailed(STREAM_VIDEO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "video", token=root_token)
    assert typed_response.status_code == 200, (
        f"video read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    video = json.loads(typed_response.content)

    response = server.request("GET", video["_links"]["content"]["href"], token=user)

    assert response.status_code == 200, f"video content failed: {response.status_code} {response.content!r}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-003")
@allure.story("SCN-DOWNLOAD-003-B")
def test_audio_content_carries_the_stored_type(server: Sut, root_token: str) -> None:
    """Audio content carries the bytes and type stored for its file."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request("GET", audio["_links"]["content"]["href"], token=root_token)

    assert response.status_code == 200, f"content failed: {response.status_code} {response.content!r}"
    assert response.headers["Content-Type"] == AUDIO_CONTENT_TYPE
    assert response.content == AUDIO_BYTES


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-003")
@allure.story("SCN-DOWNLOAD-003-B")
def test_video_content_carries_the_stored_type(server: Sut, root_token: str) -> None:
    """Video content carries the bytes and type stored for its file."""
    file_response = get_file.sync_detailed(VIDEO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "video", token=root_token)
    assert typed_response.status_code == 200, (
        f"video read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    video = json.loads(typed_response.content)

    response = server.request("GET", video["_links"]["content"]["href"], token=root_token)

    assert response.status_code == 200, f"content failed: {response.status_code} {response.content!r}"
    assert response.headers["Content-Type"] == VIDEO_CONTENT_TYPE
    assert response.content == VIDEO_BYTES


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-003")
@allure.story("SCN-DOWNLOAD-003-C")
def test_file_content_carries_the_stored_type(server: Sut, root_token: str) -> None:
    """File content carries the stored type of the file."""
    response = server.request(
        "GET",
        f"{server.href('file')}/{IMAGE_FILE}/content",
        token=root_token,
    )

    assert response.status_code == 200, f"file content failed: {response.status_code} {response.content!r}"
    assert response.headers["Content-Type"] == IMAGE_CONTENT_TYPE
    assert response.content == IMAGE_BYTES


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-004")
@allure.story("SCN-DOWNLOAD-004-B")
def test_a_head_request_answers_without_content(server: Sut, root_token: str) -> None:
    """`HEAD` answers the headers of `GET` and no body."""
    file_response = get_file.sync_detailed(IMAGE_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "image", token=root_token)
    assert typed_response.status_code == 200, (
        f"image read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    image = json.loads(typed_response.content)
    href = image["_links"]["content"]["href"]

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
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request(
        "GET",
        audio["_links"]["content"]["href"],
        token=root_token,
        headers={"Range": "bytes=0-99"},
    )

    assert response.status_code == 206
    assert response.content == AUDIO_BYTES[:100]
    assert response.headers["Content-Range"] == f"bytes 0-99/{len(AUDIO_BYTES)}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-B")
def test_an_open_ended_range(server: Sut, root_token: str) -> None:
    """An open-ended range answers from its offset to the end."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request(
        "GET",
        audio["_links"]["content"]["href"],
        token=root_token,
        headers={"Range": "bytes=100-"},
    )

    assert response.status_code == 206
    assert response.content == AUDIO_BYTES[100:]


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-C")
def test_a_suffix_range(server: Sut, root_token: str) -> None:
    """A suffix range answers the final bytes."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request(
        "GET",
        audio["_links"]["content"]["href"],
        token=root_token,
        headers={"Range": "bytes=-500"},
    )

    assert response.status_code == 206
    assert response.content == AUDIO_BYTES[-500:]


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-D")
def test_an_unsatisfiable_range(server: Sut, root_token: str) -> None:
    """A range past the end answers 416 with the content size."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request(
        "GET",
        audio["_links"]["content"]["href"],
        token=root_token,
        headers={"Range": f"bytes={len(AUDIO_BYTES)}-"},
    )

    assert response.status_code == 416
    assert response.headers["Content-Range"] == f"bytes */{len(AUDIO_BYTES)}"


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-005")
@allure.story("SCN-DOWNLOAD-005-E")
def test_more_than_one_range(server: Sut, root_token: str) -> None:
    """Several ranges in one header answer the whole content."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request(
        "GET",
        audio["_links"]["content"]["href"],
        token=root_token,
        headers={"Range": "bytes=0-1,3-4"},
    )

    assert response.status_code == 200
    assert response.content == AUDIO_BYTES


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-006")
@allure.story("SCN-DOWNLOAD-006-A")
def test_a_content_response_carries_its_validators(server: Sut, root_token: str) -> None:
    """Content carries an `ETag` and a `Last-Modified`."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request("GET", audio["_links"]["content"]["href"], token=root_token)

    assert response.headers["ETag"].startswith('"')
    assert response.headers["Last-Modified"].endswith("GMT")


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-006")
@allure.story("SCN-DOWNLOAD-006-B")
def test_unchanged_content_answers_not_modified(server: Sut, root_token: str) -> None:
    """A matching `If-None-Match` answers 304 with no body."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)
    etag = server.request("GET", audio["_links"]["content"]["href"], token=root_token).headers["ETag"]

    response = server.request(
        "GET",
        audio["_links"]["content"]["href"],
        token=root_token,
        headers={"If-None-Match": etag},
    )

    assert response.status_code == 304
    assert response.content == b""


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-006")
@allure.story("SCN-DOWNLOAD-006-C")
def test_a_range_is_conditional_on_the_content(server: Sut, root_token: str) -> None:
    """A range whose `If-Range` matches answers 206."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)
    etag = server.request("GET", audio["_links"]["content"]["href"], token=root_token).headers["ETag"]

    response = server.request(
        "GET",
        audio["_links"]["content"]["href"],
        token=root_token,
        headers={"Range": "bytes=0-9", "If-Range": etag},
    )

    assert response.status_code == 206


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-006")
@allure.story("SCN-DOWNLOAD-006-D")
def test_a_stale_validator_answers_the_whole_content(server: Sut, root_token: str) -> None:
    """A range whose `If-Range` does not match answers 200."""
    file_response = get_file.sync_detailed(AUDIO_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_response = server.follow(file, "audio", token=root_token)
    assert typed_response.status_code == 200, (
        f"audio read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    audio = json.loads(typed_response.content)

    response = server.request(
        "GET",
        audio["_links"]["content"]["href"],
        token=root_token,
        headers={"Range": "bytes=0-9", "If-Range": '"different"'},
    )

    assert response.status_code == 200
    assert response.content == AUDIO_BYTES


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-007")
@allure.story("SCN-DOWNLOAD-007-A")
def test_a_typed_file_keeps_its_original_name(server: Sut, root_token: str) -> None:
    """The file route names the typed media row's original name."""
    response = server.request(
        "GET",
        f"{server.href('file')}/{HOLIDAY_FILE}/content",
        token=root_token,
    )

    assert response.status_code == 200, f"file content failed: {response.status_code} {response.content!r}"
    assert response.headers["Content-Disposition"] == 'attachment; filename="holiday.png"'


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-007")
@allure.story("SCN-DOWNLOAD-007-B")
def test_a_bare_file_keeps_its_stored_name(server: Sut, root_token: str) -> None:
    """A file with no typed row names the final segment of its stored path."""
    response = server.request(
        "GET",
        f"{server.href('file')}/{BARE_FILE}/content",
        token=root_token,
    )

    assert response.status_code == 200, f"file content failed: {response.status_code} {response.content!r}"
    assert response.headers["Content-Disposition"] == 'attachment; filename="download-bare.png"'
    assert response.content == BARE_BYTES


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-007")
@allure.story("SCN-DOWNLOAD-007-C")
def test_a_display_route_is_not_an_attachment(server: Sut, root_token: str) -> None:
    """Image, audio and video content carry no disposition."""
    for file_id, rel in ((IMAGE_FILE, "image"), (AUDIO_FILE, "audio"), (VIDEO_FILE, "video")):
        file_response = get_file.sync_detailed(file_id, client=server.client(root_token))
        assert file_response.status_code == 200, (
            f"file read failed: {file_response.status_code} {file_response.content!r}"
        )
        file = json.loads(file_response.content)
        typed_response = server.follow(file, rel, token=root_token)
        assert typed_response.status_code == 200, (
            f"{rel} read failed: {typed_response.status_code} {typed_response.content!r}"
        )
        typed = json.loads(typed_response.content)

        response = server.request("GET", typed["_links"]["content"]["href"], token=root_token)

        assert "Content-Disposition" not in response.headers


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-007")
@allure.story("SCN-DOWNLOAD-007-D")
def test_an_awkward_file_name_is_encoded(server: Sut, root_token: str) -> None:
    """A name carrying a quote is encoded so it round-trips intact."""
    response = server.request(
        "GET",
        f"{server.href('file')}/{ODD_NAME_FILE}/content",
        token=root_token,
    )

    assert response.status_code == 200, f"file content failed: {response.status_code} {response.content!r}"
    disposition = response.headers["Content-Disposition"]
    assert 'filename="odd _name_.png"' in disposition
    assert "filename*=UTF-8''odd%20%22name%22.png" in disposition


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-002")
@allure.story("SCN-DOWNLOAD-002-C")
@allure.feature("REQ-DOWNLOAD-008")
@allure.story("SCN-DOWNLOAD-008-B")
@allure.feature("REQ-DOWNLOAD-009")
@allure.story("SCN-DOWNLOAD-009-A")
def test_a_visible_asset_whose_action_refuses_answers_forbidden(server: Sut, root_token: str) -> None:
    """A discoverable file without `Download` answers 403 and reads no bytes."""
    user_id, user = provision(server, root_token, f"download{next(_ACCOUNTS)}")
    granted = server.request(
        "PUT",
        f"{server.href('permission')}/resource/FILE/{REFUSAL_FILE}",
        token=root_token,
        json={
            "visibility": PRIVATE,
            "grants": [
                {
                    "principal": {"kind": "USER", "id": user_id},
                    "role": VIEWER,
                    "effect": "ALLOW",
                }
            ],
        },
    )
    assert granted.status_code == 200, f"policy failed: {granted.status_code} {granted.content!r}"

    visible = get_file.sync_detailed(REFUSAL_FILE, client=server.client(user))
    refused = server.request(
        "GET",
        f"{server.href('file')}/{REFUSAL_FILE}/content",
        token=user,
    )

    assert visible.status_code == 200, f"file not visible: {visible.status_code} {visible.content!r}"
    assert refused.status_code == 403, f"file content allowed: {refused.status_code} {refused.content!r}"
    assert refused.content != REFUSAL_BYTES


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-008")
@allure.story("SCN-DOWNLOAD-008-C")
@pytest.mark.own_instance(reason="restart")
def test_an_absent_library_root_answers_unavailable(sut: Sut) -> None:
    """A content request while the library root is absent answers 503."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    file = upload(server, root_token, "cover.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    typed_response = server.follow(file, "image", token=root_token)
    assert typed_response.status_code == 200, (
        f"image read failed: {typed_response.status_code} {typed_response.content!r}"
    )
    image = json.loads(typed_response.content)
    server.stop()
    shutil.rmtree(server.dir / "media", ignore_errors=True)
    server.start(create_media=False)

    response = server.request("GET", image["_links"]["content"]["href"], token=root_token)

    assert response.status_code == 503


@allure.epic("Download")
@allure.feature("REQ-DOWNLOAD-008")
@allure.story("SCN-DOWNLOAD-008-D")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_registered_file_without_bytes_answers_server_error(sut: Sut) -> None:
    """A registered file whose bytes are absent answers 500."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    file = upload(server, root_token, "cover.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    (server.dir / "media" / file["path"]).unlink()

    response = server.request(
        "GET",
        f"{server.href('file')}/{file['id']}/content",
        token=root_token,
    )

    assert response.status_code == 500
