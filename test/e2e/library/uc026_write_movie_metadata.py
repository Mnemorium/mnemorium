"""E2E tests for UC-026 - Write Movie Metadata.

No endpoint exposes the identifier of an ingested video yet, so the happy paths
that create or patch a film are covered by the SQLx movie-repository integration
tests, the application use-case tests and the handler tests. What this file
exercises against the running container is everything observable without a
seeded video: authentication, the manage permission, the input rejections and
the unknown-film rejection.
"""

import time
from collections.abc import Callable

import requests
from conftest import API_PREFIX, LIBRARY_MOVIE_PATH, UNKNOWN_ID, video

MOVIE_PATH = f"{API_PREFIX}/library/movie"


def _payload(**overrides: object) -> dict[str, object]:
    body: dict[str, object] = {"name": "Some Like It Hot", "video_id": UNKNOWN_ID}
    body.update(overrides)
    return body


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


def test_creating_a_movie_requires_credentials(server_url: str) -> None:
    response = requests.post(f"{server_url}{MOVIE_PATH}", json=_payload(), timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_a_standard_user_cannot_create_a_movie(
    server_url: str,
    standard_user_token: str,
) -> None:
    response = requests.post(
        f"{server_url}{MOVIE_PATH}",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        json=_payload(),
        timeout=10,
    )

    assert response.status_code == 403, f"expected 403, got {response.status_code}: {response.text}"


def test_creating_a_movie_with_an_unknown_video_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{MOVIE_PATH}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_payload(),
        timeout=10,
    )

    assert_error_body(response, 422)


def test_creating_a_movie_with_an_empty_name_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{MOVIE_PATH}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_payload(name=""),
        timeout=10,
    )

    assert_error_body(response, 422)


def test_creating_a_movie_with_a_malformed_date_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{MOVIE_PATH}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_payload(release_date="17/08/1959"),
        timeout=10,
    )

    assert_error_body(response, 422)


def test_creating_a_movie_with_a_malformed_country_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{MOVIE_PATH}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_payload(country_of_origin="U.S.A."),
        timeout=10,
    )

    assert_error_body(response, 422)


def test_patching_an_unknown_movie_is_not_found(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.patch(
        f"{server_url}{LIBRARY_MOVIE_PATH}/{UNKNOWN_ID}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Another Name"},
        timeout=10,
    )

    assert_error_body(response, 404)


def test_patching_a_movie_rejects_a_malformed_identifier(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.patch(
        f"{server_url}{LIBRARY_MOVIE_PATH}/not-a-number",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Another Name"},
        timeout=10,
    )

    assert_error_body(response, 400)


def test_film_create_patch_read_round_trip(
    server_url: str,
    admin_token: str,
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
) -> None:
    video_id = _video_id(create_gallery, upload_media, register_media, add_gallery_item, admin_token)

    created = requests.post(
        f"{server_url}{LIBRARY_MOVIE_PATH}",
        headers=_headers(admin_token),
        json={
            "video_id": video_id,
            "name": "Some Like It Hot",
            "plot": "Two musicians on the run",
            "release_date": "1959-08-17",
            "country_of_origin": "us",
            "genres": ["Comedy"],
        },
        timeout=10,
    )
    assert created.status_code == 201, f"film creation failed: {created.status_code} {created.text}"
    film = created.json()
    assert film["name"] == "Some Like It Hot"
    assert film["country_of_origin"] == "US"
    assert film["genres"] == ["COMEDY"]
    assert film["video"]["id"] == video_id
    movie_id = int(film["id"])

    patched = requests.patch(
        f"{server_url}{LIBRARY_MOVIE_PATH}/{movie_id}",
        headers=_headers(admin_token),
        json={"name": "A Corrected Title", "release_date": None, "genres": ["Drama"]},
        timeout=10,
    )
    assert patched.status_code == 200, f"patch failed: {patched.status_code} {patched.text}"
    corrected = patched.json()
    assert corrected["name"] == "A Corrected Title"
    assert corrected["release_date"] is None
    assert corrected["genres"] == ["DRAMA"]
    assert corrected["video"]["id"] == video_id

    read = requests.get(f"{server_url}{LIBRARY_MOVIE_PATH}/{movie_id}", headers=_headers(admin_token), timeout=10)
    assert read.status_code == 200
    assert read.json()["name"] == "A Corrected Title"
    assert read.json()["release_date"] is None
