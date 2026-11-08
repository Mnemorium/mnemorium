"""E2E tests for UC-029 - Write Episode Metadata.

No endpoint exposes the identifier of an ingested video yet, so the happy paths
that create or replace an episode are covered by the SQLx series-repository
integration tests, the application use-case tests and the handler tests. What
this file exercises against the running container is everything observable
without a seeded video: authentication, the manage permission, the position and
body rejections, and the unknown-show and unknown-season rejections.
"""

import time
from collections.abc import Callable

import requests
from conftest import API_PREFIX, LIBRARY_MOVIE_PATH, LIBRARY_SERIES_PATH, UNKNOWN_ID, video

SERIES_PATH = f"{API_PREFIX}/library/series"


def _create_show_and_season(server_url: str, token: str) -> int:
    created = requests.post(
        f"{server_url}{SERIES_PATH}",
        headers={"Authorization": f"Bearer {token}"},
        json={"name": f"Episode Show {time.time_ns()}"},
        timeout=10,
    )
    assert created.status_code == 201, f"show creation failed: {created.status_code} {created.text}"
    series_id = int(created.json()["id"])
    season = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1",
        headers={"Authorization": f"Bearer {token}"},
        json={"name": "Season One"},
        timeout=10,
    )
    assert season.status_code == 201, f"season creation failed: {season.status_code} {season.text}"
    return series_id


def _headers(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def _video_id(
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    token: str,
) -> int:
    gallery = create_gallery(token, f"video-flow-{time.time_ns()}", True)
    assert gallery.status_code == 201, f"gallery creation failed: {gallery.text}"
    gallery_id = int(gallery.json()["id"])
    file_id = register_media(token, upload_media(token, f"flow-{time.time_ns()}.mp4", video(), "video/mp4"))
    added = add_gallery_item(token, gallery_id, "video", file_id)
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    assert added.json()["type"] == "video"
    return int(added.json()["media_id"])


def test_putting_an_episode_requires_credentials(server_url: str) -> None:
    response = requests.put(
        f"{server_url}{LIBRARY_SERIES_PATH}/{UNKNOWN_ID}/season/1/episode/1",
        json={"name": "Pilot", "video_id": UNKNOWN_ID},
        timeout=10,
    )

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_a_standard_user_cannot_put_an_episode(
    server_url: str,
    admin_token: str,
    standard_user_token: str,
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/1",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        json={"name": "Pilot", "video_id": UNKNOWN_ID},
        timeout=10,
    )

    assert response.status_code == 403, f"expected 403, got {response.status_code}: {response.text}"


def test_putting_an_episode_with_an_unknown_video_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/1",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Pilot", "video_id": UNKNOWN_ID},
        timeout=10,
    )

    assert_error_body(response, 422)


def test_putting_an_episode_with_an_empty_name_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/1",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "", "video_id": UNKNOWN_ID},
        timeout=10,
    )

    assert_error_body(response, 422)


def test_putting_an_episode_of_an_unknown_season_is_not_found(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/9/episode/1",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Pilot", "video_id": UNKNOWN_ID},
        timeout=10,
    )

    assert_error_body(response, 404)


def test_putting_an_out_of_range_episode_index_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/4294967296",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Pilot", "video_id": UNKNOWN_ID},
        timeout=10,
    )

    assert_error_body(response, 422)


def test_patching_an_unknown_episode_is_not_found(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.patch(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/7",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Pilot"},
        timeout=10,
    )

    assert_error_body(response, 404)


def test_show_season_episode_create_patch_read_round_trip(
    server_url: str,
    admin_token: str,
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
) -> None:
    video_id = _video_id(create_gallery, upload_media, register_media, add_gallery_item, admin_token)

    created = requests.post(
        f"{server_url}{SERIES_PATH}",
        headers=_headers(admin_token),
        json={"name": f"Twin Peaks {time.time_ns()}", "country_of_origin": "US"},
        timeout=10,
    )
    assert created.status_code == 201, f"show creation failed: {created.status_code} {created.text}"
    series_id = int(created.json()["id"])

    season = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1",
        headers=_headers(admin_token),
        json={"name": "Season One"},
        timeout=10,
    )
    assert season.status_code == 201, f"season creation failed: {season.status_code} {season.text}"

    episode = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/1",
        headers=_headers(admin_token),
        json={"video_id": video_id, "name": "Pilot", "air_date": "1990-04-08"},
        timeout=10,
    )
    assert episode.status_code == 201, f"episode creation failed: {episode.status_code} {episode.text}"
    assert episode.json()["video"]["id"] == video_id

    patched = requests.patch(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/1",
        headers=_headers(admin_token),
        json={"name": "Pilot (corrected)", "air_date": None},
        timeout=10,
    )
    assert patched.status_code == 200, f"episode patch failed: {patched.status_code} {patched.text}"
    assert patched.json()["name"] == "Pilot (corrected)"
    assert patched.json()["air_date"] is None
    assert patched.json()["video"]["id"] == video_id, "patch must not change the video"

    read = requests.get(f"{server_url}{SERIES_PATH}/{series_id}", headers=_headers(admin_token), timeout=10)
    assert read.status_code == 200
    body = read.json()
    assert body["seasons"][0]["episode_count"] == 1
    episode_read = requests.get(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/1",
        headers=_headers(admin_token),
        timeout=10,
    )
    assert episode_read.status_code == 200
    assert episode_read.json()["name"] == "Pilot (corrected)"
    assert episode_read.json()["air_date"] is None


def test_a_video_cannot_back_both_a_film_and_an_episode(
    server_url: str,
    admin_token: str,
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
) -> None:
    film_video = _video_id(create_gallery, upload_media, register_media, add_gallery_item, admin_token)
    episode_video = _video_id(create_gallery, upload_media, register_media, add_gallery_item, admin_token)

    film = requests.post(
        f"{server_url}{LIBRARY_MOVIE_PATH}",
        headers=_headers(admin_token),
        json={"video_id": film_video, "name": "Film"},
        timeout=10,
    )
    assert film.status_code == 201, film.text

    show = requests.post(
        f"{server_url}{SERIES_PATH}",
        headers=_headers(admin_token),
        json={"name": f"Conflict Show {time.time_ns()}"},
        timeout=10,
    )
    series_id = int(show.json()["id"])
    requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1",
        headers=_headers(admin_token),
        json={"name": "Season One"},
        timeout=10,
    )

    refused_episode = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/1",
        headers=_headers(admin_token),
        json={"video_id": film_video, "name": "Pilot"},
        timeout=10,
    )
    assert refused_episode.status_code == 409, (
        f"expected 409, got {refused_episode.status_code}: {refused_episode.text}"
    )

    accepted_episode = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/1",
        headers=_headers(admin_token),
        json={"video_id": episode_video, "name": "Pilot"},
        timeout=10,
    )
    assert accepted_episode.status_code == 201, accepted_episode.text

    refused_film = requests.post(
        f"{server_url}{LIBRARY_MOVIE_PATH}",
        headers=_headers(admin_token),
        json={"video_id": episode_video, "name": "Another Film"},
        timeout=10,
    )
    assert refused_film.status_code == 409, f"expected 409, got {refused_film.status_code}: {refused_film.text}"
