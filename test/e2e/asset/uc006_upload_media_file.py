"""E2E tests for UC-006 - Upload Media File.

The chunked upload flow (begin, write chunk, complete) is exercised end to end
against the running container. Image completion and the unsupported-media
alternative flow run against the shipped image; video completion additionally
needs `ffprobe`, which the runtime image does not install yet (tracked by
https://github.com/Mnemorium/mnemorium/issues/242), so that case is marked
`xfail` and asserts the documented current `500`.
"""

import base64
import hashlib
import struct
import zlib
from collections.abc import Callable

import pytest
import requests

_API_PREFIX = "/api/v1"
_UPLOAD_PATH = f"{_API_PREFIX}/asset/upload"


def _chunk(chunk_type: bytes, data: bytes) -> bytes:
    """Build one PNG chunk with its CRC."""
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def _png(width: int, height: int) -> bytes:
    """Build a minimal valid RGB PNG of the given dimensions."""
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    scanline = b"\x00" + b"\xff\x00\x00" * width
    pixels = zlib.compress(scanline * height)
    return b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", header) + _chunk(b"IDAT", pixels) + _chunk(b"IEND", b"")


def _upload_and_complete(
    server_url: str,
    token: str,
    content_type: str,
    file_name: str,
    data: bytes,
) -> requests.Response:
    """Begin an upload, store `data` as one chunk and complete the session."""
    digest = hashlib.sha256(data).digest()
    authorization = {"Authorization": f"Bearer {token}"}
    begin = requests.post(
        f"{server_url}{_UPLOAD_PATH}",
        headers=authorization,
        json={
            "content_type": content_type,
            "file_name": file_name,
            "file_size": len(data),
            "integrity_hash": digest.hex(),
        },
        timeout=10,
    )
    assert begin.status_code == 201, f"begin failed: {begin.status_code} {begin.text}"
    upload_id = begin.json()["upload_id"]

    chunk = requests.put(
        f"{server_url}{_UPLOAD_PATH}/{upload_id}/chunk/0",
        headers={
            **authorization,
            "Content-Type": "application/octet-stream",
            "Content-Range": f"bytes 0-{len(data) - 1}/{len(data)}",
            "Content-Digest": f"sha-256=:{base64.b64encode(digest).decode()}:",
        },
        data=data,
        timeout=10,
    )
    assert chunk.status_code == 200, f"chunk failed: {chunk.status_code} {chunk.text}"

    return requests.post(
        f"{server_url}{_UPLOAD_PATH}/{upload_id}/complete",
        headers=authorization,
        timeout=30,
    )


def test_uc006_upload_image_completion(
    server_url: str,
    standard_user_token: str,
) -> None:
    response = _upload_and_complete(
        server_url,
        standard_user_token,
        "image/png",
        "pixel.png",
        _png(2, 3),
    )

    assert response.status_code == 200, f"complete failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert isinstance(body["file_id"], int) and body["file_id"] > 0
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


@pytest.mark.xfail(
    strict=False,
    reason="the runtime image does not install ffprobe; see #242",
)
def test_uc006_upload_video_completion(
    server_url: str,
    standard_user_token: str,
) -> None:
    # A video completion needs `ffprobe` in the runtime image; until it is
    # installed (https://github.com/Mnemorium/mnemorium/issues/242) the probe
    # cannot run and the endpoint answers 500. The case runs so the video path
    # is executed; when #242 lands the assertion should flip to a real video
    # upload asserting 200.
    response = _upload_and_complete(
        server_url,
        standard_user_token,
        "video/mp4",
        "clip.mp4",
        _png(2, 2),
    )
    assert response.status_code == 500, f"unexpected: {response.status_code} {response.text}"
