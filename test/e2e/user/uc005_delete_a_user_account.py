"""E2E tests for UC-005 - Delete a User Account.

The happy path and the account refusals run against the running container: an
Admin deletes a Standard account together with its private media, a shared
media row survives under the Root Admin, the deleted account's token and login
are refused, and neither the Root Admin's own account nor an Admin peer is
deletable.
"""

import time
from collections.abc import Callable

import requests
from conftest import (
    API_PREFIX,
    ASSET_FILE_PATH,
    LOGIN_PATH,
    ME_PATH,
    SECRET_PASSWORD,
    create_owned_gallery,
    png,
    register_image,
)

_GALLERY_PATH = f"{API_PREFIX}/library/gallery"


def _provision(
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
    caller_token: str,
    role: str = "STANDARD",
) -> tuple[int, str, str]:
    """Provision a fresh user of `role` and return its id, username and token."""
    username = f"uc005-{time.time_ns()}"
    response = register(
        caller_token,
        username,
        f"{username}@example.com",
        SECRET_PASSWORD,
        role,
    )
    assert response.status_code == 201, f"register failed: {response.status_code} {response.text}"
    return response.json()["id"], username, login(username, SECRET_PASSWORD)


def _get(server_url: str, path: str, token: str) -> requests.Response:
    """GET `path` with the bearer token."""
    return requests.get(
        f"{server_url}{path}",
        headers={"Authorization": f"Bearer {token}"},
        timeout=10,
    )


def test_uc005_admin_deletes_a_standard_account_with_its_private_media(
    server_url: str,
    admin_token: str,
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    delete_user: Callable[[str, int], requests.Response],
) -> None:
    target_id, username, target_token = _provision(register, login, root_admin_token)
    gallery_id = create_owned_gallery(create_gallery, target_token, is_public=False)
    file_id = register_image(
        upload_media,
        register_media,
        target_token,
        f"uc005-private-{time.time_ns()}.png",
        png(3, 3),
    )
    added = add_gallery_item(target_token, gallery_id, "image", file_id)
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    item_id = added.json()["id"]

    deleted = delete_user(admin_token, target_id)

    assert deleted.status_code == 204, f"delete failed: {deleted.status_code} {deleted.text}"
    assert deleted.content == b""
    # The deleted account can no longer use its token or its credentials.
    assert _get(server_url, ME_PATH, target_token).status_code == 401
    refused = requests.post(
        f"{server_url}{LOGIN_PATH}",
        json={"username": username, "password": SECRET_PASSWORD},
        timeout=10,
    )
    assert refused.status_code == 401, f"login was accepted: {refused.status_code} {refused.text}"
    # Its private gallery, its items and their asset are gone.
    assert _get(server_url, f"{_GALLERY_PATH}/{gallery_id}", root_admin_token).status_code == 404
    assert (
        _get(
            server_url,
            f"{_GALLERY_PATH}/{gallery_id}/item/{item_id}",
            root_admin_token,
        ).status_code
        == 404
    )
    assert _get(server_url, f"{ASSET_FILE_PATH}/{file_id}", root_admin_token).status_code == 404


def test_uc005_shared_media_survives_under_the_root_admin(
    server_url: str,
    admin_token: str,
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
    create_gallery: Callable[[str, str, bool], requests.Response],
    upload_media: Callable[[str, str, bytes, str], int],
    register_media: Callable[[str, int], int],
    add_gallery_item: Callable[[str, int, str, int], requests.Response],
    delete_user: Callable[[str, int], requests.Response],
) -> None:
    target_id, _username, target_token = _provision(register, login, root_admin_token)
    file_id = register_image(
        upload_media,
        register_media,
        target_token,
        f"uc005-shared-{time.time_ns()}.png",
        png(3, 3),
    )
    gallery_id = create_owned_gallery(create_gallery, admin_token, is_public=False)
    added = add_gallery_item(admin_token, gallery_id, "image", file_id)
    assert added.status_code == 201, f"add failed: {added.status_code} {added.text}"
    item_id = added.json()["id"]

    deleted = delete_user(admin_token, target_id)

    assert deleted.status_code == 204, f"delete failed: {deleted.status_code} {deleted.text}"
    # The other user's gallery and its item survive.
    stored = _get(server_url, f"{_GALLERY_PATH}/{gallery_id}", admin_token)
    assert stored.status_code == 200, f"gallery failed: {stored.status_code} {stored.text}"
    assert item_id in [item["id"] for item in stored.json()["items"]]
    # The transferred file stays readable, bytes included, for the Root Admin.
    file_response = _get(server_url, f"{ASSET_FILE_PATH}/{file_id}", root_admin_token)
    assert file_response.status_code == 200, f"file failed: {file_response.status_code} {file_response.text}"
    content_href = file_response.json()["_links"]["content"]["href"]
    # The content link carries its own playback credential; sending the bearer
    # token as well is an ambiguous credential and answers `400`.
    content = requests.get(f"{server_url}{content_href}", timeout=10)
    assert content.status_code == 200
    assert content.content.startswith(b"\x89PNG")


def test_uc005_root_admin_and_admin_peer_accounts_are_refused(
    server_url: str,
    root_admin_token: str,
    admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
    delete_user: Callable[[str, int], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    # The Root Admin's own account is never deletable.
    assert_error_body(delete_user(root_admin_token, 0), 403)

    # An Admin cannot delete an Admin peer.
    peer_id, _peer_username, peer_token = _provision(register, login, root_admin_token, "ADMIN")
    assert_error_body(delete_user(admin_token, peer_id), 403)
    assert _get(server_url, ME_PATH, peer_token).status_code == 200

    # An Admin cannot delete its own account either.
    admin_id = _get(server_url, ME_PATH, admin_token).json()["id"]
    assert_error_body(delete_user(admin_token, admin_id), 403)
    assert _get(server_url, ME_PATH, admin_token).status_code == 200
