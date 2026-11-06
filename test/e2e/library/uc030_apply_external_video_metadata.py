"""E2E tests for UC-030 - Apply External Video Metadata.

The test container configures no video metadata provider, so a resolvable
reference cannot be applied. What this file exercises against the running
container is everything observable without one: authentication, the manage
permission, the provider rejection, the malformed-input rejection, and the
unknown-entry rejection that happens before the provider is consulted.
"""

import time
from collections.abc import Callable

import requests
from conftest import API_PREFIX, LIBRARY_MOVIE_PATH, LIBRARY_SERIES_PATH, UNKNOWN_ID

SERIES_PATH = f"{API_PREFIX}/library/series"


def _reference(provider: str = "tmdb") -> dict[str, object]:
    return {"reference": {"provider": provider, "id": "27205"}}


def _create_show_and_season(server_url: str, token: str) -> int:
    created = requests.post(
        f"{server_url}{SERIES_PATH}",
        headers={"Authorization": f"Bearer {token}"},
        json={"name": f"Apply Show {time.time_ns()}"},
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


def test_applying_to_a_film_requires_credentials(server_url: str) -> None:
    response = requests.post(f"{server_url}{LIBRARY_MOVIE_PATH}/1/apply", json=_reference(), timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_applying_to_an_unknown_film_is_not_found(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{LIBRARY_MOVIE_PATH}/{UNKNOWN_ID}/apply",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_reference(),
        timeout=10,
    )

    assert_error_body(response, 404)


def test_applying_a_music_provider_to_a_film_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{LIBRARY_MOVIE_PATH}/{UNKNOWN_ID}/apply",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_reference("musicbrainz"),
        timeout=10,
    )

    assert_error_body(response, 422)


def test_applying_without_a_configured_provider_fails_for_a_show(
    server_url: str,
    admin_token: str,
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.post(
        f"{server_url}{SERIES_PATH}/{series_id}/apply",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_reference(),
        timeout=10,
    )

    assert response.status_code == 500, f"expected 500, got {response.status_code}: {response.text}"


def test_a_standard_user_cannot_apply_to_a_show(
    server_url: str,
    admin_token: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.post(
        f"{server_url}{SERIES_PATH}/{series_id}/apply",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        json=_reference(),
        timeout=10,
    )

    assert_error_body(response, 403)


def test_applying_to_an_unknown_episode_is_not_found(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    series_id = _create_show_and_season(server_url, admin_token)

    response = requests.post(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1/episode/7/apply",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_reference(),
        timeout=10,
    )

    assert_error_body(response, 404)


def test_applying_to_an_episode_rejects_a_malformed_index(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{LIBRARY_SERIES_PATH}/{UNKNOWN_ID}/season/not-a-number/episode/1/apply",
        headers={"Authorization": f"Bearer {admin_token}"},
        json=_reference(),
        timeout=10,
    )

    assert_error_body(response, 400)
