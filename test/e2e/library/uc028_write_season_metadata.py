"""E2E tests for UC-028 - Write Season Metadata.

A show can be created without any external media, so both season writes are
exercised end to end: create by index, replace by the same index, partial
correction and the observable rejections.
"""

import time
from collections.abc import Callable

import requests
from conftest import API_PREFIX, LIBRARY_SERIES_PATH, UNKNOWN_ID

SERIES_PATH = f"{API_PREFIX}/library/series"


def _create_show(server_url: str, token: str) -> int:
    response = requests.post(
        f"{server_url}{SERIES_PATH}",
        headers={"Authorization": f"Bearer {token}"},
        json={"name": f"Season Show {time.time_ns()}"},
        timeout=10,
    )
    assert response.status_code == 201, f"show creation failed: {response.status_code} {response.text}"
    return int(response.json()["id"])


def test_putting_a_season_creates_then_replaces_it(
    server_url: str,
    admin_token: str,
) -> None:
    series_id = _create_show(server_url, admin_token)
    season_path = f"{SERIES_PATH}/{series_id}/season/1"

    created = requests.put(
        f"{server_url}{season_path}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Season One"},
        timeout=10,
    )

    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    assert created.headers.get("Location", "").endswith(season_path)
    assert created.json()["index"] == 1
    assert created.json()["name"] == "Season One"
    assert created.json()["episode_count"] == 0

    replaced = requests.put(
        f"{server_url}{season_path}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "First Season"},
        timeout=10,
    )

    assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"
    assert replaced.json()["index"] == 1
    assert replaced.json()["name"] == "First Season"


def test_patching_a_season_changes_and_clears_its_name(
    server_url: str,
    admin_token: str,
) -> None:
    series_id = _create_show(server_url, admin_token)
    season_path = f"{SERIES_PATH}/{series_id}/season/2"
    created = requests.put(
        f"{server_url}{season_path}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Season Two"},
        timeout=10,
    )
    assert created.status_code == 201, created.text

    renamed = requests.patch(
        f"{server_url}{season_path}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Second Season"},
        timeout=10,
    )

    assert renamed.status_code == 200, renamed.text
    assert renamed.json()["name"] == "Second Season"

    cleared = requests.patch(
        f"{server_url}{season_path}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": None},
        timeout=10,
    )

    assert cleared.status_code == 200, cleared.text
    assert cleared.json()["name"] is None


def test_a_standard_user_cannot_put_a_season(
    server_url: str,
    admin_token: str,
    standard_user_token: str,
) -> None:
    series_id = _create_show(server_url, admin_token)

    response = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/1",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        json={"name": "Season One"},
        timeout=10,
    )

    assert response.status_code == 403, f"expected 403, got {response.status_code}: {response.text}"


def test_putting_a_season_of_an_unknown_show_is_not_found(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.put(
        f"{server_url}{LIBRARY_SERIES_PATH}/{UNKNOWN_ID}/season/1",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Season One"},
        timeout=10,
    )

    assert_error_body(response, 404)


def test_putting_an_out_of_range_season_index_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    series_id = _create_show(server_url, admin_token)

    response = requests.put(
        f"{server_url}{SERIES_PATH}/{series_id}/season/4294967296",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Season One"},
        timeout=10,
    )

    assert_error_body(response, 400)
