import time
from collections.abc import Callable

import requests
from conftest import REGISTER_PATH, SECRET_PASSWORD

_VALID_ROLE = "STANDARD"

FORBIDDEN_ADMIN_USERNAME = "stuart"
FORBIDDEN_ADMIN_EMAIL = "stuart@example.com"
FORBIDDEN_ADMIN_PASSWORD = SECRET_PASSWORD
FORBIDDEN_ADMIN_ROLE = "ADMIN"

# Payloads that violate UC-001 business rules; every other field is valid.
INVALID_PAYLOADS = [
    # Empty username: violates the username minimum length (4) rule.
    {"username": "", "email": None, "password": SECRET_PASSWORD, "role": _VALID_ROLE},
    # Uppercase letter: the username must be lowercase.
    {"username": "Alice", "email": None, "password": SECRET_PASSWORD, "role": _VALID_ROLE},
    # Leading whitespace: the username must not begin with whitespace.
    {"username": " alice", "email": None, "password": SECRET_PASSWORD, "role": _VALID_ROLE},
    # Trailing whitespace: the username must not end with whitespace.
    {"username": "alice ", "email": None, "password": SECRET_PASSWORD, "role": _VALID_ROLE},
    # 101 characters: violates the username maximum length (100) rule.
    {"username": "x" * 101, "email": None, "password": SECRET_PASSWORD, "role": _VALID_ROLE},
    # 7-char password with a symbol: violates the password minimum length (8) rule.
    {"username": "carol", "email": "carol@example.com", "password": "short1!", "role": _VALID_ROLE},
    # 11-char password without a symbol: violates the password pattern "[^A-Za-z0-9]".
    {"username": "dave", "email": None, "password": "longpassword", "role": _VALID_ROLE},
    # Display-name mailbox: the email field holds an address, not a mailbox.
    {
        "username": "heidi",
        "email": "Alice <alice@example.com>",
        "password": SECRET_PASSWORD,
        "role": _VALID_ROLE,
    },
    # Padded email: the parser rejects surrounding whitespace.
    {
        "username": "ivan",
        "email": " alice@example.com ",
        "password": SECRET_PASSWORD,
        "role": _VALID_ROLE,
    },
]


def test_uc001_create_user_account_happy_path(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
) -> None:
    username = f"alice-{time.time_ns()}"
    email = f"{username}@example.com"

    response = register(root_admin_token, username, email, SECRET_PASSWORD, _VALID_ROLE)
    assert response.status_code == 201, f"register failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert body["username"] == username
    assert body["role"] == _VALID_ROLE
    assert body["email"] == email
    assert isinstance(body["id"], int) and body["id"] > 0
    self_href = f"/api/v1/user/{body['id']}"
    assert body["_links"]["self"]["href"] == self_href
    assert response.headers.get("Location") == self_href


def test_uc001_create_user_account_without_email(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
) -> None:
    username = f"noemail-{time.time_ns()}"

    response = register(root_admin_token, username, None, SECRET_PASSWORD, _VALID_ROLE)

    assert response.status_code == 201, f"register failed: {response.status_code} {response.text}"
    assert response.json()["email"] is None


def test_uc001_create_user_account_username_length_boundaries(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
) -> None:
    shortest = f"a{time.time_ns() % 1000:03d}"
    longest = "x" * 90 + f"{time.time_ns():019d}"[-10:]

    for username in (shortest, longest):
        response = register(root_admin_token, username, None, SECRET_PASSWORD, _VALID_ROLE)
        assert response.status_code == 201, f"register failed: {response.status_code} {response.text}"
        assert response.json()["username"] == username


def test_uc001_create_user_account_invalid_payload(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    for payload in INVALID_PAYLOADS:
        response = register(root_admin_token, **payload)
        assert_error_body(response, 422)


def test_uc001_create_user_account_duplicate_username(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    username = f"duplicate-{time.time_ns()}"

    first = register(root_admin_token, username, None, SECRET_PASSWORD, _VALID_ROLE)
    assert first.status_code == 201, f"register failed: {first.status_code} {first.text}"

    second = register(root_admin_token, username, None, SECRET_PASSWORD, _VALID_ROLE)
    assert_error_body(second, 409)
    assert second.json()["error"] == "username is already taken"


def test_uc001_create_user_account_duplicate_email(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    suffix = time.time_ns()
    first_username = f"mail-{suffix}"
    second_username = f"mail2-{suffix}"
    email = f"{first_username}@example.com"

    first = register(root_admin_token, first_username, email, SECRET_PASSWORD, _VALID_ROLE)
    assert first.status_code == 201, f"register failed: {first.status_code} {first.text}"

    second = register(root_admin_token, second_username, email, SECRET_PASSWORD, _VALID_ROLE)
    assert_error_body(second, 409)
    assert second.json()["error"] == "email is already taken"


def test_uc001_create_user_account_unauthenticated(
    server_url: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = requests.post(
        f"{server_url}{REGISTER_PATH}",
        json={"username": "nobody", "email": None, "password": SECRET_PASSWORD, "role": _VALID_ROLE},
        timeout=10,
    )

    assert_error_body(response, 401)


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
