"""E2E tests for UC-013 - Create a Music Playlist.

The happy path and every alternative flow run against the running container.
"""

import time
from collections.abc import Callable

import pytest
import requests

_PLAYLIST_PATH = "/api/v1/library/music/playlist"

# The playlist name business rule caps the name at 100 characters.
_MAX_NAME_LENGTH = 100


def test_playlist_creation_requires_a_valid_token(server_url: str) -> None:
    response = requests.post(
        f"{server_url}{_PLAYLIST_PATH}",
        json={"name": "Unauthenticated"},
        timeout=10,
    )

    assert response.status_code == 401


def test_create_a_playlist_returns_the_created_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    me: Callable[[str], requests.Response],
) -> None:
    name = f"Late night {time.time_ns()}"

    response = create_playlist(standard_user_token, name)

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert body["name"] == name
    assert isinstance(body["id"], int) and body["id"] > 0
    assert body["is_public"] is False
    assert body["track_count"] == 0
    assert body["tracks"] == []
    caller = me(standard_user_token)
    assert caller.status_code == 200
    assert body["owner_id"] == caller.json()["id"]
    self_href = f"{_PLAYLIST_PATH}/{body['id']}"
    assert body["_links"]["self"]["href"] == self_href
    assert body["_links"]["tracks"]["href"] == f"{self_href}/track"
    assert response.headers.get("Location") == self_href


def test_create_a_playlist_trims_the_name(
    create_playlist: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
) -> None:
    name = f"  Trimmed {time.time_ns()}  "

    response = create_playlist(standard_user_token, name)

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    assert response.json()["name"] == name.strip()


def test_create_a_public_playlist_marks_it_public(
    create_playlist: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
) -> None:
    response = create_playlist(standard_user_token, f"Public {time.time_ns()}", is_public=True)

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    assert response.json()["is_public"] is True


@pytest.mark.parametrize("name", ["", "   "])
def test_create_a_playlist_with_an_empty_name_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
    name: str,
) -> None:
    response = create_playlist(standard_user_token, name)

    assert_error_body(response, 422)


def test_create_a_playlist_with_a_too_long_name_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = create_playlist(standard_user_token, "x" * (_MAX_NAME_LENGTH + 1))

    assert_error_body(response, 422)


def test_create_a_playlist_with_an_existing_name_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    name = f"Duplicate {time.time_ns()}"
    created = create_playlist(standard_user_token, name)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = create_playlist(standard_user_token, name)

    assert_error_body(response, 409)
