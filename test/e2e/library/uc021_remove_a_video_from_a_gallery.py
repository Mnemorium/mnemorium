"""E2E tests for UC-021 - Remove a Video from a Gallery.

The happy path runs against the running container: a video added to a public
gallery is removed by another authenticated user, and the item, its video
record and the observable state are gone afterwards.

Removal is destructive: the item and its video row are deleted, and the backing
file with its bytes is deleted when the caller owns the file. No endpoint
exposes files, so the backing file's deletion is asserted by the repository and
storage integration tests; these tests assert the observable: the item and its
medium are gone, which the rejection of re-adding the same media proves.
"""

import time
from collections.abc import Callable

import requests
from conftest import add_item, create_owned_gallery, video

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

    # The video row is gone too: the file identifier no longer resolves to a
    # medium, so adding it again fails the media body rule (`422`).
    readded = add_gallery_item(token, gallery_id, media_type, media_id)
    assert readded.status_code == 422, f"the medium still exists: {readded.status_code} {readded.text}"


def test_any_user_removes_a_video_from_a_public_gallery(
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
    assert removed.content == b""
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
