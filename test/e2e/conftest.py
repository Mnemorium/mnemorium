import base64
import hashlib
import os
import re
import struct
import subprocess
import time
import zlib
from collections.abc import Callable
from itertools import count
from pathlib import Path

import pytest
import requests

_BASE_URL = "http://127.0.0.1:4080"
_BASE_URL_ENV = "MNEMORIUM_E2E_BASE_URL"
_CONTAINER = "mnemorium-e2e"
_HEALTH_PATH = "/health"
_HEALTH_TIMEOUT_S = 60
_POLL_INTERVAL_S = 0.5

_DEFAULT_PASSWORD_RE = re.compile(
    r"Root admin initialized; use the default password to authenticate and change it: '([^']+)'"
)

API_PREFIX = "/api/v1"
LOGIN_PATH = f"{API_PREFIX}/identity/login"
REGISTER_PATH = f"{API_PREFIX}/identity/register"
ME_PATH = f"{API_PREFIX}/user/me"

LIBRARY_GALLERY_PATH = f"{API_PREFIX}/library/gallery"
LIBRARY_ALBUM_PATH = f"{API_PREFIX}/library/music/album"
LIBRARY_MOVIE_PATH = f"{API_PREFIX}/library/movie"
LIBRARY_SERIES_PATH = f"{API_PREFIX}/library/series"
UPLOAD_PATH = f"{API_PREFIX}/asset/upload"

ROOT_ADMIN_USERNAME = "root"

# The one secret password every generic test uses for users it provisions or
# registers (alice, mallory, ad-hoc users). Tests whose subject is password
# behavior (policy validation, change-password, wrong-password login) pick
# their own explicit passwords instead.
SECRET_PASSWORD = "C0rrect!Horse"

STANDARD_USER_USERNAME = "alice"
STANDARD_USER_EMAIL = "alice@example.com"
STANDARD_USER_PASSWORD = SECRET_PASSWORD
STANDARD_USER_ROLE = "STANDARD"

# A second Standard User, for cross-user visibility and ownership cases.
SECOND_STANDARD_USER_USERNAME = "bobby"
SECOND_STANDARD_USER_EMAIL = "bobby@example.com"
SECOND_STANDARD_USER_PASSWORD = SECRET_PASSWORD
SECOND_STANDARD_USER_ROLE = "STANDARD"

ADMIN_USER_USERNAME = "mallory"
ADMIN_USER_EMAIL = "mallory@example.com"
ADMIN_USER_PASSWORD = SECRET_PASSWORD
ADMIN_USER_ROLE = "ADMIN"

# The seeded default gallery: undeletable, and visible to every user.
DEFAULT_GALLERY_ID = 0

# An identifier no gallery, item or medium ever carries.
UNKNOWN_ID = 999_999_999

# The tiny, real H.264 fixture the container's `ffprobe` can probe (TEST-052).
_VIDEO_FIXTURE = Path(__file__).resolve().parent / "fixture" / "tiny_video.mp4"

# Each generated medium must have distinct bytes: the upload-completion handler
# deduplicates by integrity hash, so identical bytes would reuse a file (and its
# already-registered medium) across tests. The counters are seeded from the clock
# so a second run against the same container still produces distinct bytes.
_IMAGE_NONCE = count(time.time_ns() & 0xFFFFFF)
_VIDEO_NONCE = count(time.time_ns() & 0xFFFFFF)


def _png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    """Build one PNG chunk with its CRC."""
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def png(width: int, height: int) -> bytes:
    """Build a minimal valid, unique RGB PNG of the given dimensions."""
    nonce = next(_IMAGE_NONCE)
    colour = bytes([nonce & 0xFF, (nonce >> 8) & 0xFF, (nonce >> 16) & 0xFF])
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    scanline = b"\x00" + colour * width
    pixels = zlib.compress(scanline * height)
    return b"\x89PNG\r\n\x1a\n" + _png_chunk(b"IHDR", header) + _png_chunk(b"IDAT", pixels) + _png_chunk(b"IEND", b"")


def video() -> bytes:
    """Return the fixture's bytes with a unique trailing `free` box.

    The padding is valid MP4 and leaves the container metadata the probe reads
    untouched, while the bytes stay unique per upload (see `_VIDEO_NONCE`).
    """
    padding = struct.pack(">I", next(_VIDEO_NONCE))
    return _VIDEO_FIXTURE.read_bytes() + struct.pack(">I", 8 + len(padding)) + b"free" + padding


def register_image(
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    token: str,
    file_name: str,
    data: bytes,
) -> int:
    """Stage and register an image, returning its medium identifier."""
    upload_id = upload_media(token, file_name, data)
    return register_media(token, upload_id)


def create_owned_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    token: str,
    is_public: bool = False,
) -> int:
    """Create a gallery owned by `token`'s caller and return its identifier."""
    created = create_gallery(token, f"Gallery {time.time_ns()}", is_public)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    return created.json()["id"]


def add_item(
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    token: str,
    gallery_id: int,
    media_type: str,
    media_id: int,
) -> int:
    """Add `media_id` to the gallery and return the created item's identifier."""
    added = add_gallery_item(token, gallery_id, media_type, media_id)
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    return added.json()["id"]


@pytest.fixture(scope="session")
def secret_password() -> str:
    """The shared secret password for tests that just need a valid password."""
    return SECRET_PASSWORD


@pytest.fixture(scope="session")
def server_url() -> str:
    """Return the base URL of a running server, once it is healthy."""
    base_url = os.environ.get(_BASE_URL_ENV, _BASE_URL)

    deadline = time.monotonic() + _HEALTH_TIMEOUT_S
    while time.monotonic() < deadline:
        try:
            response = requests.get(f"{base_url}{_HEALTH_PATH}", timeout=1)
            if response.ok:
                return base_url
        except requests.ConnectionError:
            pass
        time.sleep(_POLL_INTERVAL_S)
    raise RuntimeError(
        f"server not healthy within {_HEALTH_TIMEOUT_S}s at {base_url}; start it first, e.g. with `devenv server`"
    )


@pytest.fixture(scope="session")
def default_password() -> str:
    """Return the root admin default password printed to the container stdout."""
    result = subprocess.run(
        ["docker", "logs", _CONTAINER],
        capture_output=True,
        text=True,
        check=True,
    )
    match = _DEFAULT_PASSWORD_RE.search(result.stdout)
    if match is None:
        raise RuntimeError("root admin default password not found in container stdout")
    return match.group(1)


@pytest.fixture(scope="session")
def assert_error_body() -> Callable[[requests.Response, int], None]:
    """Return a callable that asserts the exact ErrorBody shape declared in the OpenAPI spec."""

    def _assert_error_body(response: requests.Response, status_code: int) -> None:
        assert response.status_code == status_code, (
            f"expected {status_code}, got {response.status_code}: {response.text}"
        )
        assert response.headers.get("Content-Type", "").startswith("application/hal+json")
        body = response.json()
        assert set(body.keys()) == {"error", "_links"}
        assert isinstance(body["error"], str) and body["error"]
        links = body["_links"]
        assert set(links.keys()) == {"self"}
        assert isinstance(links["self"].get("href"), str) and links["self"]["href"]

    return _assert_error_body


@pytest.fixture(scope="session")
def login(server_url: str) -> Callable[[str, str], str]:
    """Return a callable that authenticates and returns the access token."""

    def _login(username: str, password: str) -> str:
        response = requests.post(
            f"{server_url}{LOGIN_PATH}",
            json={"username": username, "password": password},
            timeout=10,
        )
        assert response.status_code == 200, f"login failed: {response.status_code} {response.text}"
        body = response.json()
        assert body["token_type"] == "bearer"
        assert isinstance(body["access_token"], str) and body["access_token"]
        assert isinstance(body["expires_in"], int) and body["expires_in"] >= 0
        return body["access_token"]

    return _login


@pytest.fixture(scope="session")
def register(
    server_url: str,
) -> Callable[[str, str, str | None, str, str], requests.Response]:
    """Return a callable that registers a user, without asserting the status."""

    def _register(token: str, username: str, email: str | None, password: str, role: str) -> requests.Response:
        return requests.post(
            f"{server_url}{REGISTER_PATH}",
            headers={"Authorization": f"Bearer {token}"},
            json={"username": username, "email": email, "password": password, "role": role},
            timeout=10,
        )

    return _register


@pytest.fixture(scope="session")
def me(server_url: str) -> Callable[[str], requests.Response]:
    """Return a callable that GETs /api/v1/user/me with the given access token."""

    def _me(token: str) -> requests.Response:
        return requests.get(
            f"{server_url}{ME_PATH}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _me


def _provision_user(
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
    token: str,
    username: str,
    email: str | None,
    password: str,
    role: str,
) -> str:
    """Provision a user, tolerating 409 from an earlier run, and return its token."""
    response = register(token, username, email, password, role)
    if response.status_code == 201:
        assert response.json()["role"] == role
    elif response.status_code != 409:
        pytest.fail(f"provisioning user {username!r} failed: {response.status_code} {response.text}")
    return login(username, password)


@pytest.fixture
def root_admin_token(server_url: str, default_password: str, login: Callable[[str, str], str]) -> str:
    """Access token of the Root Admin (username "root", default password)."""
    return login(ROOT_ADMIN_USERNAME, default_password)


@pytest.fixture
def admin_token(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
) -> str:
    """Access token of the non-root Admin "mallory", provisioned on first use."""
    return _provision_user(
        register,
        login,
        root_admin_token,
        ADMIN_USER_USERNAME,
        ADMIN_USER_EMAIL,
        ADMIN_USER_PASSWORD,
        ADMIN_USER_ROLE,
    )


@pytest.fixture
def standard_user_token(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
) -> str:
    """Access token of the Standard User "alice", provisioned on first use."""
    return _provision_user(
        register,
        login,
        root_admin_token,
        STANDARD_USER_USERNAME,
        STANDARD_USER_EMAIL,
        STANDARD_USER_PASSWORD,
        STANDARD_USER_ROLE,
    )


@pytest.fixture
def second_standard_user_token(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
) -> str:
    """Access token of the Standard User "bob", provisioned on first use."""
    return _provision_user(
        register,
        login,
        root_admin_token,
        SECOND_STANDARD_USER_USERNAME,
        SECOND_STANDARD_USER_EMAIL,
        SECOND_STANDARD_USER_PASSWORD,
        SECOND_STANDARD_USER_ROLE,
    )


@pytest.fixture
def upload_media(server_url: str) -> Callable[[str, str, bytes, str], int]:
    """Return a callable that stages media as one upload chunk.

    The callable begins a chunked upload for `file_name` and stores `data` as
    its only chunk, returning the upload session identifier. Completing the
    session with [`register_media`] is what registers the medium.
    """

    def _upload_media(
        token: str,
        file_name: str,
        data: bytes,
        content_type: str = "image/png",
    ) -> int:
        authorization = {"Authorization": f"Bearer {token}"}
        digest = hashlib.sha256(data).digest()
        begin = requests.post(
            f"{server_url}{UPLOAD_PATH}",
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
            f"{server_url}{UPLOAD_PATH}/{upload_id}/chunk/0",
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
        return upload_id

    return _upload_media


@pytest.fixture
def register_media(server_url: str) -> Callable[[str, int], int]:
    """Return a callable that completes an upload session, registering its medium.

    Media registration has no route of its own: the upload-completion handler
    registers the image or video row as a side effect (design § 8). The callable
    returns the identifier of the finalized backing file.
    """

    def _register_media(token: str, upload_id: int) -> int:
        response = requests.post(
            f"{server_url}{UPLOAD_PATH}/{upload_id}/complete",
            headers={"Authorization": f"Bearer {token}"},
            timeout=30,
        )
        assert response.status_code == 200, f"complete failed: {response.status_code} {response.text}"
        return int(response.json()["file_id"])

    return _register_media


@pytest.fixture
def create_gallery(server_url: str) -> Callable[[str, str, bool], requests.Response]:
    """Return a callable that POSTs a gallery creation request."""

    def _create_gallery(token: str, name: str, is_public: bool = False) -> requests.Response:
        return requests.post(
            f"{server_url}{LIBRARY_GALLERY_PATH}",
            headers={"Authorization": f"Bearer {token}"},
            json={"name": name, "is_public": is_public},
            timeout=10,
        )

    return _create_gallery


@pytest.fixture
def add_gallery_item(server_url: str) -> Callable[[str, int, str, int], requests.Response]:
    """Return a callable that POSTs one of the caller's media to a gallery."""

    def _add_gallery_item(
        token: str,
        gallery_id: int,
        media_type: str,
        media_id: int,
    ) -> requests.Response:
        return requests.post(
            f"{server_url}{LIBRARY_GALLERY_PATH}/{gallery_id}/item",
            headers={"Authorization": f"Bearer {token}"},
            json={"type": media_type, "media_id": media_id},
            timeout=10,
        )

    return _add_gallery_item


@pytest.fixture
def get_gallery(server_url: str) -> Callable[[str, int], requests.Response]:
    """Return a callable that GETs a gallery and its items by identifier."""

    def _get_gallery(token: str, gallery_id: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_GALLERY_PATH}/{gallery_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_gallery


@pytest.fixture
def delete_gallery(server_url: str) -> Callable[[str, int], requests.Response]:
    """Return a callable that DELETEs a gallery by identifier."""

    def _delete_gallery(token: str, gallery_id: int) -> requests.Response:
        return requests.delete(
            f"{server_url}{LIBRARY_GALLERY_PATH}/{gallery_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _delete_gallery


@pytest.fixture
def get_gallery_item(server_url: str) -> Callable[[str, int, int], requests.Response]:
    """Return a callable that GETs one item of a gallery."""

    def _get_gallery_item(token: str, gallery_id: int, item_id: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_GALLERY_PATH}/{gallery_id}/item/{item_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_gallery_item


@pytest.fixture
def delete_gallery_item(server_url: str) -> Callable[[str, int, int], requests.Response]:
    """Return a callable that DELETEs one item of a gallery."""

    def _delete_gallery_item(token: str, gallery_id: int, item_id: int) -> requests.Response:
        return requests.delete(
            f"{server_url}{LIBRARY_GALLERY_PATH}/{gallery_id}/item/{item_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _delete_gallery_item


@pytest.fixture
def list_albums(server_url: str) -> Callable[[str, str], requests.Response]:
    """Return a callable that GETs the music album collection with a query string."""

    def _list_albums(token: str, query: str = "") -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_ALBUM_PATH}{query}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _list_albums


@pytest.fixture
def get_album(server_url: str) -> Callable[[str, int], requests.Response]:
    """Return a callable that GETs one music album by its identifier."""

    def _get_album(token: str, album_id: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_ALBUM_PATH}/{album_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_album


@pytest.fixture
def get_album_medium(server_url: str) -> Callable[[str, int, int], requests.Response]:
    """Return a callable that GETs one medium of a music album by its indexes."""

    def _get_album_medium(token: str, album_id: int, medium_index: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_ALBUM_PATH}/{album_id}/medium/{medium_index}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_album_medium


@pytest.fixture
def list_album_tracks(server_url: str) -> Callable[[str, int, int], requests.Response]:
    """Return a callable that GETs the tracks of one medium of a music album."""

    def _list_album_tracks(token: str, album_id: int, medium_index: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_ALBUM_PATH}/{album_id}/medium/{medium_index}/track",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _list_album_tracks


@pytest.fixture
def get_album_track(server_url: str) -> Callable[[str, int, int, int], requests.Response]:
    """Return a callable that GETs one track of a music album medium."""

    def _get_album_track(token: str, album_id: int, medium_index: int, track_index: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_ALBUM_PATH}/{album_id}/medium/{medium_index}/track/{track_index}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_album_track


@pytest.fixture
def list_movies(server_url: str) -> Callable[[str, str], requests.Response]:
    """Return a callable that GETs the film collection with a query string."""

    def _list_movies(token: str, query: str = "") -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_MOVIE_PATH}{query}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _list_movies


@pytest.fixture
def get_movie(server_url: str) -> Callable[[str, int], requests.Response]:
    """Return a callable that GETs one film by its identifier."""

    def _get_movie(token: str, movie_id: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_MOVIE_PATH}/{movie_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_movie


@pytest.fixture
def list_series(server_url: str) -> Callable[[str, str], requests.Response]:
    """Return a callable that GETs the show collection with a query string."""

    def _list_series(token: str, query: str = "") -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_SERIES_PATH}{query}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _list_series


@pytest.fixture
def get_series(server_url: str) -> Callable[[str, int], requests.Response]:
    """Return a callable that GETs one show by its identifier."""

    def _get_series(token: str, series_id: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_SERIES_PATH}/{series_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_series


@pytest.fixture
def get_series_season(server_url: str) -> Callable[[str, int, int], requests.Response]:
    """Return a callable that GETs one season of a show."""

    def _get_series_season(token: str, series_id: int, season_index: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_SERIES_PATH}/{series_id}/season/{season_index}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_series_season


@pytest.fixture
def list_series_episodes(server_url: str) -> Callable[[str, int, int], requests.Response]:
    """Return a callable that GETs the episodes of one season of a show."""

    def _list_series_episodes(token: str, series_id: int, season_index: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_SERIES_PATH}/{series_id}/season/{season_index}/episode",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _list_series_episodes


@pytest.fixture
def get_series_episode(server_url: str) -> Callable[[str, int, int, int], requests.Response]:
    """Return a callable that GETs one episode of a show season."""

    def _get_series_episode(token: str, series_id: int, season_index: int, episode_index: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_SERIES_PATH}/{series_id}/season/{season_index}/episode/{episode_index}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_series_episode


LIBRARY_PLAYLIST_PATH = f"{API_PREFIX}/library/music/playlist"


@pytest.fixture
def create_playlist(server_url: str) -> Callable[[str, str, bool], requests.Response]:
    """Return a callable that POSTs a playlist creation request."""

    def _create_playlist(token: str, name: str, is_public: bool = False) -> requests.Response:
        return requests.post(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}",
            headers={"Authorization": f"Bearer {token}"},
            json={"name": name, "is_public": is_public},
            timeout=10,
        )

    return _create_playlist


@pytest.fixture
def list_playlists(server_url: str) -> Callable[[str, str], requests.Response]:
    """Return a callable that GETs the playlist collection with a query string."""

    def _list_playlists(token: str, query: str = "") -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}{query}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _list_playlists


@pytest.fixture
def get_playlist(server_url: str) -> Callable[[str, int], requests.Response]:
    """Return a callable that GETs one playlist by identifier."""

    def _get_playlist(token: str, playlist_id: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}/{playlist_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_playlist


@pytest.fixture
def patch_playlist(server_url: str) -> Callable[[str, int, dict], requests.Response]:
    """Return a callable that PATCHes one playlist with a JSON payload."""

    def _patch_playlist(token: str, playlist_id: int, payload: dict) -> requests.Response:
        return requests.patch(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}/{playlist_id}",
            headers={"Authorization": f"Bearer {token}"},
            json=payload,
            timeout=10,
        )

    return _patch_playlist


@pytest.fixture
def delete_playlist(server_url: str) -> Callable[[str, int], requests.Response]:
    """Return a callable that DELETEs one playlist by identifier."""

    def _delete_playlist(token: str, playlist_id: int) -> requests.Response:
        return requests.delete(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}/{playlist_id}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _delete_playlist


@pytest.fixture
def get_playlist_track_list(server_url: str) -> Callable[[str, int], requests.Response]:
    """Return a callable that GETs the playlist track alias."""

    def _get_playlist_track_list(token: str, playlist_id: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}/{playlist_id}/track",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_playlist_track_list


@pytest.fixture
def get_playlist_track(server_url: str) -> Callable[[str, int, int], requests.Response]:
    """Return a callable that GETs one track of a playlist."""

    def _get_playlist_track(token: str, playlist_id: int, track_index: int) -> requests.Response:
        return requests.get(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}/{playlist_id}/track/{track_index}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _get_playlist_track


@pytest.fixture
def delete_playlist_track(server_url: str) -> Callable[[str, int, int], requests.Response]:
    """Return a callable that DELETEs one track of a playlist."""

    def _delete_playlist_track(token: str, playlist_id: int, track_index: int) -> requests.Response:
        return requests.delete(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}/{playlist_id}/track/{track_index}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )

    return _delete_playlist_track


@pytest.fixture
def reorder_playlist_tracks(server_url: str) -> Callable[[str, int, list[int]], requests.Response]:
    """Return a callable that PUTs a playlist track order."""

    def _reorder_playlist_tracks(token: str, playlist_id: int, order: list[int]) -> requests.Response:
        return requests.put(
            f"{server_url}{LIBRARY_PLAYLIST_PATH}/{playlist_id}/track",
            headers={"Authorization": f"Bearer {token}"},
            json={"order": order},
            timeout=10,
        )

    return _reorder_playlist_tracks


def create_owned_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    token: str,
    is_public: bool = False,
) -> int:
    """Create a playlist owned by `token`'s caller and return its identifier."""
    created = create_playlist(token, f"Playlist {time.time_ns()}", is_public)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    return created.json()["id"]
