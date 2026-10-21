"""E2E tests for UC-014 - Browse Music Playlists.

The collection, detail, track alias, one-track read and every visibility and
malformed-identifier alternative flow run against the running container.
"""

import time
from collections.abc import Callable
from urllib.parse import quote

import pytest
import requests

_PLAYLIST_PATH = "/api/v1/library/music/playlist"

# An identifier no playlist ever carries.
_UNKNOWN_ID = 999_999_999


def test_browsing_playlists_requires_a_valid_token(server_url: str) -> None:
    response = requests.get(f"{server_url}{_PLAYLIST_PATH}", timeout=10)

    assert response.status_code == 401


def test_list_playlists_returns_the_callers_private_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    list_playlists: Callable[[str, str], requests.Response],
    standard_user_token: str,
) -> None:
    name = f"Listed {time.time_ns()}"
    created = create_playlist(standard_user_token, name)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = list_playlists(standard_user_token, f"?name={quote(name, safe='')}")

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert [item["id"] for item in body["items"]] == [created.json()["id"]]
    assert body["items"][0]["track_count"] == 0
    assert body["_links"]["self"]["href"].startswith(f"{_PLAYLIST_PATH}?name={quote(name, safe='')}")
    assert body["_links"]["search"]["templated"] is True


def test_get_a_playlist_returns_its_metadata_and_tracks(
    create_playlist: Callable[[str, str, bool], requests.Response],
    get_playlist: Callable[[str, int], requests.Response],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Detail {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = get_playlist(standard_user_token, created.json()["id"])

    assert response.status_code == 200, f"get failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["id"] == created.json()["id"]
    assert body["tracks"] == []
    assert body["track_count"] == 0


def test_the_track_alias_returns_the_playlist_with_a_canonical_self(
    create_playlist: Callable[[str, str, bool], requests.Response],
    get_playlist_track_list: Callable[[str, int], requests.Response],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Alias {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    playlist_id = created.json()["id"]

    response = get_playlist_track_list(standard_user_token, playlist_id)

    assert response.status_code == 200, f"alias failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["tracks"] == []
    assert body["_links"]["self"]["href"] == f"{_PLAYLIST_PATH}/{playlist_id}"


def test_a_private_playlist_is_invisible_to_another_caller(
    create_playlist: Callable[[str, str, bool], requests.Response],
    get_playlist: Callable[[str, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Private {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = get_playlist(second_standard_user_token, created.json()["id"])

    assert_error_body(response, 404)


def test_a_public_playlist_is_readable_by_another_caller(
    create_playlist: Callable[[str, str, bool], requests.Response],
    get_playlist: Callable[[str, int], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Public {time.time_ns()}", is_public=True)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = get_playlist(second_standard_user_token, created.json()["id"])

    assert response.status_code == 200, f"get failed: {response.status_code} {response.text}"


def test_root_admin_reads_another_users_private_playlist(
    create_playlist: Callable[[str, str, bool], requests.Response],
    get_playlist: Callable[[str, int], requests.Response],
    standard_user_token: str,
    root_admin_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"Root read {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = get_playlist(root_admin_token, created.json()["id"])

    assert response.status_code == 200, f"get failed: {response.status_code} {response.text}"


def test_reading_an_unknown_track_is_not_found(
    create_playlist: Callable[[str, str, bool], requests.Response],
    get_playlist_track: Callable[[str, int, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
    standard_user_token: str,
) -> None:
    created = create_playlist(standard_user_token, f"No track {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = get_playlist_track(standard_user_token, created.json()["id"], 0)

    assert_error_body(response, 404)


@pytest.mark.parametrize(
    ("path", "expected"),
    [
        ("/abc", 400),
        ("/abc/track", 400),
        ("/abc/track/0", 400),
        (f"/{_UNKNOWN_ID}", 404),
        (f"/{_UNKNOWN_ID}/track", 404),
    ],
)
def test_malformed_and_unknown_playlist_identifiers_are_rejected(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
    path: str,
    expected: int,
) -> None:
    response = requests.get(
        f"{server_url}{_PLAYLIST_PATH}{path}",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert_error_body(response, expected)


def test_a_malformed_track_index_is_rejected(
    create_playlist: Callable[[str, str, bool], requests.Response],
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    created = create_playlist(standard_user_token, f"Bad index {time.time_ns()}")
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = requests.get(
        f"{server_url}{_PLAYLIST_PATH}/{created.json()['id']}/track/abc",
        headers={"Authorization": f"Bearer {standard_user_token}"},
        timeout=10,
    )

    assert_error_body(response, 400)
