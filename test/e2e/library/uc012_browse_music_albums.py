"""E2E tests for UC-012 - Browse Music Albums.

No endpoint creates a music album yet, so the happy paths that observe a
populated listing, a medium, an album medium's ordered tracks and one track's
representation are covered by the integration tests of the SQLx music-album
repository and the application use cases. What this file exercises against the
running container is everything observable without seeded music: authentication
on all four endpoints, the empty album collection shape, the query and path
rejections, and the unknown-album, unknown-medium and unknown-track rejections.
"""

from collections.abc import Callable

import requests
from conftest import LIBRARY_ALBUM_PATH, UNKNOWN_ID


def test_listing_albums_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_ALBUM_PATH}", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_reading_a_medium_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_ALBUM_PATH}/1/medium/1", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_reading_the_tracks_of_a_medium_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_ALBUM_PATH}/1/medium/1/track", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_reading_a_track_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_ALBUM_PATH}/1/medium/1/track/1", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_an_authenticated_user_lists_an_empty_album_collection(
    list_albums: Callable[[str, str], requests.Response],
    standard_user_token: str,
) -> None:
    response = list_albums(standard_user_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert set(body.keys()) == {"items", "_links"}
    assert body["items"] == [], "no endpoint creates music, so the collection starts empty"
    assert body["_links"]["self"]["href"] == f"{LIBRARY_ALBUM_PATH}?limit=20&offset=0"
    assert body["_links"]["search"]["href"] == f"{LIBRARY_ALBUM_PATH}{{?name,limit,offset}}"
    assert body["_links"]["search"]["templated"] is True


def test_listing_albums_rejects_an_out_of_range_limit(
    list_albums: Callable[[str, str], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_albums(standard_user_token, "?limit=0")

    assert_error_body(response, 422)


def test_listing_albums_rejects_a_malformed_query(
    list_albums: Callable[[str, str], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_albums(standard_user_token, "?limit=abc")

    assert_error_body(response, 400)


def test_reading_an_unknown_album_medium_is_not_found(
    get_album_medium: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_album_medium(standard_user_token, UNKNOWN_ID, 1)

    assert_error_body(response, 404)


def test_reading_a_medium_rejects_a_malformed_index(
    get_album_medium: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_album_medium(standard_user_token, 1, "not-an-index")

    assert_error_body(response, 400)


def test_listing_the_tracks_of_an_unknown_album_medium_is_not_found(
    list_album_tracks: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_album_tracks(standard_user_token, UNKNOWN_ID, 1)

    assert_error_body(response, 404)


def test_listing_tracks_rejects_a_malformed_album_identifier(
    list_album_tracks: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_album_tracks(standard_user_token, "not-an-identifier", 1)

    assert_error_body(response, 400)


def test_listing_tracks_rejects_a_malformed_medium_index(
    list_album_tracks: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_album_tracks(standard_user_token, 1, "not-an-index")

    assert_error_body(response, 400)


def test_reading_an_unknown_track_is_not_found(
    get_album_track: Callable[[str, int, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_album_track(standard_user_token, UNKNOWN_ID, 1, 1)

    assert_error_body(response, 404)


def test_reading_a_track_rejects_a_malformed_track_index(
    get_album_track: Callable[[str, int, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_album_track(standard_user_token, 1, 1, "not-an-index")

    assert_error_body(response, 400)
