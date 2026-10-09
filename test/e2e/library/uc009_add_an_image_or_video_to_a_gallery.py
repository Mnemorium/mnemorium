"""E2E tests for UC-009 - Add an Image or Video to a Gallery.

The happy path and every alternative flow run against the running container:
adding an owned image and an owned video to a public gallery, the
public/private contribution rules, the own-media rule for a Standard User, an
Admin and the Root Admin, and the unknown-gallery, unknown-media,
already-assigned and invalid-media-type rejections.

Media registration has no route of its own (design § 8): completing an upload
registers the image or video as a side effect. The video fixture is a real,
tiny H.264 file the container's `ffprobe` can probe.
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

_UNKNOWN_ID = 999_999_999

_VIDEO_FIXTURE = Path(__file__).resolve().parents[1] / "fixture" / "tiny_video.mp4"

# Each generated image must have distinct bytes: the upload-completion handler
# deduplicates by integrity hash, so identical bytes would reuse a file (and its
# already-registered medium) across tests.
_IMAGE_NONCE = count(1)


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
    """Return the bytes of the bundled, real video fixture."""
    return _VIDEO_FIXTURE.read_bytes()


def _register_image(
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    token: str,
    file_name: str,
    data: bytes,
) -> int:
    """Stage and register an image, returning its medium identifier."""
    upload_id = upload_media(token, file_name, data)
    return register_media(token, upload_id)


def _create_public_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    token: str,
) -> int:
    """Create a public gallery owned by `token`'s caller and return its id."""
    created = create_gallery(token, f"Gallery {time.time_ns()}", is_public=True)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    return created.json()["id"]


def test_adding_an_owned_image_to_a_public_gallery_appends_it(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
) -> None:
    gallery_id = _create_public_gallery(create_gallery, standard_user_token)
    file_name = f"sunset-{time.time_ns()}.png"
    file_id = _register_image(upload_media, register_media, standard_user_token, file_name, _png(4, 3))

    added = add_gallery_item(standard_user_token, gallery_id, "image", file_id)

    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    assert added.headers.get("Content-Type", "").startswith("application/hal+json")
    body = added.json()
    assert isinstance(body["id"], int) and body["id"] > 0
    assert body["type"] == "image"
    assert isinstance(body["media_id"], int) and body["media_id"] > 0
    assert body["file_id"] == file_id
    assert body["name"] == file_name
    assert isinstance(body["added_at"], str) and body["added_at"]
    assert body["_links"]["self"]["href"] == f"{_GALLERY_PATH}/{gallery_id}/item/{body['id']}"
    assert added.headers.get("Location") == body["_links"]["self"]["href"]

    stored = get_gallery(standard_user_token, gallery_id)
    assert stored.status_code == 200, f"get failed: {stored.status_code} {stored.text}"
    stored_body = stored.json()
    assert stored_body["item_count"] == 1
    assert [item["id"] for item in stored_body["items"]] == [body["id"]]
    assert stored_body["items"][0]["name"] == file_name


def test_adding_an_owned_video_to_a_gallery_appends_it(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
) -> None:
    gallery_id = _create_public_gallery(create_gallery, standard_user_token)
    file_id = register_media(
        standard_user_token,
        upload_media(standard_user_token, f"clip-{time.time_ns()}.mp4", _video(), "video/mp4"),
    )

    added = add_gallery_item(standard_user_token, gallery_id, "video", file_id)

    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    body = added.json()
    assert body["type"] == "video"
    assert body["file_id"] == file_id
    assert body["name"].endswith(".mp4")

    stored = get_gallery(standard_user_token, gallery_id)
    assert stored.status_code == 200, f"get failed: {stored.status_code} {stored.text}"
    assert stored.json()["item_count"] == 1
    assert stored.json()["items"][0]["type"] == "video"


def test_adding_to_another_users_private_gallery_is_forbidden(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    private = create_gallery(standard_user_token, f"Secret {time.time_ns()}")
    assert private.status_code == 201, f"create failed: {private.status_code} {private.text}"
    gallery_id = private.json()["id"]
    file_id = _register_image(
        upload_media,
        register_media,
        second_standard_user_token,
        f"own-{time.time_ns()}.png",
        _png(2, 2),
    )

    response = add_gallery_item(second_standard_user_token, gallery_id, "image", file_id)

    assert_error_body(response, 403)


def test_adding_media_owned_by_another_user_is_forbidden(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = _create_public_gallery(create_gallery, second_standard_user_token)
    file_id = _register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"alice-{time.time_ns()}.png",
        _png(2, 2),
    )

    response = add_gallery_item(second_standard_user_token, gallery_id, "image", file_id)

    assert_error_body(response, 403)


def test_an_admin_cannot_add_another_users_media(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    standard_user_token: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = _create_public_gallery(create_gallery, admin_token)
    file_id = _register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"alice-{time.time_ns()}.png",
        _png(2, 2),
    )

    response = add_gallery_item(admin_token, gallery_id, "image", file_id)

    assert_error_body(response, 403)


def test_the_root_admin_can_add_another_users_media(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    standard_user_token: str,
    root_admin_token: str,
) -> None:
    gallery_id = _create_public_gallery(create_gallery, standard_user_token)
    file_id = _register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"alice-{time.time_ns()}.png",
        _png(2, 2),
    )

    added = add_gallery_item(root_admin_token, gallery_id, "image", file_id)

    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    assert added.json()["file_id"] == file_id


def test_adding_an_unknown_media_is_not_found(
    create_gallery: Callable[[str, str, bool], requests.Response],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = _create_public_gallery(create_gallery, standard_user_token)

    response = add_gallery_item(standard_user_token, gallery_id, "image", _UNKNOWN_ID)

    assert_error_body(response, 404)


def test_adding_to_an_unknown_gallery_is_not_found(
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    file_id = _register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"nowhere-{time.time_ns()}.png",
        _png(2, 2),
    )

    response = add_gallery_item(standard_user_token, _UNKNOWN_ID, "image", file_id)

    assert_error_body(response, 404)


def test_adding_media_already_in_a_gallery_is_conflict(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = _create_public_gallery(create_gallery, standard_user_token)
    file_id = _register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"once-{time.time_ns()}.png",
        _png(2, 2),
    )
    first = add_gallery_item(standard_user_token, gallery_id, "image", file_id)
    assert first.status_code == 201, f"add failed: {first.status_code} {first.text}"

    response = add_gallery_item(standard_user_token, gallery_id, "image", file_id)

    assert_error_body(response, 409)


def test_adding_an_invalid_media_type_is_unprocessable(
    create_gallery: Callable[[str, str, bool], requests.Response],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = _create_public_gallery(create_gallery, standard_user_token)

    response = add_gallery_item(standard_user_token, gallery_id, "audio", 1)

    assert_error_body(response, 422)
