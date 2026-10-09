"""E2E tests for UC-008 - Browse Galleries.

The happy path and every alternative flow run against the running container:
listing galleries and the seeded default, fetching a gallery with its ordered
items, fetching one item's media reference, the public/private visibility rules,
the Root-Admin-only private override, and the unknown-gallery/unknown-item
rejections.

Media registration has no route of its own (design § 8): completing an upload
registers the image as a side effect, and the completed upload's finalized file
identifier is the medium identifier the tests add to a gallery.
"""

import struct
import time
import zlib
from collections.abc import Callable

import requests

_API_PREFIX = "/api/v1"
_GALLERY_PATH = f"{_API_PREFIX}/library/gallery"

# The seeded, undeletable default gallery.
_DEFAULT_GALLERY_ID = 0


def _png(width: int, height: int) -> bytes:
    """Build a minimal valid RGB PNG of the given dimensions."""
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    scanline = b"\x00" + b"\xff\x00\x00" * width
    pixels = zlib.compress(scanline * height)
    return b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", header) + _chunk(b"IDAT", pixels) + _chunk(b"IEND", b"")


def _chunk(chunk_type: bytes, data: bytes) -> bytes:
    """Build one PNG chunk with its CRC."""
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


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


def _list_galleries(
    server_url: str,
    token: str,
    query: str = "",
) -> requests.Response:
    """GET the gallery collection with the given query string."""
    return requests.get(
        f"{server_url}{_GALLERY_PATH}{query}",
        headers={"Authorization": f"Bearer {token}"},
        timeout=10,
    )


def test_an_authenticated_user_browses_public_galleries(
    server_url: str,
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
    me: Callable[[str], requests.Response],
) -> None:
    name = f"Public {time.time_ns()}"
    created = create_gallery(standard_user_token, name, is_public=True)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    gallery_id = created.json()["id"]
    owner_id = me(standard_user_token).json()["id"]

    response = _list_galleries(
        server_url,
        second_standard_user_token,
        f"?is_public=true&owner_id={owner_id}",
    )

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert set(body.keys()) == {"items", "_links"}
    found = [item for item in body["items"] if item["id"] == gallery_id]
    assert len(found) == 1, f"the public gallery is missing from the listing: {body['items']}"
    assert found[0]["name"] == name
    assert found[0]["is_public"] is True
    assert found[0]["item_count"] == 0
    assert found[0]["_links"]["self"]["href"] == f"{_GALLERY_PATH}/{gallery_id}"


def test_the_default_gallery_is_visible_to_every_user(
    server_url: str,
    standard_user_token: str,
    second_standard_user_token: str,
    admin_token: str,
    root_admin_token: str,
) -> None:
    for token in (
        standard_user_token,
        second_standard_user_token,
        admin_token,
        root_admin_token,
    ):
        response = requests.get(
            f"{server_url}{_GALLERY_PATH}/{_DEFAULT_GALLERY_ID}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=10,
        )
        assert response.status_code == 200, f"default gallery not readable: {response.status_code} {response.text}"
        body = response.json()
        assert body["id"] == _DEFAULT_GALLERY_ID
        assert body["name"] == "Default"
        assert body["is_public"] is True
        assert body["owner_id"] is None

    listed = _list_galleries(server_url, standard_user_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert any(item["id"] == _DEFAULT_GALLERY_ID for item in listed.json()["items"]), (
        "the default gallery must appear in the listing"
    )


def test_get_gallery_returns_its_ordered_items(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
) -> None:
    created = create_gallery(standard_user_token, f"Ordered {time.time_ns()}", is_public=True)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    gallery_id = created.json()["id"]

    first_name = f"first-{time.time_ns()}.png"
    second_name = f"second-{time.time_ns()}.png"
    first_file = _register_image(upload_media, register_media, standard_user_token, first_name, _png(2, 2))
    second_file = _register_image(upload_media, register_media, standard_user_token, second_name, _png(3, 3))

    first_added = add_gallery_item(standard_user_token, gallery_id, "image", first_file)
    assert first_added.status_code == 201, f"add failed: {first_added.status_code} {first_added.text}"
    second_added = add_gallery_item(standard_user_token, gallery_id, "image", second_file)
    assert second_added.status_code == 201, f"add failed: {second_added.status_code} {second_added.text}"

    response = get_gallery(standard_user_token, gallery_id)

    assert response.status_code == 200, f"get failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["item_count"] == 2
    names = [item["name"] for item in body["items"]]
    assert names == [first_name, second_name], f"items are not in insertion order: {names}"
    file_ids = [item["file_id"] for item in body["items"]]
    assert file_ids == [first_file, second_file]


def test_get_gallery_item_returns_the_media_metadata(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
) -> None:
    created = create_gallery(standard_user_token, f"Item {time.time_ns()}", is_public=True)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    gallery_id = created.json()["id"]
    file_name = f"sunset-{time.time_ns()}.png"
    file_id = _register_image(upload_media, register_media, standard_user_token, file_name, _png(4, 5))
    added = add_gallery_item(standard_user_token, gallery_id, "image", file_id)
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    item_id = added.json()["id"]

    response = get_gallery_item(standard_user_token, gallery_id, item_id)

    assert response.status_code == 200, f"get failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert body["id"] == item_id
    assert body["type"] == "image"
    assert isinstance(body["media_id"], int) and body["media_id"] > 0
    assert body["file_id"] == file_id
    assert body["name"] == file_name
    assert isinstance(body["added_at"], str) and body["added_at"]
    assert body["_links"]["self"]["href"] == f"{_GALLERY_PATH}/{gallery_id}/item/{item_id}"


def test_a_private_gallery_is_hidden_from_another_user(
    server_url: str,
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
    me: Callable[[str], requests.Response],
) -> None:
    name = f"Secret {time.time_ns()}"
    created = create_gallery(standard_user_token, name)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    gallery_id = created.json()["id"]
    owner_id = me(standard_user_token).json()["id"]

    hidden = _list_galleries(server_url, second_standard_user_token, f"?owner_id={owner_id}")
    assert hidden.status_code == 200, f"list failed: {hidden.status_code} {hidden.text}"
    assert all(item["id"] != gallery_id for item in hidden.json()["items"]), (
        "another user's private gallery must not appear in the listing"
    )

    visible = _list_galleries(server_url, standard_user_token, f"?owner_id={owner_id}")
    assert visible.status_code == 200, f"list failed: {visible.status_code} {visible.text}"
    assert any(item["id"] == gallery_id for item in visible.json()["items"])


def test_reading_another_users_private_gallery_is_forbidden(
    create_gallery: Callable[[str, str, bool], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    created = create_gallery(standard_user_token, f"Secret {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    gallery_id = created.json()["id"]

    response = get_gallery(second_standard_user_token, gallery_id)

    assert_error_body(response, 403)


def test_an_admin_cannot_read_another_users_private_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    created = create_gallery(standard_user_token, f"Secret {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    gallery_id = created.json()["id"]

    response = get_gallery(admin_token, gallery_id)

    assert_error_body(response, 403)


def test_the_root_admin_can_read_any_private_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    root_admin_token: str,
) -> None:
    created = create_gallery(standard_user_token, f"Secret {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    gallery_id = created.json()["id"]

    response = get_gallery(root_admin_token, gallery_id)

    assert response.status_code == 200, f"get failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["id"] == gallery_id
    assert body["is_public"] is False


def test_reading_an_unknown_gallery_is_not_found(
    get_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_gallery(standard_user_token, 999_999_999)

    assert_error_body(response, 404)


def test_reading_an_unknown_item_is_not_found(
    create_gallery: Callable[[str, str, bool], requests.Response],
    get_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    created = create_gallery(standard_user_token, f"Empty {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    gallery_id = created.json()["id"]

    response = get_gallery_item(standard_user_token, gallery_id, 999_999_999)

    assert_error_body(response, 404)
