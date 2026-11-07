"""E2E tests for UC-006 - Upload Media File.

The chunked upload lifecycle (begin, write chunk, status, complete, cancel,
expiry) is exercised end to end against the running container. Image, video and
audio completion, and the unsupported-media alternative flow, all run against
the shipped image, whose `ffmpeg` package provides the `ffprobe` binary the
media probe needs.
"""

import base64
import hashlib
import time
from collections.abc import Callable

import requests
from conftest import (
    ASSET_FILE_PATH,
    AUDIO_CONTENT_TYPE,
    UPLOAD_EXPIRY_WAIT_SECONDS,
    audio,
    png,
    video,
)

_API_PREFIX = "/api/v1"
_UPLOAD_PATH = f"{_API_PREFIX}/asset/upload"
_AUDIO_LINK_PREFIX = f"{_API_PREFIX}/asset/audio/"


def _begin(
    server_url: str,
    token: str,
    content_type: str,
    file_name: str,
    data: bytes,
) -> tuple[int, int]:
    """Begin an upload for `data` and return its identifier and chunk size."""
    digest = hashlib.sha256(data).digest()
    begin = requests.post(
        f"{server_url}{_UPLOAD_PATH}",
        headers={"Authorization": f"Bearer {token}"},
        json={
            "content_type": content_type,
            "file_name": file_name,
            "file_size": len(data),
            "integrity_hash": digest.hex(),
        },
        timeout=10,
    )
    assert begin.status_code == 201, f"begin failed: {begin.status_code} {begin.text}"
    body = begin.json()
    return body["upload_id"], body["chunk_size"]


def _put_chunk(
    server_url: str,
    token: str,
    upload_id: int,
    chunk_number: int,
    start: int,
    chunk: bytes,
    total: int,
) -> requests.Response:
    """Send one chunk of the content, with its range and digest."""
    digest = hashlib.sha256(chunk).digest()
    return requests.put(
        f"{server_url}{_UPLOAD_PATH}/{upload_id}/chunk/{chunk_number}",
        headers={
            "Authorization": f"Bearer {token}",
            "Content-Type": "application/octet-stream",
            "Content-Range": f"bytes {start}-{start + len(chunk) - 1}/{total}",
            "Content-Digest": f"sha-256=:{base64.b64encode(digest).decode()}:",
        },
        data=chunk,
        timeout=30,
    )


def _status(server_url: str, token: str, upload_id: int) -> requests.Response:
    """Read one upload session."""
    return requests.get(
        f"{server_url}{_UPLOAD_PATH}/{upload_id}",
        headers={"Authorization": f"Bearer {token}"},
        timeout=10,
    )


def _complete(server_url: str, token: str, upload_id: int) -> requests.Response:
    """Complete one upload session."""
    return requests.post(
        f"{server_url}{_UPLOAD_PATH}/{upload_id}/complete",
        headers={"Authorization": f"Bearer {token}"},
        timeout=30,
    )


def _cancel(server_url: str, token: str, upload_id: int) -> requests.Response:
    """Cancel one upload session."""
    return requests.delete(
        f"{server_url}{_UPLOAD_PATH}/{upload_id}",
        headers={"Authorization": f"Bearer {token}"},
        timeout=10,
    )


def _get_file(server_url: str, token: str, file_id: int) -> requests.Response:
    """Read one stored file."""
    return requests.get(
        f"{server_url}{ASSET_FILE_PATH}/{file_id}",
        headers={"Authorization": f"Bearer {token}"},
        timeout=10,
    )


def _upload_and_complete(
    server_url: str,
    token: str,
    content_type: str,
    file_name: str,
    data: bytes,
) -> requests.Response:
    """Begin an upload, store `data` as one chunk and complete the session."""
    upload_id, _chunk_size = _begin(server_url, token, content_type, file_name, data)
    chunk = _put_chunk(server_url, token, upload_id, 0, 0, data, len(data))
    assert chunk.status_code == 200, f"chunk failed: {chunk.status_code} {chunk.text}"
    return _complete(server_url, token, upload_id)


def _finalized_file_id(body: dict) -> int:
    """Return the file identifier a completed upload links in `_links.file`."""
    href = body["_links"]["file"]["href"]
    assert href.startswith(f"{ASSET_FILE_PATH}/"), f"complete did not link the file: {href}"
    return int(href.removeprefix(f"{ASSET_FILE_PATH}/"))


def test_uc006_upload_image_completion(
    server_url: str,
    standard_user_token: str,
) -> None:
    response = _upload_and_complete(
        server_url,
        standard_user_token,
        "image/png",
        "pixel.png",
        png(2, 3),
    )

    assert response.status_code == 200, f"complete failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert _finalized_file_id(body) > 0
    assert body["is_finished"] is True
    assert body["_links"]["self"]["href"].startswith(f"{_UPLOAD_PATH}/")


def test_uc006_upload_unsupported_media(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    # Declared as an image but the bytes are not decodable: a client-fixable
    # rejection, and nothing is stored.
    response = _upload_and_complete(
        server_url,
        standard_user_token,
        "image/png",
        "not-an-image.png",
        b"definitely not an image",
    )

    assert_error_body(response, 422)


def test_uc006_upload_video_completion(
    server_url: str,
    standard_user_token: str,
) -> None:
    response = _upload_and_complete(
        server_url,
        standard_user_token,
        "video/mp4",
        "clip.mp4",
        video(),
    )

    assert response.status_code == 200, f"complete failed: {response.status_code} {response.text}"
    body = response.json()
    assert _finalized_file_id(body) > 0
    assert body["is_finished"] is True


def test_uc006_upload_audio_completion(
    server_url: str,
    standard_user_token: str,
) -> None:
    # Audio completion probes the content and writes the audio row in the same
    # unit of work, so the file exposes its typed audio immediately.
    response = _upload_and_complete(
        server_url,
        standard_user_token,
        AUDIO_CONTENT_TYPE,
        "tone.wav",
        audio(),
    )

    assert response.status_code == 200, f"complete failed: {response.status_code} {response.text}"
    body = response.json()
    file_id = _finalized_file_id(body)
    assert file_id > 0
    assert body["is_finished"] is True

    stored = _get_file(server_url, standard_user_token, file_id)
    assert stored.status_code == 200, f"file read failed: {stored.status_code} {stored.text}"
    audio_href = stored.json()["_links"]["audio"]["href"]
    assert audio_href.startswith(_AUDIO_LINK_PREFIX)
    assert audio_href.removeprefix(_AUDIO_LINK_PREFIX).isdigit()


def test_uc006_upload_resumes_a_multi_chunk_session(
    server_url: str,
    standard_user_token: str,
) -> None:
    data = audio(60)
    upload_id, chunk_size = _begin(server_url, standard_user_token, AUDIO_CONTENT_TYPE, "long.wav", data)
    assert chunk_size > 0
    assert len(data) > chunk_size, "the fixture must span more than one chunk"

    first = _put_chunk(server_url, standard_user_token, upload_id, 0, 0, data[:chunk_size], len(data))
    assert first.status_code == 200, f"chunk failed: {first.status_code} {first.text}"
    partial = _status(server_url, standard_user_token, upload_id)
    assert partial.status_code == 200
    assert partial.json()["bitmap"][0] == "1"
    assert partial.json()["bitmap"].count("0") > 0
    assert partial.json()["is_finished"] is False

    second = _put_chunk(
        server_url,
        standard_user_token,
        upload_id,
        1,
        chunk_size,
        data[chunk_size:],
        len(data),
    )
    assert second.status_code == 200, f"chunk failed: {second.status_code} {second.text}"
    resumed = _status(server_url, standard_user_token, upload_id)
    assert resumed.json()["bitmap"] == "1" * resumed.json()["total_chunks"]

    completed = _complete(server_url, standard_user_token, upload_id)
    assert completed.status_code == 200, f"complete failed: {completed.status_code} {completed.text}"
    assert _finalized_file_id(completed.json()) > 0


def test_uc006_upload_expired_session_is_gone(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    data = audio()
    chunk_upload_id, _chunk_size = _begin(server_url, standard_user_token, AUDIO_CONTENT_TYPE, "expiring.wav", data)
    complete_upload_id, _chunk_size = _begin(
        server_url, standard_user_token, AUDIO_CONTENT_TYPE, "expiring.wav", data + b"\x00"
    )

    time.sleep(UPLOAD_EXPIRY_WAIT_SECONDS)

    chunk = _put_chunk(server_url, standard_user_token, chunk_upload_id, 0, 0, data, len(data))
    assert_error_body(chunk, 410)
    completed = _complete(server_url, standard_user_token, complete_upload_id)
    assert_error_body(completed, 410)


def test_uc006_upload_cancelled_session_is_removed(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    data = audio()
    upload_id, _chunk_size = _begin(server_url, standard_user_token, AUDIO_CONTENT_TYPE, "cancel.wav", data)

    assert _cancel(server_url, standard_user_token, upload_id).status_code == 204
    assert_error_body(_cancel(server_url, standard_user_token, upload_id), 404)
    assert_error_body(_complete(server_url, standard_user_token, upload_id), 404)


def test_uc006_upload_completed_session_cannot_be_cancelled(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    data = png(4, 4)
    upload_id, _chunk_size = _begin(server_url, standard_user_token, "image/png", "kept.png", data)
    stored = _put_chunk(server_url, standard_user_token, upload_id, 0, 0, data, len(data))
    assert stored.status_code == 200, f"chunk failed: {stored.status_code} {stored.text}"
    completed = _complete(server_url, standard_user_token, upload_id)
    assert completed.status_code == 200, f"complete failed: {completed.status_code} {completed.text}"
    file_id = _finalized_file_id(completed.json())

    assert_error_body(_cancel(server_url, standard_user_token, upload_id), 409)

    # The completed file survives the rejected cancellation.
    assert _get_file(server_url, standard_user_token, file_id).status_code == 200


def test_uc006_upload_deduplicates_within_a_caller(
    server_url: str,
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    data = png(5, 5)
    first = _upload_and_complete(server_url, standard_user_token, "image/png", "same.png", data)
    assert first.status_code == 200
    first_file_id = _finalized_file_id(first.json())
    assert first.json()["is_finished"] is True

    duplicate = _upload_and_complete(server_url, standard_user_token, "image/png", "same.png", data)
    assert duplicate.status_code == 200
    assert _finalized_file_id(duplicate.json()) == first_file_id
    assert duplicate.json()["is_finished"] is False

    other_caller = _upload_and_complete(server_url, second_standard_user_token, "image/png", "same.png", data)
    assert other_caller.status_code == 200
    assert _finalized_file_id(other_caller.json()) != first_file_id
