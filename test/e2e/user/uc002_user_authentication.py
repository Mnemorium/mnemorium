"""E2E tests for UC-002 - User Authentication."""

from collections.abc import Callable
from uuid import uuid4

import requests
from conftest import SECRET_PASSWORD

_WRONG_PASSWORD = "Wr0ng!pass"

_LOGIN_PATH = "/api/v1/identity/login"

_STANDARD_USER_USERNAME = "alice"
_STANDARD_USER_PASSWORD = SECRET_PASSWORD
_STANDARD_USER_ROLE = "STANDARD"


def test_uc002_user_authentication_happy_path(
    server_url: str,
    standard_user_token: str,
    me: Callable[[str], requests.Response],
) -> None:
    response = requests.post(
        f"{server_url}{_LOGIN_PATH}",
        json={"username": _STANDARD_USER_USERNAME, "password": _STANDARD_USER_PASSWORD},
        timeout=10,
    )
    assert response.status_code == 200, f"login failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/json")
    body = response.json()
    assert set(body.keys()) == {"token_type", "access_token", "expires_in"}
    assert body["token_type"] == "bearer"
    assert isinstance(body["access_token"], str) and body["access_token"]
    assert isinstance(body["expires_in"], int) and body["expires_in"] >= 0

    # Post condition: user holds a valid token granting access to authorized
    # features and resources.
    response = me(body["access_token"])
    assert response.status_code == 200, f"me failed: {response.status_code} {response.text}"
    profile = response.json()
    assert isinstance(profile["id"], int) and profile["id"] > 0
    assert profile["username"] == _STANDARD_USER_USERNAME
    assert profile["role"] == _STANDARD_USER_ROLE
    assert profile["email"] == "alice@example.com"
    assert profile["_links"]["self"]["href"] == f"/api/v1/user/{profile['id']}"

    # The provisioned token must authenticate for the same user.
    response = me(standard_user_token)
    assert response.json()["username"] == _STANDARD_USER_USERNAME


def test_uc002_user_authentication_wrong_password(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    # Depends on the fixture so "alice" exists and this exercises the
    # wrong-password branch rather than the unknown-username branch.
    response = requests.post(
        f"{server_url}{_LOGIN_PATH}",
        json={"username": _STANDARD_USER_USERNAME, "password": _WRONG_PASSWORD},
        timeout=10,
    )
    assert_error_body(response, 401)


def test_uc002_user_authentication_bad_credentials_are_indistinguishable(
    server_url: str,
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    # An unknown account and a wrong password must be indistinguishable to an
    # anonymous caller: same status and the same error message.
    wrong_password = requests.post(
        f"{server_url}{_LOGIN_PATH}",
        json={"username": _STANDARD_USER_USERNAME, "password": _WRONG_PASSWORD},
        timeout=10,
    )
    unknown_username = requests.post(
        f"{server_url}{_LOGIN_PATH}",
        json={"username": f"ghost-{uuid4().hex}", "password": _WRONG_PASSWORD},
        timeout=10,
    )

    assert_error_body(wrong_password, 401)
    assert_error_body(unknown_username, 401)
    assert wrong_password.json()["error"] == unknown_username.json()["error"]
