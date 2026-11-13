"""Collect the system suite's feature-area modules and share their fixtures and helpers.

The suite names one module per feature area — `authentication.py` and `upload.py` among them — but the three global
`python_files` patterns (`test_*.py`, `*_test.py`, `*_*.py`) match none of those single-word names. This hook collects
every module directly in this folder that those patterns leave out, so a feature area needs no prefix and no entry in
`pytest.ini`.

Every feature module drives one instance per case and authenticates as the Root Admin the instance bootstrapped, so both
fixtures live here rather than in each module. The seed-media directory and the request helpers that more than one
operation needs are shared here the same way; every route those helpers address comes from the API index or from a
representation's links, never from a path built in this suite.

`/api/v1` is the system suite's only base literal, and it lives in `harness.Sut.index`.
"""

import base64
import hashlib
import hmac
import json
import time
from fnmatch import fnmatch
from pathlib import Path

import pytest
from harness import (
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    WRONG_PASSWORD,
    Sut,
    begin_upload,
    complete_upload,
    hex64,
    link_of,
    put_chunk,
    register,
)

HERE = Path(__file__).parent
FIXTURE_DIRECTORY = HERE.parent / "seed" / "media"

# The instance secrets are fixed for every feature module that mints a token or proves redaction, so a case can restart
# the same directory deterministically and a leak assertion is a substring search.
JWT_SECRET = hex64("a1")
PEPPER = hex64("b2")
INSTANCE_ENV = {"MNEMORIUM_JWT_SECRET": JWT_SECRET, "MNEMORIUM_PEPPER": PEPPER}
PASSWORD_SYMBOLS = frozenset("!@#$%^&*()-_=+")


def pytest_collect_file(file_path: Path, parent: pytest.Collector) -> pytest.Collector | None:
    """Collect each unprefixed Python module of the system suite."""
    if file_path.suffix != ".py" or file_path.parent != HERE or file_path.name == "conftest.py":
        return None
    if any(fnmatch(file_path.name, pattern) for pattern in parent.config.getini("python_files")):
        return None
    return pytest.Module.from_parent(parent, path=file_path)


@pytest.fixture
def server(sut: Sut) -> Sut:
    """A fresh instance serving the feature under test."""
    return sut.start()


@pytest.fixture
def root_token(server: Sut) -> str:
    """A bearer token for the Root Admin the instance bootstrapped."""
    return server.login(ROOT_ADMIN_USERNAME, server.root_password())


def provision(server: Sut, root_token: str, username: str, role: str = "STANDARD") -> tuple[int, str]:
    """Create an account as the Root Admin and return its identifier and access token."""
    response = register(server, root_token, username, None, SECRET_PASSWORD, role=role)
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    return response.json()["id"], server.login(username, SECRET_PASSWORD)


def start_instance(sut: Sut, *, config: dict | None = None, env: dict[str, str] | None = None) -> Sut:
    """Start a container whose signing secret and site pepper are fixed for every case."""
    return sut.start(env={**INSTANCE_ENV, **(env or {})}, config=config)


def limiter_config(*, trusted: str | None = None) -> dict:
    """A login limiter with one attempt per address and a long refill period."""
    rate_limit = {
        "burst_size": 1,
        "client_ip_header": "x-real-ip",
        "period_seconds": 60,
    }
    if trusted is not None:
        rate_limit["trusted_proxies"] = trusted
    return {"security": {"rate_limit": rate_limit}}


def _base64url(value: bytes) -> str:
    """Base64url-encode `value` without padding, as a JWT segment."""
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


def mint_token(secret: str, subject: str, *, expires_in: int = 3600, tampered: bool = False) -> str:
    """Mint one HS256 token, optionally with a corrupted signature or a past expiry."""
    header = _base64url(json.dumps({"alg": "HS256", "typ": "JWT"}, separators=(",", ":")).encode())
    payload = _base64url(json.dumps({"exp": int(time.time()) + expires_in, "sub": subject}).encode())
    signature = hmac.new(secret.encode(), f"{header}.{payload}".encode(), hashlib.sha256).digest()
    if tampered:
        signature = bytes([signature[0] ^ 0xFF, *signature[1:]])
    return f"{header}.{payload}.{_base64url(signature)}"


def security_events(server: Sut, name: str) -> list[str]:
    """Return the log lines carrying the security event `name`."""
    marker = f'event="{name}"'
    return [line for line in server.logs().splitlines() if marker in line]


def attempt(server: Sut, username: str, address: str | None = None):
    """A login attempt the limiter counts; the credentials cannot succeed."""
    headers = {"x-real-ip": address} if address is not None else None
    return server.attempt_login(username, WRONG_PASSWORD, headers=headers)


def upload(
    server: Sut,
    token: str,
    file_name: str,
    data: bytes,
    content_type: str = "image/png",
) -> dict:
    """Upload one file to completion and return the file representation."""
    session = begin_upload(server, token, file_name, data, content_type)
    chunk = put_chunk(server, token, session, 0, data, len(data))
    assert chunk.status_code == 200, f"chunk failed: {chunk.status_code} {chunk.text}"
    completed = complete_upload(server, token, session)
    assert completed.status_code == 200, f"completion failed: {completed.status_code} {completed.text}"
    response = server.request("GET", link_of(completed.json(), "file"), token=token)
    assert response.status_code == 200, f"file read failed: {response.status_code} {response.text}"
    return response.json()


def file_profile(server: Sut, token: str, file: dict) -> dict:
    """Read a stored file's representation again; fails unless it answers 200."""
    response = server.request("GET", link_of(file, "self"), token=token)
    assert response.status_code == 200, f"file read failed: {response.status_code} {response.text}"
    return response.json()


def linked_id(href: str) -> int:
    """The numeric identifier at the end of a HAL link."""
    return int(href.rsplit("/", 1)[1])


def create_gallery(server: Sut, token: str, name: str, is_public: bool = False) -> dict:
    """Create a gallery and return its representation."""
    response = server.request(
        "POST",
        server.href("gallery"),
        token=token,
        json={"name": name, "is_public": is_public},
    )
    assert response.status_code == 201, f"gallery failed: {response.status_code} {response.text}"
    return response.json()


def add_to_gallery(server: Sut, token: str, gallery: dict, media_id: int) -> dict:
    """Add an image to a gallery and return the created item's representation."""
    response = server.request(
        "POST",
        link_of(gallery, "items"),
        token=token,
        json={"type": "image", "media_id": media_id},
    )
    assert response.status_code == 201, f"item failed: {response.status_code} {response.text}"
    return response.json()
