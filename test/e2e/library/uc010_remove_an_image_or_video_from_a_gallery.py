"""E2E tests for UC-010 - Remove an Image or Video from a Gallery.

The happy path and every alternative flow run against the running container:
the gallery owner removing its own item, any authenticated user removing an
item of a public gallery, the denial of a non-owner Standard User and of a
non-owner Admin on a private gallery, the Root Admin override, and the
unknown-gallery and item-not-in-gallery rejections.

Removal is destructive: the item and its image or video row are deleted, and
the backing file with its bytes is deleted when the caller owns the file. No
endpoint exposes files, so the backing file's deletion is asserted by the
repository and storage integration tests; these tests assert the observable:
the item and its medium are gone, which the rejection of re-adding the same
media proves.
"""

import time
from collections.abc import Callable

import requests
from conftest import UNKNOWN_ID, add_item, create_owned_gallery, png, register_image, video

_API_PREFIX = "/api/v1"
_GALLERY_PATH = f"{_API_PREFIX}/library/gallery"


def _assert_item_and_medium_are_gone(
    get_gallery: Callable[[str, int], requests.Response],
    get_gallery_item: Callable[[str, int, int], requests.Response],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    token: str,
    gallery_id: int,
    item_id: int,
    media_type: str,
    media_id: int,
) -> None:
    """Assert the removed item is gone and its medium no longer exists."""
    stored = get_gallery(token, gallery_id)
    assert stored.status_code == 200, f"get failed: {stored.status_code} {stored.text}"
    body = stored.json()
    assert body["item_count"] == 0
    assert body["items"] == []

    fetched = get_gallery_item(token, gallery_id, item_id)
    assert fetched.status_code == 404, f"the removed item is still readable: {fetched.text}"

    # The image or video row is gone too: the file identifier no longer resolves
    # to a medium, so adding it again fails the media body rule (`422`).
    readded = add_gallery_item(token, gallery_id, media_type, media_id)
    assert readded.status_code == 422, f"the medium still exists: {readded.status_code} {readded.text}"


def test_the_owner_removes_its_own_item(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    get_gallery_item: Callable[[str, int, int], requests.Response],
    delete_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
) -> None:
    gallery_id = create_owned_gallery(create_gallery, standard_user_token, is_public=False)
    media_id = register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"mine-{time.time_ns()}.png",
        png(4, 3),
    )
    item_id = add_item(add_gallery_item, standard_user_token, gallery_id, "image", media_id)

    removed = delete_gallery_item(standard_user_token, gallery_id, item_id)

    assert removed.status_code == 204, f"delete failed: {removed.status_code} {removed.text}"
    assert removed.content == b""
    _assert_item_and_medium_are_gone(
        get_gallery,
        get_gallery_item,
        add_gallery_item,
        standard_user_token,
        gallery_id,
        item_id,
        "image",
        media_id,
    )


def test_any_user_removes_an_item_from_a_public_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    get_gallery_item: Callable[[str, int, int], requests.Response],
    delete_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    gallery_id = create_owned_gallery(create_gallery, second_standard_user_token, is_public=True)
    media_id = register_media(
        second_standard_user_token,
        upload_media(second_standard_user_token, f"clip-{time.time_ns()}.mp4", video(), "video/mp4"),
    )
    item_id = add_item(add_gallery_item, second_standard_user_token, gallery_id, "video", media_id)

    # Any authenticated user may remove an item of a public gallery, not only the
    # gallery's owner.
    removed = delete_gallery_item(standard_user_token, gallery_id, item_id)

    assert removed.status_code == 204, f"delete failed: {removed.status_code} {removed.text}"
    _assert_item_and_medium_are_gone(
        get_gallery,
        get_gallery_item,
        add_gallery_item,
        standard_user_token,
        gallery_id,
        item_id,
        "video",
        media_id,
    )


def test_an_admin_cannot_remove_an_item_from_a_private_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery_item: Callable[[str, int, int], requests.Response],
    delete_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = create_owned_gallery(create_gallery, standard_user_token, is_public=False)
    media_id = register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"private-{time.time_ns()}.png",
        png(2, 2),
    )
    item_id = add_item(add_gallery_item, standard_user_token, gallery_id, "image", media_id)

    response = delete_gallery_item(admin_token, gallery_id, item_id)

    assert_error_body(response, 403)
    kept = get_gallery_item(standard_user_token, gallery_id, item_id)
    assert kept.status_code == 200, f"the item must survive a denied removal: {kept.text}"


def test_another_user_cannot_remove_an_item_from_a_private_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery_item: Callable[[str, int, int], requests.Response],
    delete_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = create_owned_gallery(create_gallery, standard_user_token, is_public=False)
    media_id = register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"private-other-{time.time_ns()}.png",
        png(2, 2),
    )
    item_id = add_item(add_gallery_item, standard_user_token, gallery_id, "image", media_id)

    response = delete_gallery_item(second_standard_user_token, gallery_id, item_id)

    assert_error_body(response, 403)
    kept = get_gallery_item(standard_user_token, gallery_id, item_id)
    assert kept.status_code == 200, f"the item must survive a denied removal: {kept.text}"


def test_the_root_admin_can_remove_an_item_from_a_private_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    get_gallery_item: Callable[[str, int, int], requests.Response],
    delete_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    root_admin_token: str,
) -> None:
    gallery_id = create_owned_gallery(create_gallery, standard_user_token, is_public=False)
    media_id = register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"override-{time.time_ns()}.png",
        png(3, 2),
    )
    item_id = add_item(add_gallery_item, standard_user_token, gallery_id, "image", media_id)

    removed = delete_gallery_item(root_admin_token, gallery_id, item_id)

    assert removed.status_code == 204, f"delete failed: {removed.status_code} {removed.text}"
    _assert_item_and_medium_are_gone(
        get_gallery,
        get_gallery_item,
        add_gallery_item,
        root_admin_token,
        gallery_id,
        item_id,
        "image",
        media_id,
    )


def test_removing_an_item_not_in_the_gallery_is_not_found(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    delete_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    holding_gallery_id = create_owned_gallery(create_gallery, standard_user_token, is_public=False)
    media_id = register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"elsewhere-{time.time_ns()}.png",
        png(2, 3),
    )
    item_id = add_item(add_gallery_item, standard_user_token, holding_gallery_id, "image", media_id)
    other_gallery_id = create_owned_gallery(create_gallery, standard_user_token, is_public=True)

    # The item is scoped to the gallery named in the path.
    response = delete_gallery_item(standard_user_token, other_gallery_id, item_id)

    assert_error_body(response, 404)


def test_removing_from_an_unknown_gallery_is_not_found(
    delete_gallery_item: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = delete_gallery_item(standard_user_token, UNKNOWN_ID, 1)

    assert_error_body(response, 404)
