"""E2E tests for UC-016 - Add a Track to a Music Playlist.

Music fixtures are absent on the E2E server, so the happy path stays in the
adapter and application tests. The unknown-recording, mixed-authorization,
visibility and malformed-body flows run against the running container.
"""

import time
from collections.abc import Callable

import pytest
import requests

_PLAYLIST_PATH = "/api/v1/library/music/playlist"


def test_adding_a_track_requires_a_valid_token(server_url: str) -> None:
    response = requests.patch(
        f"{server_url}{_PLAYLIST_PATH}/1",
        json={"add_track": {"recording_id": 1}},
        timeout=10,
    )

    assert response.status_code == 401


def test_an_unknown_recording_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Add {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = patch_playlist(
        standard_user_token,
        created.json()["id"],
        {"add_track": {"recording_id": 999_999_999}},
    )

    assert_error_body(response, 422)


def test_a_non_owner_mixed_request_rejects_both_changes(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    get_playlist: Callable[[str, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Mixed {time.time_ns()}", is_public=True)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    playlist_id = created.json()["id"]

    response = patch_playlist(
        second_standard_user_token,
        playlist_id,
        {"name": f"Stolen {time.time_ns()}", "add_track": {"recording_id": 999_999_999}},
    )

    assert_error_body(response, 403)
    unchanged = get_playlist(standard_user_token, playlist_id)
    assert unchanged.status_code == 200
    assert unchanged.json()["name"] == created.json()["name"]
    assert unchanged.json()["tracks"] == []


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
        {"add_track": {"recording_id": 1}},
    )

    assert_error_body(response, 404)


@pytest.mark.parametrize(
    "payload",
    [
        {"add_track": {"recording_id": "abc"}},
        {"add_track": "not an object"},
    ],
)
def test_a_malformed_add_track_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    patch_playlist: Callable[[str, int, dict], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    payload: dict,
) -> None:
    created = create_playlist(standard_user_token, f"Malformed {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = patch_playlist(standard_user_token, created.json()["id"], payload)

    assert_error_body(response, 422)


def test_a_malformed_playlist_identifier_is_rejected(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.patch(
        f"{server_url}{_PLAYLIST_PATH}/abc",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        json={"add_track": {"recording_id": 1}},
        timeout=10,
    )

    assert_error_body(response, 400)
