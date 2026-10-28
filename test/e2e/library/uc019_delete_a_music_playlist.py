"""E2E tests for UC-019 - Delete a Music Playlist.

The owner deletion, invisibility, non-owner and malformed-identifier alternative
flows run against the running container.
"""

import time
from collections.abc import Callable

import pytest
import requests

_PLAYLIST_PATH = "/api/v1/library/music/playlist"


def test_deleting_a_playlist_requires_a_valid_token(server_url: str) -> None:
    response = requests.delete(f"{server_url}{_PLAYLIST_PATH}/1", timeout=10)

    assert response.status_code == 401


def test_the_owner_deletes_a_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    delete_playlist: Callable[[str, int], requests.Response],
    get_playlist: Callable[[str, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Deleted {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    playlist_id = created.json()["id"]

    deleted = delete_playlist(standard_user_token, playlist_id)

    assert deleted.status_code == 204, f"delete failed: {deleted.status_code} {deleted.text}"
    assert_error_body(get_playlist(standard_user_token, playlist_id), 404)


def test_a_visible_non_owner_cannot_delete_a_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    delete_playlist: Callable[[str, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Public {time.time_ns()}", is_public=True)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = delete_playlist(second_standard_user_token, created.json()["id"])

    assert_error_body(response, 403)


def test_root_admin_deletes_another_users_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    delete_playlist: Callable[[str, int], requests.Response],
    get_playlist: Callable[[str, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    root_admin_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Root {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    playlist_id = created.json()["id"]

    response = delete_playlist(root_admin_token, playlist_id)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    assert_error_body(get_playlist(standard_user_token, playlist_id), 404)


def test_an_invisible_playlist_answers_not_found(
    create_playlist: Callable[[str, str, bool], requests.Response],
    delete_playlist: Callable[[str, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Invisible {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = delete_playlist(second_standard_user_token, created.json()["id"])

    assert_error_body(response, 404)


@pytest.mark.parametrize(("playlist_id", "expected"), [("abc", 400), ("-1", 404)])
def test_a_malformed_identifier_is_rejected(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
    playlist_id: str,
    expected: int,
) -> None:
    response = requests.delete(
        f"{server_url}{_PLAYLIST_PATH}/{playlist_id}",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert_error_body(response, expected)
