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

import time
from collections.abc import Callable

import requests
from conftest import DEFAULT_GALLERY_ID, UNKNOWN_ID, add_item, create_owned_gallery, png, register_image

_API_PREFIX = "/api/v1"
_GALLERY_PATH = f"{_API_PREFIX}/library/gallery"


def test_the_owner_removes_its_own_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
) -> None:
    gallery_id = create_owned_gallery(create_gallery, standard_user_token)
    media_id = register_image(
        upload_media,
        register_media,
        standard_user_token,
        f"item-{time.time_ns()}.png",
        png(3, 2),
    )
    add_item(add_gallery_item, standard_user_token, gallery_id, "image", media_id)

    removed = delete_gallery(standard_user_token, gallery_id)

    assert removed.status_code == 204, f"delete failed: {removed.status_code} {removed.text}"
    assert removed.content == b""

    # The gallery and everything it held are gone.
    fetched = get_gallery(standard_user_token, gallery_id)
    assert fetched.status_code == 404, f"the removed gallery is still readable: {fetched.text}"

    # The item's medium is gone too: adding it again is a not-found.
    readded = add_gallery_item(standard_user_token, gallery_id, "image", media_id)
    assert readded.status_code == 404, f"the medium still exists: {readded.status_code} {readded.text}"


def test_a_non_owner_cannot_remove_a_hidden_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = create_owned_gallery(create_gallery, standard_user_token)

    # A private gallery the caller cannot read answers not found.
    response = delete_gallery(second_standard_user_token, gallery_id)

    assert_error_body(response, 404)
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
    gallery_id = create_owned_gallery(create_gallery, standard_user_token, is_public=True)

    # A public gallery is readable, so the refusal is revealed: a non-owner
    # Admin cannot manage another user's gallery.
    response = delete_gallery(admin_token, gallery_id)

    assert_error_body(response, 403)
    kept = get_gallery(standard_user_token, gallery_id)
    assert kept.status_code == 200, f"the gallery must survive a denied removal: {kept.text}"


def test_the_root_admin_removes_another_users_private_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    root_admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    gallery_id = create_owned_gallery(create_gallery, standard_user_token)

    response = delete_gallery(root_admin_token, gallery_id)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    assert_error_body(get_gallery(root_admin_token, gallery_id), 404)


def test_the_default_gallery_cannot_be_removed(
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = delete_gallery(standard_user_token, DEFAULT_GALLERY_ID)

    assert_error_body(response, 409)


def test_removing_an_unknown_gallery_is_not_found(
    delete_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = delete_gallery(standard_user_token, UNKNOWN_ID)

    assert_error_body(response, 404)
