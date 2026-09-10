"""E2E tests for UC-002 - User Authentication."""

from collections.abc import Callable

import requests

_WRONG_PASSWORD = "Wr0ng!pass"

_LOGIN_PATH = "/api/v1/identity/login"

_STANDARD_USER_USERNAME = "alice"
_STANDARD_USER_PASSWORD = "S3cret!pass"
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

    # The provisioned token must authenticate for the same user.
    response = me(standard_user_token)
    assert response.json()["username"] == _STANDARD_USER_USERNAME


def test_uc002_user_authentication_wrong_password(
    server_url: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{_LOGIN_PATH}",
        json={"username": _STANDARD_USER_USERNAME, "password": _WRONG_PASSWORD},
        timeout=10,
    )
    assert_error_body(response, 401)
