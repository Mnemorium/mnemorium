import time
from collections.abc import Callable

import requests
from conftest import SECRET_PASSWORD

_CATALOG_READ = "catalog.read"
_GALLERY_READ = "gallery.read"
_PLAYLIST_READ = "playlist.read"
_USER_CREATE = "user.create"
_STANDARD = "STANDARD"


def _user_id(me: Callable[[str], requests.Response], token: str) -> int:
    response = me(token)
    assert response.status_code == 200, f"me failed: {response.status_code} {response.text}"
    return response.json()["id"]


def _default(body: dict, action: str) -> dict:
    return next(entry for entry in body["defaults"] if entry["action"] == action)


def _override(body: dict, action: str) -> dict | None:
    return next((entry for entry in body["overrides"] if entry["action"] == action), None)


def _reset(put_permission: Callable[[str, int, list[dict]], requests.Response], token: str, user_id: int) -> None:
    """Replace every override with an empty set, so the role presets apply again."""
    response = put_permission(token, user_id, [])
    assert response.status_code == 200, f"reset failed: {response.status_code} {response.text}"


def test_uc024_permission_round_trip(
    root_admin_token: str,
    standard_user_token: str,
    me: Callable[[str], requests.Response],
    get_permission: Callable[[str, int], requests.Response],
    put_permission: Callable[[str, int, list[dict]], requests.Response],
) -> None:
    user_id = _user_id(me, standard_user_token)
    try:
        read = get_permission(root_admin_token, user_id)
        assert read.status_code == 200, f"read failed: {read.status_code} {read.text}"
        assert read.headers.get("Content-Type", "").startswith("application/hal+json")
        body = read.json()
        assert body["user_id"] == user_id
        assert body["role"] == _STANDARD
        assert body["_links"]["self"]["href"] == f"/api/v1/user/{user_id}/permission"
        assert _default(body, _GALLERY_READ)["scope"] == "PUBLIC_OR_OWNED"
        assert _override(body, _GALLERY_READ) is None

        replaced = put_permission(
            root_admin_token,
            user_id,
            [{"action": _GALLERY_READ, "scope": "OWNED", "resources": []}],
        )
        assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"
        assert _override(replaced.json(), _GALLERY_READ)["scope"] == "OWNED"

        reread = get_permission(root_admin_token, user_id)
        assert _override(reread.json(), _GALLERY_READ)["scope"] == "OWNED"

        _reset(put_permission, root_admin_token, user_id)

        after = get_permission(root_admin_token, user_id)
        assert _default(after.json(), _GALLERY_READ)["scope"] == "PUBLIC_OR_OWNED"
        assert _override(after.json(), _GALLERY_READ) is None
    finally:
        _reset(put_permission, root_admin_token, user_id)


def test_uc024_non_root_cannot_administer_permissions(
    standard_user_token: str,
    me: Callable[[str], requests.Response],
    get_permission: Callable[[str, int], requests.Response],
    put_permission: Callable[[str, int, list[dict]], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    user_id = _user_id(me, standard_user_token)

    assert_error_body(get_permission(standard_user_token, user_id), 403)
    assert_error_body(put_permission(standard_user_token, user_id, []), 403)


def test_uc024_granted_user_creation_lets_a_standard_user_register(
    root_admin_token: str,
    standard_user_token: str,
    me: Callable[[str], requests.Response],
    put_permission: Callable[[str, int, list[dict]], requests.Response],
    register: Callable[[str, str, str | None, str, str], requests.Response],
) -> None:
    user_id = _user_id(me, standard_user_token)
    granted = f"granted-{time.time_ns()}"
    denied = f"denied-{time.time_ns()}"
    try:
        replaced = put_permission(
            root_admin_token,
            user_id,
            [{"action": _USER_CREATE, "scope": "ALL", "resources": []}],
        )
        assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"

        created = register(standard_user_token, granted, None, SECRET_PASSWORD, _STANDARD)
        assert created.status_code == 201, f"register failed: {created.status_code} {created.text}"
        assert created.json()["role"] == _STANDARD
    finally:
        _reset(put_permission, root_admin_token, user_id)

    refused = register(standard_user_token, denied, None, SECRET_PASSWORD, _STANDARD)
    assert refused.status_code == 403, f"expected 403 after reset: {refused.status_code} {refused.text}"


def test_uc024_catalog_none_hides_the_library(
    root_admin_token: str,
    standard_user_token: str,
    me: Callable[[str], requests.Response],
    put_permission: Callable[[str, int, list[dict]], requests.Response],
    list_movies: Callable[[str, str], requests.Response],
    get_movie: Callable[[str, int], requests.Response],
) -> None:
    user_id = _user_id(me, standard_user_token)
    try:
        replaced = put_permission(
            root_admin_token,
            user_id,
            [{"action": _CATALOG_READ, "scope": "NONE", "resources": []}],
        )
        assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"

        restricted = list_movies(standard_user_token)
        assert restricted.status_code == 200
        assert restricted.json()["items"] == []

        permitted = list_movies(root_admin_token)
        assert permitted.status_code == 200
        movies = permitted.json()["items"]
        if movies:
            hidden = get_movie(standard_user_token, movies[0]["id"])
            assert hidden.status_code == 404, f"expected 404: {hidden.status_code} {hidden.text}"
            visible = get_movie(root_admin_token, movies[0]["id"])
            assert visible.status_code == 200
    finally:
        _reset(put_permission, root_admin_token, user_id)


def test_uc024_listed_scope_shares_one_private_playlist(
    root_admin_token: str,
    standard_user_token: str,
    second_standard_user_token: str,
    me: Callable[[str], requests.Response],
    put_permission: Callable[[str, int, list[dict]], requests.Response],
    create_playlist: Callable[[str, str, bool], requests.Response],
    get_playlist: Callable[[str, int], requests.Response],
    delete_playlist: Callable[[str, int], requests.Response],
) -> None:
    bob_id = _user_id(me, second_standard_user_token)
    created = create_playlist(standard_user_token, f"Shared {time.time_ns()}", False)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    playlist_id = created.json()["id"]
    try:
        hidden = get_playlist(second_standard_user_token, playlist_id)
        assert hidden.status_code == 404, f"expected 404: {hidden.status_code} {hidden.text}"

        replaced = put_permission(
            root_admin_token,
            bob_id,
            [
                {
                    "action": _PLAYLIST_READ,
                    "scope": "LISTED",
                    "resources": [{"kind": "MUSIC_PLAYLIST", "id": playlist_id}],
                }
            ],
        )
        assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"
        entry = _override(replaced.json(), _PLAYLIST_READ)
        assert entry["scope"] == "LISTED"
        assert entry["resources"] == [{"id": playlist_id, "kind": "MUSIC_PLAYLIST"}]

        shared = get_playlist(second_standard_user_token, playlist_id)
        assert shared.status_code == 200, f"share failed: {shared.status_code} {shared.text}"
    finally:
        _reset(put_permission, root_admin_token, bob_id)
        delete_playlist(standard_user_token, playlist_id)
