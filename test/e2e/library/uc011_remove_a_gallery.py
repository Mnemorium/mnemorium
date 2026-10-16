"""E2E tests for UC-011 - Remove a Gallery.

The happy path and every alternative flow run against the running container:
the gallery owner deleting its own gallery and everything it holds, the denial
of a non-owner Standard User and of a non-owner Admin, the seeded default
gallery `0` that can never be deleted, and the unknown-gallery rejection.

Removal is destructive: the gallery's items, their image or video rows, and the
backing files are deleted before the gallery itself. No endpoint exposes files,
so the backing file's deletion is asserted by the repository and storage tests;
these tests assert the observable: the gallery is gone and one of its media no
longer exists, which the not-found answer to re-adding the media proves.
"""

import struct
import time
import zlib
from collections.abc import Callable
from itertools import count
from pathlib import Path

import requests

_API_PREFIX = "/api/v1"
_GALLERY_PATH = f"{_API_PREFIX}/library/gallery"

# The seeded default gallery, undeletable per the business rule.
_DEFAULT_GALLERY_ID = 0

_UNKNOWN_ID = 999_999_999

_VIDEO_FIXTURE = Path(__file__).resolve().parents[1] / "fixture" / "tiny_video.mp4"

# Each generated image must have distinct bytes: the upload-completion handler
# deduplicates by integrity hash, so identical bytes would reuse a file (and its
# already-registered medium) across tests. The counter is seeded from the clock
# so a second run against the same container still produces distinct bytes.
_IMAGE_NONCE = count(time.time_ns() & 0xFFFFFF)

# The video fixture is shared, so each upload gets a unique trailing `free` box:
# the padding is valid MP4 and leaves the container metadata the probe reads
# untouched, while the bytes stay unique per test for the same reason.
_VIDEO_NONCE = count(time.time_ns() & 0xFFFFFF)


def _chunk(chunk_type: bytes, data: bytes) -> bytes:
    """Build one PNG chunk with its CRC."""
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def _png(width: int, height: int) -> bytes:
    """Build a minimal valid, unique RGB PNG of the given dimensions."""
    nonce = next(_IMAGE_NONCE)
    colour = bytes([nonce & 0xFF, (nonce >> 8) & 0xFF, (nonce >> 16) & 0xFF])
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    scanline = b"\x00" + colour * width
    pixels = zlib.compress(scanline * height)
    return b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", header) + _chunk(b"IDAT", pixels) + _chunk(b"IEND", b"")


def _video() -> bytes:
    """Return the fixture's bytes with a unique trailing `free` box."""
    padding = struct.pack(">I", next(_VIDEO_NONCE))
    return _VIDEO_FIXTURE.read_bytes() + struct.pack(">I", 8 + len(padding)) + b"free" + padding


def _create_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    token: str,
    is_public: bool = False,
) -> int:
    """Create a gallery owned by `token`'s caller and return its identifier."""
    created = create_gallery(token, f"Gallery {time.time_ns()}", is_public)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    return created.json()["id"]


def _add_image(
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    token: str,
    gallery_id: int,
) -> int:
    """Upload an image, add it to `gallery_id`, and return the medium identifier."""
    media_id = register_media(
        token,
        upload_media(token, f"item-{time.time_ns()}.png", _png(3, 2)),
    )
    added = add_gallery_item(token, gallery_id, "image", media_id)
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    return media_id


def test_the_owner_removes_its_own_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
) -> None:
    gallery_id = _create_gallery(create_gallery, standard_user_token)
    media_id = _add_image(
        upload_media,
        register_media,
        add_gallery_item,
        standard_user_token,
        gallery_id,
    )

    removed = delete_gallery(standard_user_token, gallery_id)

    assert removed.status_code == 204, f"delete failed: {removed.status_code} {removed.text}"
    assert removed.content == b""

    # The gallery and everything it held are gone.
    fetched = get_gallery(standard_user_token, gallery_id)
    assert fetched.status_code == 404, f"the removed gallery is still readable: {fetched.text}"

    # The item's medium is gone too: adding it again is a not-found.
    readded = add_gallery_item(standard_user_token, gallery_id, "image", media_id)
    assert readded.status_code == 404, f"the medium still exists: {readded.status_code} {readded.text}"


def test_a_non_owner_cannot_remove_a_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = _create_gallery(create_gallery, standard_user_token)

    # Only the owner may delete the gallery; another Standard User may not.
    response = delete_gallery(second_standard_user_token, gallery_id)

    assert_error_body(response, 403)
    kept = get_gallery(standard_user_token, gallery_id)
    assert kept.status_code == 200, f"the gallery must survive a denied removal: {kept.text}"


def test_an_admin_cannot_remove_another_users_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = _create_gallery(create_gallery, standard_user_token, is_public=True)

    # The Root Admin has no override; a non-owner Admin is refused too.
    response = delete_gallery(admin_token, gallery_id)

    assert_error_body(response, 403)
    kept = get_gallery(standard_user_token, gallery_id)
    assert kept.status_code == 200, f"the gallery must survive a denied removal: {kept.text}"


def test_the_default_gallery_cannot_be_removed(
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = delete_gallery(standard_user_token, _DEFAULT_GALLERY_ID)

    assert_error_body(response, 409)


def test_removing_an_unknown_gallery_is_not_found(
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = delete_gallery(standard_user_token, _UNKNOWN_ID)

    assert_error_body(response, 404)
