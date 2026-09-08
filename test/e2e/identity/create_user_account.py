import requests

API_PREFIX = "/api/v1"
LOGIN_PATH = f"{API_PREFIX}/identity/login"
REGISTER_PATH = f"{API_PREFIX}/identity/register"

ROOT_ADMIN_USERNAME = "root"
NEW_USER_USERNAME = "alice"
NEW_USER_EMAIL = "alice@example.com"
NEW_USER_PASSWORD = "S3cret!pass"
NEW_USER_ROLE = "STANDARD"

NON_ROOT_ADMIN_USERNAME = "mallory"
NON_ROOT_ADMIN_EMAIL = "mallory@example.com"
NON_ROOT_ADMIN_PASSWORD = "Mallory!pass"
NON_ROOT_ADMIN_ROLE = "ADMIN"

FORBIDDEN_ADMIN_USERNAME = "stuart"
FORBIDDEN_ADMIN_EMAIL = "stuart@example.com"
FORBIDDEN_ADMIN_PASSWORD = "Stuart!pass"

# Payloads that violate UC-001 business rules; every other field is valid.
INVALID_PAYLOADS = [
    # Empty username: violates the username minLength (4) rule.
    {"username": "", "email": None, "password": NEW_USER_PASSWORD, "role": NEW_USER_ROLE},
    # 7-char password (contains a symbol): violates the password minLength (8) rule.
    {"username": "carol", "email": "carol@example.com", "password": "short1!", "role": NEW_USER_ROLE},
    # 11-char password without a symbol: violates the password pattern "[^A-Za-z0-9]".
    {"username": "dave", "email": None, "password": "longpassword", "role": NEW_USER_ROLE},
]


def _login(base_url: str, username: str, password: str) -> str:
    """Log in and return the access token."""
    response = requests.post(
        f"{base_url}{LOGIN_PATH}",
        json={"username": username, "password": password},
        timeout=10,
    )
    assert response.status_code == 200, f"login failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["token_type"] == "bearer"
    assert isinstance(body["access_token"], str) and body["access_token"]
    assert isinstance(body["expires_in"], int) and body["expires_in"] >= 0
    return body["access_token"]


def _register(
    base_url: str, token: str, username: str, email: str | None, password: str, role: str
) -> requests.Response:
    """Register a user and return the raw response, without asserting the status."""
    return requests.post(
        f"{base_url}{REGISTER_PATH}",
        headers={"Authorization": f"Bearer {token}"},
        json={"username": username, "email": email, "password": password, "role": role},
        timeout=10,
    )


def _assert_error_body(response: requests.Response, status_code: int) -> None:
    """Assert the exact ErrorBody shape declared in the OpenAPI spec."""
    assert response.status_code == status_code, f"expected {status_code}, got {response.status_code}: {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/json")
    body = response.json()
    assert set(body.keys()) == {"error"}
    assert isinstance(body["error"], str) and body["error"]


def test_uc001_create_user_account_happy_path(server_url: str, default_password: str) -> None:
    token = _login(server_url, ROOT_ADMIN_USERNAME, default_password)

    response = requests.post(
        f"{server_url}{REGISTER_PATH}",
        headers={"Authorization": f"Bearer {token}"},
        json={
            "username": NEW_USER_USERNAME,
            "email": NEW_USER_EMAIL,
            "password": NEW_USER_PASSWORD,
            "role": NEW_USER_ROLE,
        },
        timeout=10,
    )
    assert response.status_code == 201, f"register failed: {response.status_code} {response.text}"
    assert response.headers.get("Location")
    assert response.headers.get("Content-Type", "").startswith("application/json")
    body = response.json()
    assert body["username"] == NEW_USER_USERNAME
    assert body["role"] == NEW_USER_ROLE
    assert body["email"] == NEW_USER_EMAIL
    assert isinstance(body["id"], int) and body["id"] > 0


def test_uc001_create_user_account_invalid_payload(server_url: str, default_password: str) -> None:
    token = _login(server_url, ROOT_ADMIN_USERNAME, default_password)

    for payload in INVALID_PAYLOADS:
        response = _register(server_url, token, **payload)
        _assert_error_body(response, 400)


def test_uc001_create_user_account_admin_role_forbidden(server_url: str, default_password: str) -> None:
    root_token = _login(server_url, ROOT_ADMIN_USERNAME, default_password)

    # Provision the non-root Admin caller: only the Root Admin can register an ADMIN user.
    response = _register(
        server_url,
        root_token,
        NON_ROOT_ADMIN_USERNAME,
        NON_ROOT_ADMIN_EMAIL,
        NON_ROOT_ADMIN_PASSWORD,
        NON_ROOT_ADMIN_ROLE,
    )
    assert response.status_code == 201, f"provision admin failed: {response.status_code} {response.text}"
    assert response.headers.get("Location")
    assert response.headers.get("Content-Type", "").startswith("application/json")
    body = response.json()
    assert body["role"] == NON_ROOT_ADMIN_ROLE
    assert isinstance(body["id"], int) and body["id"] > 0

    admin_token = _login(server_url, NON_ROOT_ADMIN_USERNAME, NON_ROOT_ADMIN_PASSWORD)

    # An Admin who is not the Root Admin may not grant the ADMIN role.
    response = _register(
        server_url,
        admin_token,
        FORBIDDEN_ADMIN_USERNAME,
        FORBIDDEN_ADMIN_EMAIL,
        FORBIDDEN_ADMIN_PASSWORD,
        NON_ROOT_ADMIN_ROLE,
    )
    _assert_error_body(response, 403)

    # Post-check: the rejected user was never created.
    response = requests.post(
        f"{server_url}{LOGIN_PATH}",
        json={"username": FORBIDDEN_ADMIN_USERNAME, "password": FORBIDDEN_ADMIN_PASSWORD},
        timeout=10,
    )
    _assert_error_body(response, 401)
