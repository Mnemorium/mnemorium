"""Behavioural cases for the configuration layers: defaults, `config.yaml`, and the environment.

The layer order is proved on cheap probes rather than on every setting: `security.jwt.ttl` is visible in the login
response, `logging.ansi` in one escape byte, and `security.rate_limit.burst_size` in two logins. No case waits for a
clock and no assertion reads a repository file (`TEST-058`).
"""

import os

from harness import CONFIG_PATH, LOGIN_PATH, MSG_ENV_OVERRIDES, ROOT_ADMIN_USERNAME, WRONG_PASSWORD

_DEFAULT_TTL_S = 3600


def _login(sut, password: str) -> int:
    """Log in as the Root Admin and return the advertised token lifetime."""
    response = sut.request(
        "POST",
        LOGIN_PATH,
        json={"username": ROOT_ADMIN_USERNAME, "password": password},
    )
    assert response.status_code == 200, f"login failed: {response.status_code} {response.text}"
    return response.json()["expires_in"]


def test_the_file_overrides_the_defaults(sut):
    """A `config.yaml` value replaces the built-in default."""
    sut.start(config={"security": {"jwt": {"ttl": 11}}})

    assert _login(sut, sut.root_password()) == 11


def test_the_environment_overrides_the_file(sut):
    """A `MNEMORIUM_*` variable wins over the value the file carries."""
    sut.start(
        config={"security": {"jwt": {"ttl": 11}}},
        env={"MNEMORIUM_SECURITY_JWT_TTL": "13"},
    )

    assert _login(sut, sut.root_password()) == 13


def test_an_existing_file_is_never_rewritten(sut):
    """A file that already exists is left byte-identical, and its content is what the next run applies."""
    sut.write(CONFIG_PATH, "")
    before = os.stat(sut.dir / CONFIG_PATH)

    sut.start()
    password = sut.root_password()
    sut.stop()

    after = os.stat(sut.dir / CONFIG_PATH)

    assert sut.read(CONFIG_PATH) == ""
    assert after.st_mtime_ns == before.st_mtime_ns

    sut.write(CONFIG_PATH, "security:\n  jwt:\n    ttl: 7\n")
    sut.start()

    assert _login(sut, password) == 7


def test_a_partial_file_keeps_the_other_defaults(sut):
    """A one-key file takes effect and leaves every other setting at its default."""
    sut.start(config={"logging": {"ansi": True}})

    assert b"\x1b[" in sut.logs().encode()
    assert _login(sut, sut.root_password()) == _DEFAULT_TTL_S


def test_applied_overrides_are_reported_without_values(sut):
    """The start-up report names the setting the environment applied and never its value."""
    sut.start(env={"MNEMORIUM_SECURITY_JWT_TTL": "13"})
    sut.stop()

    lines = [line for line in sut.log_text().splitlines() if MSG_ENV_OVERRIDES in line]

    assert lines
    for line in lines:
        report = line[line.index(MSG_ENV_OVERRIDES) :]
        assert "security.jwt.ttl" in report
        assert "13" not in report


def test_a_runtime_edit_has_no_effect_until_restart(sut):
    """The resolved configuration is immutable: an edit is invisible until the server restarts."""
    sut.start(config={"security": {"rate_limit": {"burst_size": 5}}})

    for _ in range(3):
        assert sut.attempt_login(ROOT_ADMIN_USERNAME, WRONG_PASSWORD).status_code == 401

    sut.write(CONFIG_PATH, "security:\n  rate_limit:\n    burst_size: 1\n")
    sut.restart()

    assert sut.attempt_login(ROOT_ADMIN_USERNAME, WRONG_PASSWORD).status_code == 401
    assert sut.attempt_login(ROOT_ADMIN_USERNAME, WRONG_PASSWORD).status_code == 429
