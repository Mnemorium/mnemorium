"""E2E tests for UC-027 - Write Series Metadata.

A show needs no backing video, so this file exercises both writes against the
running container: creation, the insert-only rule, partial correction and the
observable rejections.
"""

import time
from collections.abc import Callable

import requests
from conftest import API_PREFIX, LIBRARY_SERIES_PATH, UNKNOWN_ID

SERIES_PATH = f"{API_PREFIX}/library/series"


def _create_series(server_url: str, token: str, name: str) -> requests.Response:
    return requests.post(
        f"{server_url}{SERIES_PATH}",
        headers={"Authorization": f"Bearer {token}"},
        json={"name": name, "plot": "A plot"},
        timeout=10,
    )


def test_creating_a_show_requires_credentials(server_url: str) -> None:
    response = requests.post(f"{server_url}{SERIES_PATH}", json={"name": "Twin Peaks"}, timeout=10)

    assert response.status_code == 401, f"expected 401, got {response.status_code}: {response.text}"


def test_a_standard_user_cannot_create_a_show(
    server_url: str,
    standard_user_token: str,
) -> None:
    response = _create_series(server_url, standard_user_token, "Twin Peaks")

    assert response.status_code == 403, f"expected 403, got {response.status_code}: {response.text}"


def test_creating_a_show_returns_the_stored_show(
    server_url: str,
    admin_token: str,
) -> None:
    name = f"Twin Peaks {time.time_ns()}"

    response = _create_series(server_url, admin_token, name)

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    assert response.headers.get("Location", "").startswith(SERIES_PATH)
    body = response.json()
    assert body["id"] > 0
    assert body["name"] == name
    assert body["plot"] == "A plot"
    assert body["seasons"] == []
    assert body["_links"]["self"]["href"] == f"{SERIES_PATH}/{body['id']}"


def test_a_repeated_name_creates_another_show(
    server_url: str,
    admin_token: str,
) -> None:
    name = f"Repeated {time.time_ns()}"

    first = _create_series(server_url, admin_token, name)
    second = _create_series(server_url, admin_token, name)

    assert first.status_code == 201, first.text
    assert second.status_code == 201, second.text
    assert first.json()["id"] != second.json()["id"]


def test_patching_a_show_changes_and_clears_metadata(
    server_url: str,
    admin_token: str,
) -> None:
    created = _create_series(server_url, admin_token, f"Patch {time.time_ns()}")
    assert created.status_code == 201, created.text
    series_id = created.json()["id"]

    response = requests.patch(
        f"{server_url}{SERIES_PATH}/{series_id}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={
            "name": "Corrected Show",
            "first_air_date": "1990-04-08",
            "country_of_origin": "us",
        },
        timeout=10,
    )

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["name"] == "Corrected Show"
    assert body["first_air_date"] == "1990-04-08"
    assert body["country_of_origin"] == "US"
    assert body["seasons"] == []

    cleared = requests.patch(
        f"{server_url}{SERIES_PATH}/{series_id}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"first_air_date": None, "country_of_origin": None},
        timeout=10,
    )

    assert cleared.status_code == 200, cleared.text
    assert cleared.json()["first_air_date"] is None
    assert cleared.json()["country_of_origin"] is None


def test_creating_a_show_with_an_empty_name_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = _create_series(server_url, admin_token, "")

    assert_error_body(response, 422)


def test_creating_a_show_with_a_malformed_first_air_date_is_rejected(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{SERIES_PATH}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Twin Peaks", "first_air_date": "08/04/1990"},
        timeout=10,
    )

    assert_error_body(response, 422)


def test_patching_an_unknown_show_is_not_found(
    server_url: str,
    admin_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.patch(
        f"{server_url}{LIBRARY_SERIES_PATH}/{UNKNOWN_ID}",
        headers={"Authorization": f"Bearer {admin_token}"},
        json={"name": "Another Name"},
        timeout=10,
    )

    assert_error_body(response, 404)
