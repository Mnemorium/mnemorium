"""E2E tests for UC-022 - Browse Movies.

No endpoint creates a film yet, so the happy paths that observe a populated
listing or one film's representation are covered by the SQLx movie-repository
integration tests and the application use-case tests. What this file exercises
against the running container is everything observable without seeded films:
authentication on both endpoints, the empty collection shape, the query and path
rejections, and the unknown-film rejection.
"""

from collections.abc import Callable

import requests
from conftest import LIBRARY_MOVIE_PATH, UNKNOWN_ID


def test_listing_movies_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_MOVIE_PATH}", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_reading_a_movie_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_MOVIE_PATH}/1", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_an_authenticated_user_lists_an_empty_movie_collection(
    list_movies: Callable[[str, str], requests.Response],
    standard_user_token: str,
) -> None:
    response = list_movies(standard_user_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert set(body.keys()) == {"items", "_links"}
    assert body["items"] == [], "no endpoint creates films, so the collection starts empty"
    assert body["_links"]["self"]["href"] == f"{LIBRARY_MOVIE_PATH}?limit=20&offset=0"
    assert body["_links"]["search"]["href"] == f"{LIBRARY_MOVIE_PATH}{{?name,limit,offset}}"
    assert body["_links"]["search"]["templated"] is True


def test_listing_movies_rejects_an_out_of_range_limit(
    list_movies: Callable[[str, str], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    for query in ("?limit=0", "?limit=101"):
        response = list_movies(standard_user_token, query)

        assert_error_body(response, 422)


def test_listing_movies_rejects_a_malformed_query(
    list_movies: Callable[[str, str], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_movies(standard_user_token, "?limit=abc")

    assert_error_body(response, 400)


def test_reading_an_unknown_movie_is_not_found(
    get_movie: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_movie(standard_user_token, UNKNOWN_ID)

    assert_error_body(response, 404)


def test_reading_a_movie_rejects_a_malformed_identifier(
    get_movie: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_movie(standard_user_token, "not-an-identifier")

    assert_error_body(response, 400)
