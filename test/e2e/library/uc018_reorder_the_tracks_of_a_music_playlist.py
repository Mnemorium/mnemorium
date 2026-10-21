"""E2E tests for UC-018 - Reorder the Tracks of a Music Playlist.

Music fixtures are absent on the E2E server, so the empty-playlist success and
the invalid-order, visibility and malformed-identifier flows run against the
running container.
"""

import time
from collections.abc import Callable

import pytest
import requests

_PLAYLIST_PATH = "/api/v1/library/music/playlist"


def test_reordering_requires_a_valid_token(server_url: str) -> None:
    response = requests.put(
        f"{server_url}{_PLAYLIST_PATH}/1/track",
        json={"order": []},
        timeout=10,
    )

    assert response.status_code == 401


def test_reordering_an_empty_playlist_is_accepted(
    create_playlist: Callable[[str, str, bool], requests.Response],
    reorder_playlist_tracks: Callable[[str, int, list[int]], requests.Response],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Empty order {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = reorder_playlist_tracks(standard_user_token, created.json()["id"], [])

    assert response.status_code == 200, f"reorder failed: {response.status_code} {response.text}"
    assert response.json()["tracks"] == []


@pytest.mark.parametrize("order", [[0], [1, 0], [0, 0]])
def test_an_invalid_order_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    reorder_playlist_tracks: Callable[[str, int, list[int]], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    order: list[int],
) -> None:
    created = create_playlist(standard_user_token, f"Invalid order {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = reorder_playlist_tracks(standard_user_token, created.json()["id"], order)

    assert_error_body(response, 422)


def test_reordering_an_invisible_playlist_answers_not_found(
    create_playlist: Callable[[str, str, bool], requests.Response],
    reorder_playlist_tracks: Callable[[str, int, list[int]], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Invisible {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = reorder_playlist_tracks(second_standard_user_token, created.json()["id"], [])

    assert_error_body(response, 404)


@pytest.mark.parametrize(
    ("playlist_id", "payload"),
    [
        ("abc", {"order": []}),
        ("1", {"order": "not an array"}),
        ("1", {"order": ["abc"]}),
    ],
)
def test_a_malformed_request_is_rejected(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
    playlist_id: str,
    payload: dict,
) -> None:
    response = requests.put(
        f"{server_url}{_PLAYLIST_PATH}/{playlist_id}/track",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        json=payload,
        timeout=10,
    )

    expected = 400 if playlist_id == "abc" else 422
    assert_error_body(response, expected)
