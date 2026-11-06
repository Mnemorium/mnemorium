"""E2E tests for UC-025 - Identify Video Metadata.

The test container configures no video metadata provider, so the observable
surface is authentication, the kind-scoped routes, the unavailable document, and
the query and path rejections. A configured provider's candidates and metadata
documents are covered by the handler and use-case tests.
"""

from collections.abc import Callable

import requests
from conftest import API_PREFIX

LIBRARY_MOVIE_IDENTIFY_PATH = f"{API_PREFIX}/library/movie/identify"
LIBRARY_SERIES_IDENTIFY_PATH = f"{API_PREFIX}/library/series/identify"


def test_identifying_a_video_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_MOVIE_IDENTIFY_PATH}?name=Inception", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_identifying_a_film_reports_unavailable_without_a_provider(server_url: str, standard_user_token: str) -> None:
    response = requests.get(
        f"{server_url}{LIBRARY_MOVIE_IDENTIFY_PATH}?name=Inception",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert response.status_code == 200, f"{response.status_code}: {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert body["available"] is False, "no provider is configured, so identification is unavailable"
    assert body["items"] == []
    assert body["_links"]["self"]["href"] == LIBRARY_MOVIE_IDENTIFY_PATH


def test_identifying_a_show_reports_unavailable_without_a_provider(server_url: str, standard_user_token: str) -> None:
    response = requests.get(
        f"{server_url}{LIBRARY_SERIES_IDENTIFY_PATH}?name=Twin%20Peaks",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert response.status_code == 200, f"{response.status_code}: {response.text}"
    assert response.json()["available"] is False


def test_identifying_rejects_an_empty_name(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.get(
        f"{server_url}{LIBRARY_MOVIE_IDENTIFY_PATH}?name=",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert_error_body(response, 422)


def test_resolving_a_film_reference_reports_unavailable_without_a_provider(
    server_url: str, standard_user_token: str
) -> None:
    response = requests.get(
        f"{server_url}{LIBRARY_MOVIE_IDENTIFY_PATH}/tmdb/27205",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert response.status_code == 200, f"{response.status_code}: {response.text}"
    body = response.json()
    assert body["available"] is False
    assert body["_links"]["self"]["href"] == f"{LIBRARY_MOVIE_IDENTIFY_PATH}/tmdb/27205"


def test_resolving_a_show_reference_reports_unavailable_without_a_provider(
    server_url: str, standard_user_token: str
) -> None:
    response = requests.get(
        f"{server_url}{LIBRARY_SERIES_IDENTIFY_PATH}/tmdb/1920?season=1&episode=2",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert response.status_code == 200, f"{response.status_code}: {response.text}"
    assert response.json()["available"] is False


def test_resolving_rejects_an_unknown_provider(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.get(
        f"{server_url}{LIBRARY_MOVIE_IDENTIFY_PATH}/musicbrainz/27205",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert_error_body(response, 404)


def test_resolving_rejects_a_partial_episode_position(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.get(
        f"{server_url}{LIBRARY_SERIES_IDENTIFY_PATH}/tmdb/1920?season=1",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert_error_body(response, 422)


def test_resolving_rejects_an_out_of_range_episode_position(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.get(
        f"{server_url}{LIBRARY_SERIES_IDENTIFY_PATH}/tmdb/1920?season=-1&episode=2",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert_error_body(response, 422)
