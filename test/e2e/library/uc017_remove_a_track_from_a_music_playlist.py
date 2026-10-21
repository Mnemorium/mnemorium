"""E2E tests for UC-017 - Remove a Track from a Music Playlist.

Music fixtures are absent on the E2E server, so only the missing-track,
visibility and malformed-index alternative flows can run against the running
container.
"""

import time
from collections.abc import Callable

import pytest
import requests

_PLAYLIST_PATH = "/api/v1/library/music/playlist"


def test_removing_a_track_requires_a_valid_token(server_url: str) -> None:
    response = requests.delete(
        f"{server_url}{_PLAYLIST_PATH}/1/track/0",
        timeout=10,
    )

    assert response.status_code == 401


def test_removing_a_missing_track_answers_not_found(
    create_playlist: Callable[[str, str, bool], requests.Response],
    delete_playlist_track: Callable[[str, int, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Remove {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = delete_playlist_track(standard_user_token, created.json()["id"], 0)

    assert_error_body(response, 404)


def test_removing_a_track_from_an_invisible_playlist_answers_not_found(
    create_playlist: Callable[[str, str, bool], requests.Response],
    delete_playlist_track: Callable[[str, int, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Invisible {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = delete_playlist_track(second_standard_user_token, created.json()["id"], 0)

    assert_error_body(response, 404)


@pytest.mark.parametrize(
    ("playlist_id", "track_index"),
    [("abc", "0"), ("1", "abc"), ("1", "-1")],
)
def test_a_malformed_identifier_is_rejected(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
    playlist_id: str,
    track_index: str,
) -> None:
    response = requests.delete(
        f"{server_url}{_PLAYLIST_PATH}/{playlist_id}/track/{track_index}",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert_error_body(response, 400)
