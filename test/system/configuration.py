"""Black-box proof of the `configuration` capability's resolution mechanism."""

import base64
import json
import os
import time

import allure
import pytest
import yaml
from harness import (
    CONFIG_PATH,
    MSG_ENV_OVERRIDES,
    ROOT_ADMIN_USERNAME,
    WRONG_PASSWORD,
    Sut,
)
from mnemorium_client.api.identity import post_login
from mnemorium_client.models import LoginRequest

# Configuration owns its instances: every case writes or observes `config.yaml`, the environment and a restart.
pytestmark = pytest.mark.own_instance(reason="global_filesystem")

_DEFAULT_TTL_S = 3600


def _login(sut: Sut, password: str) -> int:
    """Log in as the Root Admin and return the advertised token lifetime."""
    response = post_login.sync_detailed(
        client=sut.client(), body=LoginRequest(password=password, identifier=ROOT_ADMIN_USERNAME)
    )
    assert response.status_code == 200, f"login failed: {response.status_code} {response.content!r}"
    return json.loads(response.content)["expires_in"]


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-001")
@allure.feature("REQ-CONFIGURATION-003")
@allure.story("SCN-CONFIGURATION-001-A")
@allure.story("SCN-CONFIGURATION-003-A")
def test_nothing_supplied_uses_the_defaults_and_writes_the_file(sut: Sut) -> None:
    """An untouched working directory gets a complete default file and every setting at its default."""
    server = sut.start()

    assert server.request("GET", server.href("health")).status_code == 200
    assert server.exists(CONFIG_PATH)
    generated = yaml.safe_load(server.read(CONFIG_PATH))
    assert isinstance(generated, dict), "the generated file must be a complete document"
    assert generated["security"]["jwt"]["ttl"] == _DEFAULT_TTL_S
    assert _login(server, server.root_password()) == _DEFAULT_TTL_S


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-001")
@allure.story("SCN-CONFIGURATION-001-E")
@allure.feature("REQ-AUTHENTICATION-007")
@allure.story("SCN-AUTHENTICATION-007-A")
def test_the_resolved_lifetime_reaches_the_issued_token(sut: Sut) -> None:
    """A configured `security.jwt.ttl` is advertised and carried by the token's expiry."""
    ttl = 120
    server = sut.start(config={"security": {"jwt": {"ttl": ttl}}})

    before = int(time.time())
    response = post_login.sync_detailed(
        client=server.client(), body=LoginRequest(password=server.root_password(), identifier=ROOT_ADMIN_USERNAME)
    )
    after = int(time.time())

    assert response.status_code == 200
    body = json.loads(response.content)
    payload = body["access_token"].split(".")[1]
    issued = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
    assert body["expires_in"] == ttl
    assert before + ttl <= issued["exp"] <= after + ttl


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-001")
@allure.story("SCN-CONFIGURATION-001-B")
def test_the_file_overrides_the_defaults(sut: Sut) -> None:
    """A `config.yaml` value replaces the built-in default."""
    server = sut.start(config={"security": {"jwt": {"ttl": 11}}})

    assert _login(server, server.root_password()) == 11


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-001")
@allure.story("SCN-CONFIGURATION-001-C")
def test_the_environment_overrides_the_file(sut: Sut) -> None:
    """A `MNEMORIUM_*` variable wins over the value the file carries."""
    server = sut.start(
        config={"security": {"jwt": {"ttl": 11}}},
        env={"MNEMORIUM_SECURITY_JWT_TTL": "13"},
    )

    assert _login(server, server.root_password()) == 13


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-001")
@allure.story("SCN-CONFIGURATION-001-D")
def test_a_partial_file_keeps_the_other_defaults(sut: Sut) -> None:
    """A one-key file takes effect and leaves every other setting at its default."""
    server = sut.start(config={"logging": {"ansi": True}})

    assert b"\x1b[" in server.logs().encode()
    assert _login(server, server.root_password()) == _DEFAULT_TTL_S


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-003")
@allure.story("SCN-CONFIGURATION-003-B")
@allure.story("SCN-CONFIGURATION-003-C")
def test_an_existing_file_is_never_rewritten(sut: Sut) -> None:
    """An existing file, empty or partial, is left byte-identical, and its content is what the next run applies."""
    sut.write(CONFIG_PATH, "")
    before = os.stat(sut.dir / CONFIG_PATH)

    server = sut.start()
    password = server.root_password()
    server.stop()

    after = os.stat(sut.dir / CONFIG_PATH)

    assert sut.read(CONFIG_PATH) == ""
    assert after.st_mtime_ns == before.st_mtime_ns

    partial = "security:\n  jwt:\n    ttl: 7\n"
    sut.write(CONFIG_PATH, partial)
    server = sut.start()

    assert sut.read(CONFIG_PATH) == partial
    assert _login(server, password) == 7


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-007")
@allure.story("SCN-CONFIGURATION-007-A")
@allure.story("SCN-CONFIGURATION-007-B")
def test_applied_overrides_are_reported_without_values(sut: Sut) -> None:
    """The start-up report names the setting the environment applied and never its value."""
    server = sut.start(env={"MNEMORIUM_SECURITY_JWT_TTL": "13"})
    server.stop()

    lines = [line for line in sut.log_text().splitlines() if MSG_ENV_OVERRIDES in line]

    assert lines
    for line in lines:
        report = line[line.index(MSG_ENV_OVERRIDES) :]
        assert "security.jwt.ttl" in report
        assert "13" not in report


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-007")
@allure.story("SCN-CONFIGURATION-007-C")
def test_no_override_is_reported_when_the_environment_names_nothing(sut: Sut) -> None:
    """A start whose environment names no setting reports no applied override."""
    server = sut.start()
    server.stop()

    assert not [line for line in sut.log_text().splitlines() if MSG_ENV_OVERRIDES in line]


@allure.epic("Configuration")
@allure.feature("REQ-CONFIGURATION-008")
@allure.story("SCN-CONFIGURATION-008-A")
@allure.story("SCN-CONFIGURATION-008-B")
def test_a_runtime_edit_has_no_effect_until_restart(sut: Sut) -> None:
    """The resolved configuration is immutable: an edit is invisible until the server restarts."""
    server = sut.start(config={"security": {"rate_limit": {"burst_size": 5}}})

    for _ in range(3):
        assert (
            post_login.sync_detailed(
                client=server.client(), body=LoginRequest(password=WRONG_PASSWORD, identifier=ROOT_ADMIN_USERNAME)
            ).status_code
            == 401
        )

    sut.write(CONFIG_PATH, "security:\n  rate_limit:\n    burst_size: 1\n")

    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=WRONG_PASSWORD, identifier=ROOT_ADMIN_USERNAME)
        ).status_code
        == 401
    )

    server.restart()

    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=WRONG_PASSWORD, identifier=ROOT_ADMIN_USERNAME)
        ).status_code
        == 401
    )
    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=WRONG_PASSWORD, identifier=ROOT_ADMIN_USERNAME)
        ).status_code
        == 429
    )
