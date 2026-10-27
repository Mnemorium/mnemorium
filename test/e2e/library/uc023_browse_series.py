"""E2E tests for UC-023 - Browse Series.

No endpoint creates a show, a season or an episode yet, so the happy paths that
observe a populated hierarchy are covered by the SQLx series-repository
integration tests and the application use-case tests. What this file exercises
against the running container is everything observable without seeded shows:
authentication on all five endpoints, the empty collection shape, the query and
path rejections, the unknown-show, unknown-season and unknown-episode
rejections, and the 32-bit unsigned bounds of the season and episode indexes.
"""

from collections.abc import Callable

import requests
from conftest import LIBRARY_SERIES_PATH, UNKNOWN_ID

# The largest index a 32-bit unsigned position admits.
MAX_U32_INDEX = 4_294_967_295

# The first value a 32-bit unsigned position cannot represent.
OUT_OF_RANGE_INDEX = 4_294_967_296


def test_listing_series_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_SERIES_PATH}", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_reading_a_series_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_SERIES_PATH}/1", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_reading_a_season_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_SERIES_PATH}/1/season/1", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_listing_episodes_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_SERIES_PATH}/1/season/1/episode", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_reading_an_episode_requires_credentials(server_url: str) -> None:
    response = requests.get(f"{server_url}{LIBRARY_SERIES_PATH}/1/season/1/episode/1", timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_an_authenticated_user_lists_an_empty_series_collection(
    list_series: Callable[[str, str], requests.Response],
    standard_user_token: str,
) -> None:
    response = list_series(standard_user_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert set(body.keys()) == {"items", "_links"}
    assert body["items"] == [], "no endpoint creates shows, so the collection starts empty"
    assert body["_links"]["self"]["href"] == f"{LIBRARY_SERIES_PATH}?limit=20&offset=0"
    assert body["_links"]["search"]["href"] == f"{LIBRARY_SERIES_PATH}{{?name,limit,offset}}"
    assert body["_links"]["search"]["templated"] is True


def test_listing_series_rejects_an_out_of_range_limit(
    list_series: Callable[[str, str], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    for query in ("?limit=0", "?limit=101"):
        response = list_series(standard_user_token, query)

        assert_error_body(response, 422)


def test_listing_series_rejects_a_malformed_query(
    list_series: Callable[[str, str], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_series(standard_user_token, "?offset=abc")

    assert_error_body(response, 400)


def test_reading_an_unknown_series_is_not_found(
    get_series: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series(standard_user_token, UNKNOWN_ID)

    assert_error_body(response, 404)


def test_reading_a_series_rejects_a_malformed_identifier(
    get_series: Callable[[str, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series(standard_user_token, "not-an-identifier")

    assert_error_body(response, 400)


def test_reading_an_unknown_season_is_not_found(
    get_series_season: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series_season(standard_user_token, UNKNOWN_ID, 1)

    assert_error_body(response, 404)


def test_reading_a_season_rejects_a_malformed_index(
    get_series_season: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series_season(standard_user_token, 1, "not-an-index")

    assert_error_body(response, 400)


def test_listing_the_episodes_of_an_unknown_season_is_not_found(
    list_series_episodes: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_series_episodes(standard_user_token, UNKNOWN_ID, 1)

    assert_error_body(response, 404)


def test_listing_episodes_rejects_a_malformed_series_identifier(
    list_series_episodes: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_series_episodes(standard_user_token, "not-an-identifier", 1)

    assert_error_body(response, 400)


def test_listing_episodes_rejects_a_malformed_season_index(
    list_series_episodes: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = list_series_episodes(standard_user_token, 1, "not-an-index")

    assert_error_body(response, 400)


def test_reading_an_unknown_episode_is_not_found(
    get_series_episode: Callable[[str, int, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series_episode(standard_user_token, UNKNOWN_ID, 1, 1)

    assert_error_body(response, 404)


def test_reading_an_episode_rejects_a_malformed_episode_index(
    get_series_episode: Callable[[str, int, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series_episode(standard_user_token, 1, 1, "not-an-index")

    assert_error_body(response, 400)


def test_the_largest_u32_season_index_is_addressed_but_unknown(
    get_series_season: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series_season(standard_user_token, UNKNOWN_ID, MAX_U32_INDEX)

    assert_error_body(response, 404)


def test_a_season_index_above_the_u32_range_is_rejected(
    get_series_season: Callable[[str, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series_season(standard_user_token, 1, OUT_OF_RANGE_INDEX)

    assert_error_body(response, 400)


def test_the_largest_u32_episode_index_is_addressed_but_unknown(
    get_series_episode: Callable[[str, int, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series_episode(standard_user_token, UNKNOWN_ID, 1, MAX_U32_INDEX)

    assert_error_body(response, 404)


def test_an_episode_index_above_the_u32_range_is_rejected(
    get_series_episode: Callable[[str, int, int, int], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = get_series_episode(standard_user_token, 1, 1, OUT_OF_RANGE_INDEX)

    assert_error_body(response, 400)
