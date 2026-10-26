"""Behavioural cases for the `security.*` settings.

`jwt.ttl` sets the advertised and actual token lifetime, `log_root_admin_password` decides whether the one-time password
is printed, and the four `rate_limit.*` settings build the login limiter's topology: `burst_size` is the quota,
`period_seconds` its refill, `client_ip_header` selects the header a trusted proxy overwrites, and `trusted_proxies`
decides whether that header is read at all.

A failed login is the counted request: it needs no provisioned user and cannot succeed, so each case stays small. `429`
is asserted by status and the `Retry-After` header only; the prose belongs to the endpoint (`TEST-059`).
"""

import base64
import json
import time

import pytest
import requests
from harness import (
    HEALTH_PATH,
    LOGIN_PATH,
    MSG_PASSWORD_HIDDEN,
    MSG_PASSWORD_LOGGED,
    ROOT_ADMIN_USERNAME,
    WRONG_PASSWORD,
)

_HEADER = "x-real-ip"
_LONG_PERIOD_S = 60


def _claims(token: str) -> dict:
    """The token's claims, decoded from its base64url payload without verifying the signature."""
    payload = token.split(".")[1]
    return json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))


def _bad_login(sut, headers: dict[str, str] | None = None) -> requests.Response:
    """A login attempt the limiter counts, with credentials that cannot succeed."""
    return sut.attempt_login(ROOT_ADMIN_USERNAME, WRONG_PASSWORD, headers=headers)


def test_ttl_sets_the_advertised_lifetime(sut):
    """The login response mirrors `security.jwt.ttl`."""
    sut.start(config={"security": {"jwt": {"ttl": 1}}})

    response = sut.request(
        "POST",
        LOGIN_PATH,
        json={"username": ROOT_ADMIN_USERNAME, "password": sut.root_password()},
    )

    assert response.status_code == 200
    assert response.json()["expires_in"] == 1


def test_a_token_expires_after_the_configured_ttl(sut):
    """The token's own expiry is one second after issue.

    The assertion reads the `exp` claim rather than waiting for the `401`: the token provider validates with
    jsonwebtoken's default 60-second clock-skew leeway, so the rejection is not observable inside the sleep budget
    (`TEST-058`).
    """
    sut.start(config={"security": {"jwt": {"ttl": 1}}})

    before = int(time.time())
    token = sut.login(ROOT_ADMIN_USERNAME, sut.root_password())
    after = int(time.time())

    assert before + 1 <= _claims(token)["exp"] <= after + 1


def test_the_default_password_is_printed_when_enabled(sut):
    """The default keeps the one-time Root Admin password on stdout, where it can be used."""
    sut.start()

    password = sut.root_password()

    assert sut.login(ROOT_ADMIN_USERNAME, password)


def test_the_default_password_is_not_printed_when_disabled(sut):
    """`log_root_admin_password: false` replaces the password with a notice, in no sink."""
    sut.start(config={"security": {"log_root_admin_password": False}})

    assert sut.request("GET", HEALTH_PATH).status_code == 200

    stdout = sut.logs()
    sut.stop()
    written = sut.log_text()

    assert MSG_PASSWORD_HIDDEN in stdout
    assert MSG_PASSWORD_HIDDEN in written
    assert MSG_PASSWORD_LOGGED.search(stdout) is None
    assert MSG_PASSWORD_LOGGED.search(written) is None


def test_burst_size_throttles_a_login_burst(sut):
    """`burst_size: 1` admits one attempt, refuses the next with `Retry-After`, then refills."""
    sut.start(config={"security": {"rate_limit": {"burst_size": 1, "period_seconds": 1}}})

    assert _bad_login(sut).status_code == 401

    throttled = _bad_login(sut)

    assert throttled.status_code == 429
    assert "Retry-After" in throttled.headers

    time.sleep(2)

    assert _bad_login(sut).status_code == 401


def test_period_controls_the_refill(sut):
    """`period_seconds: 2` refills nothing after one second and one element after two."""
    sut.start(config={"security": {"rate_limit": {"burst_size": 1, "period_seconds": 2}}})

    assert _bad_login(sut).status_code == 401

    time.sleep(1)

    assert _bad_login(sut).status_code == 429

    time.sleep(2)

    assert _bad_login(sut).status_code == 401


@pytest.mark.slow
def test_a_trusted_proxy_keys_on_the_header(sut):
    """A trusted peer's declared header names the bucket, so two clients get two quotas."""
    sut.start()
    peer = sut.peer_address()
    sut.restart(
        config={
            "security": {
                "rate_limit": {
                    "burst_size": 1,
                    "client_ip_header": _HEADER,
                    "period_seconds": _LONG_PERIOD_S,
                    "trusted_proxies": peer,
                }
            }
        }
    )

    assert _bad_login(sut, {_HEADER: "203.0.113.1"}).status_code == 401
    assert _bad_login(sut, {_HEADER: "203.0.113.1"}).status_code == 429
    assert _bad_login(sut, {_HEADER: "203.0.113.2"}).status_code == 401
    assert _bad_login(sut, {_HEADER: "203.0.113.3"}).status_code == 401


@pytest.mark.slow
def test_an_untrusted_peer_ignores_the_header(sut):
    """The same header cannot mint a bucket for a peer `trusted_proxies` does not list."""
    sut.start(
        config={
            "security": {
                "rate_limit": {
                    "burst_size": 1,
                    "client_ip_header": _HEADER,
                    "period_seconds": _LONG_PERIOD_S,
                }
            }
        }
    )

    assert _bad_login(sut, {_HEADER: "203.0.113.1"}).status_code == 401
    assert _bad_login(sut, {_HEADER: "203.0.113.2"}).status_code == 429
