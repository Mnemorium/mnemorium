import time
from collections.abc import Callable

import requests

_VALID_PASSWORD = "S3cret!pass"
_VALID_ROLE = "STANDARD"

FORBIDDEN_ADMIN_USERNAME = "stuart"
FORBIDDEN_ADMIN_EMAIL = "stuart@example.com"
FORBIDDEN_ADMIN_PASSWORD = "Stuart!pass"
FORBIDDEN_ADMIN_ROLE = "ADMIN"

# Payloads that violate UC-001 business rules; every other field is valid.
INVALID_PAYLOADS = [
    # Empty username: violates the username minLength (4) rule.
    {"username": "", "email": None, "password": _VALID_PASSWORD, "role": _VALID_ROLE},
    # 7-char password (contains a symbol): violates the password minLength (8) rule.
    {"username": "carol", "email": "carol@example.com", "password": "short1!", "role": _VALID_ROLE},
    # 11-char password without a symbol: violates the password pattern "[^A-Za-z0-9]".
    {"username": "dave", "email": None, "password": "longpassword", "role": _VALID_ROLE},
]


def test_uc001_create_user_account_happy_path(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
) -> None:
    username = f"alice-{time.time_ns()}"
    email = f"{username}@example.com"

    response = register(root_admin_token, username, email, _VALID_PASSWORD, _VALID_ROLE)
    assert response.status_code == 201, f"register failed: {response.status_code} {response.text}"
    assert response.headers.get("Location")
    assert response.headers.get("Content-Type", "").startswith("application/json")
    body = response.json()
    assert body["username"] == username
    assert body["role"] == _VALID_ROLE
    assert body["email"] == email
    assert isinstance(body["id"], int) and body["id"] > 0


def test_uc001_create_user_account_invalid_payload(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    for payload in INVALID_PAYLOADS:
        response = register(root_admin_token, **payload)
        assert_error_body(response, 400)


def test_uc001_create_user_account_admin_role_forbidden(
    server_url: str,
    admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    # An Admin who is not the Root Admin may not grant the ADMIN role.
    response = register(
        admin_token,
        FORBIDDEN_ADMIN_USERNAME,
        FORBIDDEN_ADMIN_EMAIL,
        FORBIDDEN_ADMIN_PASSWORD,
        FORBIDDEN_ADMIN_ROLE,
    )
    assert_error_body(response, 403)

    # Post-check: the rejected user was never created.
    response = requests.post(
        f"{server_url}/api/v1/identity/login",
        json={"username": FORBIDDEN_ADMIN_USERNAME, "password": FORBIDDEN_ADMIN_PASSWORD},
        timeout=10,
    )

    assert_error_body(response, 401)
