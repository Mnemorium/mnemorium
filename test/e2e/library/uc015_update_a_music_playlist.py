"""E2E tests for UC-015 - Update a Music Playlist.

The metadata update, empty update, ownership, naming and malformed-identifier
alternative flows run against the running container.
"""

import time
from collections.abc import Callable

import pytest
import requests

_PLAYLIST_PATH = "/api/v1/library/music/playlist"

# The playlist name business rule caps the name at 100 characters.
_MAX_NAME_LENGTH = 100


def test_updating_a_playlist_requires_a_valid_token(server_url: str) -> None:
    response = requests.patch(
        f"{server_url}{_PLAYLIST_PATH}/1",
        json={"name": "Unauthenticated"},
        timeout=10,
    )

    assert response.status_code == 401


def test_update_a_playlist_renames_and_sets_visibility(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Mine {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    name = f"Renamed {time.time_ns()}"

    response = patch_playlist(
        standard_user_token,
        created.json()["id"],
        {"name": name, "is_public": True},
    )

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["name"] == name
    assert body["is_public"] is True
    assert body["_links"]["self"]["href"] == f"{_PLAYLIST_PATH}/{body['id']}"


def test_patch_with_an_empty_body_returns_the_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Empty body {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = patch_playlist(standard_user_token, created.json()["id"], {})

    assert response.status_code == 200
    assert response.json()["name"] == created.json()["name"]


def test_a_non_owner_cannot_update_metadata(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Public {time.time_ns()}", is_public=True)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = patch_playlist(
        second_standard_user_token,
        created.json()["id"],
        {"name": f"Stolen {time.time_ns()}"},
    )

    assert_error_body(response, 403)


def test_root_admin_cannot_update_another_users_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    root_admin_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Root {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = patch_playlist(
        root_admin_token,
        created.json()["id"],
        {"is_public": True},
    )

    assert_error_body(response, 403)


def test_a_duplicate_name_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
) -> None:
    first = create_playlist(standard_user_token, f"First {time.time_ns()}")
    assert first.status_code == 201, f"create failed: {first.status_code} {first.text}"
    second = create_playlist(standard_user_token, f"Second {time.time_ns()}")
    assert second.status_code == 201, f"create failed: {second.status_code} {second.text}"

    response = patch_playlist(
        standard_user_token,
        second.json()["id"],
        {"name": first.json()["name"]},
    )

    assert_error_body(response, 409)


@pytest.mark.parametrize("name", ["", "   ", "x" * (_MAX_NAME_LENGTH + 1)])
def test_an_invalid_name_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    name: str,
) -> None:
    created = create_playlist(standard_user_token, f"Invalid {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = patch_playlist(standard_user_token, created.json()["id"], {"name": name})

    assert_error_body(response, 422)


def test_an_invisible_playlist_answers_not_found(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Invisible {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = patch_playlist(
        second_standard_user_token,
        created.json()["id"],
        {"name": f"Renamed {time.time_ns()}"},
    )

    assert_error_body(response, 404)
