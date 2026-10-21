"""E2E tests for UC-020 - Add a Video to a Gallery.

The happy path runs against the running container: an owned video is added to a
public gallery, and the gallery then shows the new item with its video medium.
The video fixture is a real, tiny H.264 file the container's `ffprobe` can
probe.

Media registration has no route of its own (design § 8): completing an upload
registers the video as a side effect.
"""

import time
from collections.abc import Callable

import requests
from conftest import create_owned_gallery, video

_API_PREFIX = "/api/v1"
_GALLERY_PATH = f"{_API_PREFIX}/library/gallery"


def test_adding_an_owned_video_to_a_gallery_appends_it(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    get_gallery: Callable[[str, int], requests.Response],
    standard_user_token: str,
) -> None:
    gallery_id = create_owned_gallery(create_gallery, standard_user_token, is_public=True)
    file_id = register_media(
        standard_user_token,
        upload_media(standard_user_token, f"clip-{time.time_ns()}.mp4", video(), "video/mp4"),
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
