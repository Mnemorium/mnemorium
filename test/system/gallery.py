"""Black-box proof of the `gallery` capability.

A gallery is an owned, ordered collection of images and videos. This module proves what the collection and its items
report and the effects an operator can observe: creation and its refusals, the seeded default gallery, browse and its
filters, an ordered read, adding and removing one item at a time, the advanced last-modified time, and the
byte-preserving removal policy.

The declarative contract — the media types, the status of each operation, the representation's fields and its
`_links` — is validated by Schemathesis from the served document. These cases prove only what a schema cannot: which
gallery a caller may discover, that a refused change stores nothing, that items read back in position order, that the
removed medium is gone while its bytes survive, and that every change advances the last-modified time. Every route comes
from the API index (`Sut.href`), from a representation's named link, or from the response's own `_links.self`, so the
suite keeps its single `/api/v1` base literal in `harness.Sut.index`.
"""

import struct
import time
import zlib
from itertools import count

import allure
from conftest import FIXTURE_DIRECTORY, create_gallery, linked_id, provision, upload
from harness import Sut, link_of

IMAGE_CONTENT_TYPE = "image/png"
VIDEO_CONTENT_TYPE = "video/mp4"

# The seeded default gallery: ownerless, public and never deletable.
DEFAULT_GALLERY_ID = 0

# An identifier no gallery, item or medium ever carries.
UNKNOWN_ID = 999_999_999

# Each generated medium must carry distinct bytes: upload completion deduplicates
# by integrity hash, so identical bytes would reuse a file and its typed row.
_NONCE = count(time.time_ns() & 0xFFFFFF)


def _png(width: int, height: int) -> bytes:
    """Build a minimal valid, unique RGB PNG of the given dimensions."""
    nonce = next(_NONCE)
    colour = bytes([nonce & 0xFF, (nonce >> 8) & 0xFF, (nonce >> 16) & 0xFF])

    def chunk(chunk_type: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    scanline = b"\x00" + colour * width
    pixels = zlib.compress(scanline * height)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", pixels) + chunk(b"IEND", b"")


def _video() -> bytes:
    """Return the fixture's bytes with a unique trailing `free` box the probe ignores."""
    padding = struct.pack(">I", next(_NONCE))
    return (FIXTURE_DIRECTORY / "clip.mp4").read_bytes() + struct.pack(">I", 8 + len(padding)) + b"free" + padding


def image_file(server: Sut, token: str, name: str, dimensions: tuple[int, int] = (4, 3)) -> dict:
    """Upload one image and return its file representation."""
    width, height = dimensions
    return upload(server, token, name, _png(width, height), IMAGE_CONTENT_TYPE)


def video_file(server: Sut, token: str, name: str = "clip.mp4") -> dict:
    """Upload one video and return its file representation."""
    return upload(server, token, name, _video(), VIDEO_CONTENT_TYPE)


def create_gallery_response(server: Sut, token: str, name: str, is_public: bool = False):
    """POST a gallery creation and return the raw response."""
    return server.request(
        "POST",
        server.href("gallery"),
        token=token,
        json={"name": name, "is_public": is_public},
    )


def list_galleries(server: Sut, token: str, query: str = ""):
    """GET the gallery collection with an optional query string."""
    return server.request("GET", f"{server.href('gallery')}{query}", token=token)


def read_gallery(server: Sut, token: str, gallery_id: int):
    """GET one gallery by identifier."""
    return server.request("GET", f"{server.href('gallery')}/{gallery_id}", token=token)


def add_item(server: Sut, token: str, gallery: dict, media_type: str, file_id: int):
    """POST one medium to a gallery and return the raw response."""
    return server.request(
        "POST",
        link_of(gallery, "items"),
        token=token,
        json={"type": media_type, "media_id": file_id},
    )


def await_next_second() -> None:
    """Sleep just past a whole second so a second-precision timestamp advances."""
    time.sleep(1.05)


def gallery_ids(response) -> list[int]:
    """The identifiers a collection response lists."""
    return [item["id"] for item in response.json()["items"]]


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-001")
@allure.story("SCN-GALLERY-001-A")
@allure.feature("REQ-GALLERY-005")
@allure.story("SCN-GALLERY-005-A")
@allure.feature("REQ-GALLERY-007")
@allure.story("SCN-GALLERY-007-C")
@allure.feature("REQ-GALLERY-011")
@allure.story("SCN-GALLERY-011-A")
def test_a_gallery_reads_its_items_in_position_order(server: Sut, root_token: str) -> None:
    """The read lists each item after the previous one."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "Ordered")

    first = image_file(server, alice, "first.png", (2, 2))
    second = image_file(server, alice, "second.png", (3, 3))
    third = image_file(server, alice, "third.png", (5, 3))
    for file in (first, second, third):
        added = add_item(server, alice, gallery, "image", file["id"])
        assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"

    response = server.follow(gallery, "self", token=alice)

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["item_count"] == 3
    assert [item["file_id"] for item in body["items"]] == [first["id"], second["id"], third["id"]]
    assert [item["name"] for item in body["items"]] == ["first.png", "second.png", "third.png"]
    assert all(item["type"] == "image" and item["media_id"] > 0 for item in body["items"])


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-002")
@allure.story("SCN-GALLERY-002-A")
@allure.story("SCN-GALLERY-002-B")
def test_the_default_gallery_is_ownerless_public_and_empty(server: Sut, root_token: str) -> None:
    """The seeded gallery is readable by any caller, ownerless, public and empty."""
    _alice_id, alice = provision(server, root_token, "alice")

    response = read_gallery(server, alice, DEFAULT_GALLERY_ID)

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["id"] == DEFAULT_GALLERY_ID
    assert body["owner_id"] is None
    assert body["is_public"] is True
    assert body["item_count"] == 0
    assert body["items"] == []


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-002")
@allure.story("SCN-GALLERY-002-C")
def test_the_default_gallery_cannot_be_deleted(server: Sut, root_token: str) -> None:
    """Deleting the seeded gallery answers 409 and deletes nothing."""
    _alice_id, alice = provision(server, root_token, "alice")

    response = server.request(
        "DELETE",
        f"{server.href('gallery')}/{DEFAULT_GALLERY_ID}",
        token=alice,
    )

    assert response.status_code == 409, f"default gallery deleted: {response.status_code} {response.text}"
    kept = read_gallery(server, alice, DEFAULT_GALLERY_ID)
    assert kept.status_code == 200, f"the default gallery must survive: {kept.text}"


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-003")
@allure.story("SCN-GALLERY-003-A")
def test_a_created_gallery_is_owned_and_private(server: Sut, root_token: str) -> None:
    """Creation answers 201 with a Location; the gallery is owned and private."""
    alice_id, alice = provision(server, root_token, "alice")

    response = create_gallery_response(server, alice, "Holidays")

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["id"] > 0
    assert body["name"] == "Holidays"
    assert body["owner_id"] == alice_id
    assert body["is_public"] is False
    assert body["item_count"] == 0
    assert body["items"] == []
    assert response.headers["Location"] == body["_links"]["self"]["href"]

    stored = server.follow(body, "self", token=alice)
    assert stored.status_code == 200, f"read failed: {stored.status_code} {stored.text}"
    assert stored.json()["owner_id"] == alice_id
    assert stored.json()["is_public"] is False


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-003")
@allure.story("SCN-GALLERY-003-B")
def test_a_public_gallery_is_created_and_readable_by_another_caller(server: Sut, root_token: str) -> None:
    """`is_public` selects a public gallery another authenticated caller may read."""
    _alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")

    response = create_gallery_response(server, alice, "Shared", is_public=True)

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["is_public"] is True

    readable = server.follow(body, "self", token=bob)
    assert readable.status_code == 200, f"public read failed: {readable.status_code} {readable.text}"
    assert readable.json()["is_public"] is True


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-003")
@allure.story("SCN-GALLERY-003-C")
def test_a_duplicate_name_is_refused_and_stores_nothing(server: Sut, root_token: str) -> None:
    """A second gallery with the same trimmed name answers 409 and creates nothing."""
    alice_id, alice = provision(server, root_token, "alice")
    created = create_gallery(server, alice, "Duplicates")

    response = create_gallery_response(server, alice, "  Duplicates  ")

    assert response.status_code == 409, f"duplicate accepted: {response.status_code} {response.text}"
    listed = list_galleries(server, alice, f"?name=Duplicates&owner_id={alice_id}")
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert gallery_ids(listed) == [created["id"]]


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-003")
@allure.story("SCN-GALLERY-003-D")
def test_an_unusable_name_is_refused_and_stores_nothing(server: Sut, root_token: str) -> None:
    """Empty, whitespace-only and overlong names answer 422 and create nothing."""
    _alice_id, alice = provision(server, root_token, "alice")

    for name in ("", "   ", "x" * 101):
        response = create_gallery_response(server, alice, name)
        assert response.status_code == 422, f"name accepted: {response.status_code} {response.text}"

    listed = list_galleries(server, alice)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert gallery_ids(listed) == [DEFAULT_GALLERY_ID], "a refused creation stored a gallery"


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-004")
@allure.story("SCN-GALLERY-004-A")
@allure.story("SCN-GALLERY-004-B")
def test_the_collection_lists_what_a_caller_may_discover_and_filters_it(server: Sut, root_token: str) -> None:
    """The listing carries item counts and narrows by name, public flag and owner."""
    alice_id, alice = provision(server, root_token, "alice")
    bob_id, bob = provision(server, root_token, "bobby")

    public_gallery = create_gallery(server, alice, "Alpha", is_public=True)
    private_gallery = create_gallery(server, alice, "Beta")
    bob_gallery = create_gallery(server, bob, "Alpha", is_public=True)
    image = image_file(server, alice, "alpha.png")
    added = add_item(server, alice, public_gallery, "image", image["id"])
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"

    listed = list_galleries(server, alice)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    ids = gallery_ids(listed)
    assert public_gallery["id"] in ids and private_gallery["id"] in ids

    by_name = list_galleries(server, alice, f"?name=Alpha&owner_id={alice_id}")
    assert by_name.status_code == 200, f"filter failed: {by_name.status_code} {by_name.text}"
    assert gallery_ids(by_name) == [public_gallery["id"]]
    assert by_name.json()["items"][0]["item_count"] == 1

    private_only = list_galleries(server, alice, f"?is_public=false&owner_id={alice_id}")
    assert private_only.status_code == 200, f"filter failed: {private_only.status_code} {private_only.text}"
    assert gallery_ids(private_only) == [private_gallery["id"]]

    by_bob = list_galleries(server, alice, f"?owner_id={bob_id}&is_public=true")
    assert by_bob.status_code == 200, f"filter failed: {by_bob.status_code} {by_bob.text}"
    assert gallery_ids(by_bob) == [bob_gallery["id"]]

    discoverable = list_galleries(server, bob)
    assert discoverable.status_code == 200, f"list failed: {discoverable.status_code} {discoverable.text}"
    discoverable_ids = gallery_ids(discoverable)
    assert public_gallery["id"] in discoverable_ids
    assert private_gallery["id"] not in discoverable_ids


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-004")
@allure.story("SCN-GALLERY-004-C")
def test_a_caller_with_no_discoverable_gallery_gets_an_empty_collection(server: Sut, root_token: str) -> None:
    """A filter matching only another caller's private gallery answers an empty collection."""
    alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")
    create_gallery(server, alice, "Secret")

    response = list_galleries(server, bob, f"?owner_id={alice_id}")

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.json()["items"] == []


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-004")
@allure.story("SCN-GALLERY-004-D")
def test_a_malformed_query_is_rejected(server: Sut, root_token: str) -> None:
    """A query value that cannot be decoded answers 400 with a stable message."""
    _alice_id, alice = provision(server, root_token, "alice")

    response = list_galleries(server, alice, "?is_public=not-a-flag")

    assert response.status_code == 400, f"malformed query accepted: {response.status_code} {response.text}"
    assert response.json()["error"] == "the request query string is invalid"


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-005")
@allure.story("SCN-GALLERY-005-B")
def test_an_unknown_or_unreadable_gallery_is_not_found(server: Sut, root_token: str) -> None:
    """An unknown gallery and another caller's private one answer 404 alike."""
    _alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")
    private = create_gallery(server, alice, "Secret")

    unknown = read_gallery(server, alice, UNKNOWN_ID)
    hidden = server.follow(private, "self", token=bob)

    assert unknown.status_code == 404, f"unknown gallery: {unknown.status_code} {unknown.text}"
    assert hidden.status_code == 404, f"private gallery leaked: {hidden.status_code} {hidden.text}"


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-005")
@allure.story("SCN-GALLERY-005-C")
def test_a_malformed_gallery_identifier_is_rejected(server: Sut, root_token: str) -> None:
    """A non-numeric gallery identifier answers 400 with a stable message."""
    _alice_id, alice = provision(server, root_token, "alice")

    response = server.request("GET", f"{server.href('gallery')}/not-a-number", token=alice)

    assert response.status_code == 400, f"malformed identifier accepted: {response.status_code} {response.text}"
    assert response.json()["error"] == "invalid gallery identifier"


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-007")
@allure.story("SCN-GALLERY-007-A")
@allure.story("SCN-GALLERY-007-B")
@allure.feature("REQ-GALLERY-011")
@allure.story("SCN-GALLERY-011-B")
def test_adding_an_image_and_a_video_appends_them_and_advances_time(server: Sut, root_token: str) -> None:
    """Both media are appended by their file identifier and each change advances the time."""
    _alice_id, alice = provision(server, root_token, "alice")
    image_gallery = create_gallery(server, alice, "Images")
    video_gallery = create_gallery(server, alice, "Videos")
    image = image_file(server, alice, "sunset.png", (7, 5))
    video = video_file(server, alice, "clip.mp4")
    image_before = image_gallery["last_modified_at"]
    video_before = video_gallery["last_modified_at"]

    await_next_second()
    image_added = add_item(server, alice, image_gallery, "image", image["id"])
    video_added = add_item(server, alice, video_gallery, "video", video["id"])

    for added, media_type, file in (
        (image_added, "image", image),
        (video_added, "video", video),
    ):
        assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
        body = added.json()
        assert body["type"] == media_type
        assert body["file_id"] == file["id"]
        assert body["media_id"] > 0
        assert body["name"]
        assert body["added_at"]
        assert body["_links"]["self"]["href"] == added.headers["Location"]

    stored_image = server.follow(image_gallery, "self", token=alice).json()
    stored_video = server.follow(video_gallery, "self", token=alice).json()
    assert [item["id"] for item in stored_image["items"]] == [image_added.json()["id"]]
    assert [item["id"] for item in stored_video["items"]] == [video_added.json()["id"]]
    assert stored_image["item_count"] == 1 and stored_video["item_count"] == 1
    assert stored_image["last_modified_at"] > image_before
    assert stored_video["last_modified_at"] > video_before


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-007")
@allure.story("SCN-GALLERY-007-D")
def test_a_medium_already_in_a_gallery_is_refused(server: Sut, root_token: str) -> None:
    """A medium assigned to one gallery answers 409 in another and creates nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    first = create_gallery(server, alice, "First")
    second = create_gallery(server, alice, "Second")
    image = image_file(server, alice, "once.png")
    added = add_item(server, alice, first, "image", image["id"])
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"

    response = add_item(server, alice, second, "image", image["id"])

    assert response.status_code == 409, f"assigned medium accepted: {response.status_code} {response.text}"
    assert server.follow(second, "self", token=alice).json()["items"] == []


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-007")
@allure.story("SCN-GALLERY-007-E")
def test_a_missing_or_unowned_medium_is_refused(server: Sut, root_token: str) -> None:
    """An unknown file and another caller's file answer 422 and create nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "Mine")
    bobs_image = image_file(server, bob, "bobs.png")

    missing = add_item(server, alice, gallery, "image", UNKNOWN_ID)
    unowned = add_item(server, alice, gallery, "image", bobs_image["id"])

    assert missing.status_code == 422, f"unknown medium accepted: {missing.status_code} {missing.text}"
    assert unowned.status_code == 422, f"unowned medium accepted: {unowned.status_code} {unowned.text}"
    assert server.follow(gallery, "self", token=alice).json()["items"] == []


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-008")
@allure.story("SCN-GALLERY-008-A")
def test_an_image_item_carries_the_image_name(server: Sut, root_token: str) -> None:
    """An image item reports its medium, file, added instant and the name intake assigned."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "Images")
    image = image_file(server, alice, "sunset.png", (5, 4))
    added = add_item(server, alice, gallery, "image", image["id"])
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    item_id = added.json()["id"]

    response = server.request("GET", added.headers["Location"], token=alice)

    assert response.status_code == 200, f"item read failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["id"] == item_id
    assert body["type"] == "image"
    assert body["media_id"] > 0
    assert body["file_id"] == image["id"]
    assert body["name"] == "sunset.png"
    assert body["added_at"]
    assert body["_links"]["self"]["href"] == added.headers["Location"]


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-008")
@allure.story("SCN-GALLERY-008-B")
def test_a_video_item_carries_the_file_path(server: Sut, root_token: str) -> None:
    """A video item carries the backing file's path as its display name."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "Videos")
    video = video_file(server, alice, "clip.mp4")
    added = add_item(server, alice, gallery, "video", video["id"])
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"

    response = server.request("GET", added.headers["Location"], token=alice)

    assert response.status_code == 200, f"item read failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["type"] == "video"
    assert body["file_id"] == video["id"]
    assert body["name"] == video["path"]


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-008")
@allure.story("SCN-GALLERY-008-C")
@allure.feature("REQ-GALLERY-010")
@allure.story("SCN-GALLERY-010-D")
def test_an_item_of_another_gallery_is_not_found(server: Sut, root_token: str) -> None:
    """An item is scoped to its gallery for both the read and the removal."""
    _alice_id, alice = provision(server, root_token, "alice")
    holding = create_gallery(server, alice, "Holding")
    other = create_gallery(server, alice, "Other")
    image = image_file(server, alice, "elsewhere.png")
    added = add_item(server, alice, holding, "image", image["id"])
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    item_id = added.json()["id"]
    other_item_href = f"{server.href('gallery')}/{other['id']}/item/{item_id}"

    read = server.request("GET", other_item_href, token=alice)
    removed = server.request("DELETE", other_item_href, token=alice)

    assert read.status_code == 404, f"foreign item readable: {read.status_code} {read.text}"
    assert removed.status_code == 404, f"foreign item removed: {removed.status_code} {removed.text}"
    kept = server.request("GET", added.headers["Location"], token=alice)
    assert kept.status_code == 200, f"the item must survive a refused removal: {kept.text}"


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-010")
@allure.story("SCN-GALLERY-010-A")
@allure.story("SCN-GALLERY-010-B")
@allure.story("SCN-GALLERY-010-C")
@allure.feature("REQ-GALLERY-011")
@allure.story("SCN-GALLERY-011-B")
def test_removing_an_item_removes_its_medium_but_keeps_the_bytes(server: Sut, root_token: str) -> None:
    """Removal answers 204, deletes the medium row, keeps the bytes and advances the time."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "Removals")
    image = image_file(server, alice, "remove-me.png")
    added = add_item(server, alice, gallery, "image", image["id"])
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    item_id = added.json()["id"]
    item_href = added.headers["Location"]
    before = server.follow(gallery, "self", token=alice).json()["last_modified_at"]
    path = f"media/{image['path']}"
    digest = server.sha256(path)

    await_next_second()
    removed = server.request("DELETE", item_href, token=alice)

    assert removed.status_code == 204, f"delete failed: {removed.status_code} {removed.text}"
    assert removed.content == b""
    stored = server.follow(gallery, "self", token=alice).json()
    assert stored["item_count"] == 0
    assert stored["items"] == []
    assert stored["last_modified_at"] > before
    assert server.request("GET", item_href, token=alice).status_code == 404
    readded = add_item(server, alice, gallery, "image", image["id"])
    assert readded.status_code == 422, f"the medium survived: {readded.status_code} {readded.text}"
    assert server.sha256(path) == digest, "the stored bytes changed"
    assert item_id > 0


@allure.epic("Gallery")
@allure.feature("REQ-GALLERY-010")
@allure.story("SCN-GALLERY-010-D")
def test_removing_an_unknown_item_is_not_found(server: Sut, root_token: str) -> None:
    """An item identifier the gallery never issued answers 404 and removes nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "Empty")

    response = server.request(
        "DELETE",
        f"{link_of(gallery, 'self')}/item/{UNKNOWN_ID}",
        token=alice,
    )

    assert response.status_code == 404, f"unknown item removed: {response.status_code} {response.text}"
    assert server.follow(gallery, "self", token=alice).json()["items"] == []
    assert linked_id(link_of(gallery, "self")) == gallery["id"]


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-038")
@allure.story("SCN-AUTHORIZATION-038-A")
def test_adding_to_and_removing_from_an_unreadable_gallery_is_not_found(server: Sut, root_token: str) -> None:
    """Adding to and removing from another caller's private gallery answer 404 and change nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "Secret")
    owned = image_file(server, alice, "alices.png")
    foreign = image_file(server, bob, "bobs.png")
    added = add_item(server, alice, gallery, "image", owned["id"])
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    item_href = added.headers["Location"]

    appended = add_item(server, bob, gallery, "image", foreign["id"])
    removed = server.request("DELETE", item_href, token=bob)

    assert appended.status_code == 404, f"hidden gallery accepted the add: {appended.status_code}"
    assert removed.status_code == 404, f"hidden gallery accepted the removal: {removed.status_code}"
    kept = server.follow(gallery, "self", token=alice).json()
    assert kept["item_count"] == 1
    assert [item["file_id"] for item in kept["items"]] == [owned["id"]]
