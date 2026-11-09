"""UC-031: read an asset's content.

The suite uploads one image, one audio and one video through the real upload
flow, then reads their content over HTTP: whole, ranged, conditional, `HEAD`,
credential-only, and each typed view with its file alias. It finally removes a
registered file from the host library root and watches the reconciliation turn
the content into `404`.
"""

import time
from collections.abc import Callable
from pathlib import Path

import pytest
import requests
from conftest import API_PREFIX, ASSET_FILE_PATH, audio, png, video

ASSET_AUDIO_PATH = f"{API_PREFIX}/asset/audio"
ASSET_IMAGE_PATH = f"{API_PREFIX}/asset/image"
ASSET_VIDEO_PATH = f"{API_PREFIX}/asset/video"

CONTENT_TIMEOUT_S = 10
RECONCILE_TIMEOUT_S = 15
_POLL_INTERVAL_S = 0.5


def _upload(
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    token: str,
    name: str,
    data: bytes,
    content_type: str,
) -> int:
    """Upload `data` and return the finalized backing file identifier."""
    upload_id = upload_media(token, name, data, content_type)
    return register_media(token, upload_id)


def _get(
    server_url: str,
    path: str,
    token: str | None = None,
    headers: dict | None = None,
) -> requests.Response:
    """GET `path` with the bearer token and any extra headers."""
    merged = dict(headers or {})
    if token is not None:
        merged["Authorization"] = f"Bearer {token}"
    return requests.get(
        f"{server_url}{path}",
        headers=merged,
        timeout=CONTENT_TIMEOUT_S,
        stream=True,
        allow_redirects=False,
    )


def _file_representation(server_url: str, token: str, file_id: int) -> dict:
    """Fetch the stored-file representation."""
    response = _get(server_url, f"{ASSET_FILE_PATH}/{file_id}", token)
    assert response.status_code == 200, f"file representation failed: {response.status_code} {response.text}"
    return response.json()


def _content_path(representation: dict) -> str:
    """Return the `_links.content` href of a representation."""
    href = representation["_links"]["content"]["href"]
    assert isinstance(href, str) and href.startswith("/api/v1/asset/"), href
    return href


@pytest.mark.parametrize("kind", ["image", "audio", "video"])
def test_typed_view_links_and_aliases(
    server_url: str,
    root_admin_token: str,
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    kind: str,
) -> None:
    """Each typed view is readable, links playable content and aliases its file."""
    data, content_type, name, path = {
        "image": (png(8, 8), "image/png", "uc031.png", ASSET_IMAGE_PATH),
        "audio": (audio(1), "audio/vnd.wave", "uc031.wav", ASSET_AUDIO_PATH),
        "video": (video(), "video/mp4", "uc031.mp4", ASSET_VIDEO_PATH),
    }[kind]
    file_id = _upload(upload_media, register_media, root_admin_token, name, data, content_type)
    representation = _file_representation(server_url, root_admin_token, file_id)
    typed_href = representation["_links"][kind]["href"]
    typed_id = int(typed_href.rsplit("/", 1)[1])

    # The typed representation links its own content and its file.
    typed = _get(server_url, typed_href, root_admin_token)
    assert typed.status_code == 200, f"{kind} view failed: {typed.status_code} {typed.text}"
    assert typed.headers["Cache-Control"] == "no-store"
    body = typed.json()
    assert body["_links"]["content"]["href"].startswith(f"{path}/{typed_id}/content")
    assert body["_links"]["file"]["href"] == f"{ASSET_FILE_PATH}/{file_id}"

    # The credential-only content read needs no Authorization header.
    content = _get(server_url, _content_path(body))
    assert content.status_code == 200
    assert content.content == data

    # The `/file` alias answers `303 See Other` naming the canonical file.
    alias = _get(server_url, f"{path}/{typed_id}/file", root_admin_token)
    assert alias.status_code == 303
    assert alias.headers["Location"] == f"{ASSET_FILE_PATH}/{file_id}"


def test_ranges_validators_head_and_credential(
    server_url: str,
    root_admin_token: str,
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
) -> None:
    """Whole, ranged, conditional, `HEAD` and credential-only reads agree."""
    data = png(16, 16)
    file_id = _upload(upload_media, register_media, root_admin_token, "uc031-range.png", data, "image/png")
    representation = _file_representation(server_url, root_admin_token, file_id)
    path = f"{ASSET_FILE_PATH}/{file_id}/content"

    whole = _get(server_url, path, root_admin_token)
    assert whole.status_code == 200
    assert whole.content == data
    assert whole.headers["Accept-Ranges"] == "bytes"
    assert whole.headers["Content-Length"] == str(len(data))
    assert whole.headers["Content-Type"] == "image/png"
    etag = whole.headers.get("ETag")
    modified = whole.headers.get("Last-Modified")
    assert etag is not None and modified is not None

    closed = _get(server_url, path, root_admin_token, {"Range": "bytes=0-9"})
    assert closed.status_code == 206
    assert closed.content == data[:10]
    assert closed.headers["Content-Range"] == f"bytes 0-9/{len(data)}"

    open_ended = _get(server_url, path, root_admin_token, {"Range": "bytes=10-"})
    assert open_ended.status_code == 206
    assert open_ended.content == data[10:]

    suffix = _get(server_url, path, root_admin_token, {"Range": "bytes=-10"})
    assert suffix.status_code == 206
    assert suffix.content == data[-10:]

    unsatisfiable = _get(server_url, path, root_admin_token, {"Range": f"bytes={len(data)}-"})
    assert unsatisfiable.status_code == 416
    assert unsatisfiable.headers["Content-Range"] == f"bytes */{len(data)}"

    head = requests.head(
        f"{server_url}{path}",
        headers={"Authorization": f"Bearer {root_admin_token}"},
        timeout=CONTENT_TIMEOUT_S,
    )
    assert head.status_code == 200
    assert head.content == b""
    assert head.headers["Content-Length"] == str(len(data))

    unchanged = _get(server_url, path, root_admin_token, {"If-None-Match": etag})
    assert unchanged.status_code == 304
    assert unchanged.content == b""

    stale = _get(
        server_url,
        path,
        root_admin_token,
        {"Range": "bytes=0-9", "If-Range": '"different"'},
    )
    assert stale.status_code == 200
    assert stale.content == data

    matching = _get(
        server_url,
        path,
        root_admin_token,
        {"Range": "bytes=0-9", "If-Range": etag},
    )
    assert matching.status_code == 206

    credential = _content_path(representation)
    without_header = _get(server_url, credential)
    assert without_header.status_code == 200
    assert without_header.content == data

    # A credential minted for this file does not address another one.
    other_data = png(16, 16)
    other_id = _upload(upload_media, register_media, root_admin_token, "uc031-other.png", other_data, "image/png")
    other_credential = _content_path(_file_representation(server_url, root_admin_token, other_id))
    # The real check: present the other file's credential for this file's route.
    other_query = other_credential.split("?", 1)[1]
    mismatch = _get(server_url, f"{ASSET_FILE_PATH}/{file_id}/content?{other_query}")
    assert mismatch.status_code == 401


def test_a_removed_file_is_reconciled_to_not_found(
    server_url: str,
    root_admin_token: str,
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    media_dir: Path,
) -> None:
    """Removing the bytes from the host root turns the content into `404`."""
    data = png(8, 8)
    file_id = _upload(upload_media, register_media, root_admin_token, "uc031-gone.png", data, "image/png")
    representation = _file_representation(server_url, root_admin_token, file_id)
    host_path = media_dir / representation["path"]
    assert host_path.is_file(), f"the registered file must exist on the host: {host_path}"
    host_path.unlink()

    deadline = time.monotonic() + RECONCILE_TIMEOUT_S
    while time.monotonic() < deadline:
        response = _get(
            server_url,
            f"{ASSET_FILE_PATH}/{file_id}/content",
            root_admin_token,
        )
        if response.status_code == 404:
            return
        time.sleep(_POLL_INTERVAL_S)
    pytest.fail(f"the removed file was not reconciled within {RECONCILE_TIMEOUT_S}s")
