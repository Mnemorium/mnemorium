import os
import re
import subprocess
import time
from collections.abc import Callable

import pytest
import requests

_BASE_URL = "http://127.0.0.1:4080"
_BASE_URL_ENV = "MNEMORIUM_E2E_BASE_URL"
_CONTAINER = "mnemorium-e2e"
_HEALTH_PATH = "/health"
_HEALTH_TIMEOUT_S = 60
_POLL_INTERVAL_S = 0.5

_DEFAULT_PASSWORD_RE = re.compile(
    r"Root admin initialized; use the default password to authenticate and change it: '([^']+)'"
)

API_PREFIX = "/api/v1"
LOGIN_PATH = f"{API_PREFIX}/identity/login"
REGISTER_PATH = f"{API_PREFIX}/identity/register"

ROOT_ADMIN_USERNAME = "root"

STANDARD_USER_USERNAME = "alice"
STANDARD_USER_EMAIL = "alice@example.com"
STANDARD_USER_PASSWORD = "S3cret!pass"
STANDARD_USER_ROLE = "STANDARD"

ADMIN_USER_USERNAME = "mallory"
ADMIN_USER_EMAIL = "mallory@example.com"
ADMIN_USER_PASSWORD = "Mallory!pass"
ADMIN_USER_ROLE = "ADMIN"


@pytest.fixture(scope="session")
def server_url() -> str:
    """Return the base URL of a running server, once it is healthy."""
    base_url = os.environ.get(_BASE_URL_ENV, _BASE_URL)

    deadline = time.monotonic() + _HEALTH_TIMEOUT_S
    while time.monotonic() < deadline:
        try:
            response = requests.get(f"{base_url}{_HEALTH_PATH}", timeout=1)
            if response.ok:
                return base_url
        except requests.ConnectionError:
            pass
        time.sleep(_POLL_INTERVAL_S)
    raise RuntimeError(
        f"server not healthy within {_HEALTH_TIMEOUT_S}s at {base_url}; start it first, e.g. with `devenv server`"
    )


@pytest.fixture(scope="session")
def default_password() -> str:
    """Return the root admin default password printed to the container stdout."""
    result = subprocess.run(
        ["docker", "logs", _CONTAINER],
        capture_output=True,
        text=True,
        check=True,
    )
    match = _DEFAULT_PASSWORD_RE.search(result.stdout)
    if match is None:
        raise RuntimeError("root admin default password not found in container stdout")
    return match.group(1)


@pytest.fixture(scope="session")
def assert_error_body() -> Callable[[requests.Response, int], None]:
    """Return a callable that asserts the exact ErrorBody shape declared in the OpenAPI spec."""

    def _assert_error_body(response: requests.Response, status_code: int) -> None:
        assert response.status_code == status_code, (
            f"expected {status_code}, got {response.status_code}: {response.text}"
        )
        assert response.headers.get("Content-Type", "").startswith("application/json")
        body = response.json()
        assert set(body.keys()) == {"error"}
        assert isinstance(body["error"], str) and body["error"]

    return _assert_error_body


@pytest.fixture(scope="session")
def login(server_url: str) -> Callable[[str, str], str]:
    """Return a callable that authenticates and returns the access token."""

    def _login(username: str, password: str) -> str:
        response = requests.post(
            f"{server_url}{LOGIN_PATH}",
            json={"username": username, "password": password},
            timeout=10,
        )
        assert response.status_code == 200, f"login failed: {response.status_code} {response.text}"
        body = response.json()
        assert body["token_type"] == "bearer"
        assert isinstance(body["access_token"], str) and body["access_token"]
        assert isinstance(body["expires_in"], int) and body["expires_in"] >= 0
        return body["access_token"]

    return _login


@pytest.fixture(scope="session")
def register(
    server_url: str,
) -> Callable[[str, str, str | None, str, str], requests.Response]:
    """Return a callable that registers a user, without asserting the status."""

    def _register(token: str, username: str, email: str | None, password: str, role: str) -> requests.Response:
        return requests.post(
            f"{server_url}{REGISTER_PATH}",
            headers={"Authorization": f"Bearer {token}"},
            json={"username": username, "email": email, "password": password, "role": role},
            timeout=10,
        )

    return _register


def _provision_user(
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
    token: str,
    username: str,
    email: str | None,
    password: str,
    role: str,
) -> str:
    """Provision a user, tolerating 409 from an earlier run, and return its token."""
    response = register(token, username, email, password, role)
    if response.status_code == 201:
        assert response.json()["role"] == role
    elif response.status_code != 409:
        pytest.fail(f"provisioning user {username!r} failed: {response.status_code} {response.text}")
    return login(username, password)


@pytest.fixture
def root_admin_token(server_url: str, default_password: str, login: Callable[[str, str], str]) -> str:
    """Access token of the Root Admin (username "root", default password)."""
    return login(ROOT_ADMIN_USERNAME, default_password)


@pytest.fixture
def admin_token(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
) -> str:
    """Access token of the non-root Admin "mallory", provisioned on first use."""
    return _provision_user(
        register,
        login,
        root_admin_token,
        ADMIN_USER_USERNAME,
        ADMIN_USER_EMAIL,
        ADMIN_USER_PASSWORD,
        ADMIN_USER_ROLE,
    )


@pytest.fixture
def standard_user_token(
    root_admin_token: str,
    register: Callable[[str, str, str | None, str, str], requests.Response],
    login: Callable[[str, str], str],
) -> str:
    """Access token of the Standard User "alice", provisioned on first use."""
    return _provision_user(
        register,
        login,
        root_admin_token,
        STANDARD_USER_USERNAME,
        STANDARD_USER_EMAIL,
        STANDARD_USER_PASSWORD,
        STANDARD_USER_ROLE,
    )
