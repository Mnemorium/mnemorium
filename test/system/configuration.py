"""Black-box proof of the `configuration` capability's resolution mechanism."""

import base64
import json
import os
import time

import pytest
import yaml
from harness import (
    CONFIG_PATH,
    DATABASE_PATH,
    MSG_ENV_OVERRIDES,
    ROOT_ADMIN_USERNAME,
    WRONG_PASSWORD,
    StartupFailure,
    Sut,
)
from labels import TestType, alternate_flow, business_rule, error_handling, positive, recovery, scenario
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


def _refused(sut: Sut, *, config_yaml: str | None = None, env: dict[str, str] | None = None) -> StartupFailure:
    """Start an instance expected to die, asserting a supplied file is left byte-identical."""
    failure = sut.start_expecting_failure(config_yaml=config_yaml, env=env)
    assert failure.exit_code != 0, f"the server started instead of refusing:\n{failure.output}"
    if config_yaml is not None:
        assert sut.read(CONFIG_PATH) == config_yaml, "the refused startup rewrote the configuration file"
    return failure


@positive
@scenario("SCN-CONFIGURATION-001-A")
@scenario("SCN-CONFIGURATION-003-A")
def test_nothing_supplied_uses_the_defaults_and_writes_the_file(sut: Sut) -> None:
    """An untouched working directory gets a complete default file and every setting at its default."""
    server = sut.start()

    assert server.request("GET", server.href("health")).status_code == 200
    assert server.exists(CONFIG_PATH)
    generated = yaml.safe_load(server.read(CONFIG_PATH))
    assert isinstance(generated, dict), "the generated file must be a complete document"
    assert generated["security"]["jwt"]["ttl"] == _DEFAULT_TTL_S
    assert _login(server, server.root_password()) == _DEFAULT_TTL_S


@business_rule
@scenario("SCN-CONFIGURATION-001-E")
@scenario("SCN-AUTHENTICATION-007-A")
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


@business_rule
@scenario("SCN-CONFIGURATION-001-B")
@scenario("SCN-CONFIGURATION-002-B", TestType.BUSINESS_RULE)
def test_the_file_overrides_the_defaults(sut: Sut) -> None:
    """A `config.yaml` value replaces the built-in default, and a sibling of the section keeps its own."""
    server = sut.start(config={"security": {"jwt": {"ttl": 11}}})

    # Reading the logged password also proves `security.log_root_admin_password`,
    # the named section's sibling, kept its default of `true`.
    assert _login(server, server.root_password()) == 11


@business_rule
@scenario("SCN-CONFIGURATION-001-C")
@scenario("SCN-CONFIGURATION-005-A")
def test_the_environment_overrides_the_file(sut: Sut) -> None:
    """A `MNEMORIUM_*` variable wins over the value the file carries."""
    server = sut.start(
        config={"security": {"jwt": {"ttl": 11}}},
        env={"MNEMORIUM_SECURITY_JWT_TTL": "13"},
    )

    assert _login(server, server.root_password()) == 13


@alternate_flow
@scenario("SCN-CONFIGURATION-001-D")
@scenario("SCN-CONFIGURATION-002-A", TestType.BUSINESS_RULE)
def test_a_partial_file_keeps_the_other_defaults(sut: Sut) -> None:
    """A one-key file takes effect and leaves every other setting at its default."""
    server = sut.start(config={"logging": {"ansi": True}})

    assert b"\x1b[" in server.logs().encode()
    assert _login(server, server.root_password()) == _DEFAULT_TTL_S


@business_rule
@scenario("SCN-CONFIGURATION-003-B")
@scenario("SCN-CONFIGURATION-003-C")
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


@scenario("SCN-CONFIGURATION-007-A", TestType.POSITIVE)
@scenario("SCN-CONFIGURATION-007-B", TestType.SECURITY)
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


@positive
@scenario("SCN-CONFIGURATION-007-C")
def test_no_override_is_reported_when_the_environment_names_nothing(sut: Sut) -> None:
    """A start whose environment names no setting reports no applied override."""
    server = sut.start()
    server.stop()

    assert not [line for line in sut.log_text().splitlines() if MSG_ENV_OVERRIDES in line]


@recovery
@scenario("SCN-CONFIGURATION-008-A")
@scenario("SCN-CONFIGURATION-008-B")
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


@error_handling
@scenario("SCN-CONFIGURATION-004-A")
def test_an_unknown_key_refuses_startup(sut: Sut) -> None:
    """An unknown key stops startup, names the key, and leaves the file as the operator wrote it."""
    unknown = "security:\n  jwt:\n    lifetime: 60\n"

    failure = _refused(sut, config_yaml=unknown)

    assert "lifetime" in failure.output, f"the refusal does not name the key:\n{failure.output}"
    assert not sut.exists(DATABASE_PATH), "a refused startup created the datastore"


@error_handling
@scenario("SCN-CONFIGURATION-004-B")
def test_a_malformed_document_refuses_startup(sut: Sut) -> None:
    """A document that is not valid YAML stops startup instead of reverting to the built-in defaults."""
    malformed = "logging: [unclosed\n"

    _refused(sut, config_yaml=malformed)

    assert not sut.exists(DATABASE_PATH), "a refused startup created the datastore"


@error_handling
@scenario("SCN-CONFIGURATION-004-C")
def test_a_wrongly_typed_value_refuses_startup(sut: Sut) -> None:
    """A setting given a value of the wrong type stops startup instead of being ignored."""
    mistyped = "logging:\n  max_files: many\n"

    _refused(sut, config_yaml=mistyped)

    assert not sut.exists(DATABASE_PATH), "a refused startup created the datastore"


@error_handling
@scenario("SCN-CONFIGURATION-004-D")
def test_an_unreadable_configuration_file_refuses_startup(sut: Sut) -> None:
    """A file that exists but cannot be read stops startup, naming the path it could not read."""
    (sut.dir / CONFIG_PATH).mkdir()

    failure = sut.start_expecting_failure()

    assert failure.exit_code != 0, f"the server started instead of refusing:\n{failure.output}"
    assert CONFIG_PATH in failure.output, f"the refusal does not name the path:\n{failure.output}"
    assert not sut.exists(DATABASE_PATH), "a refused startup created the datastore"


@error_handling
@scenario("SCN-CONFIGURATION-003-D")
def test_an_unwritable_configuration_location_refuses_startup(sut: Sut) -> None:
    """An absent file that cannot be written stops startup, naming the path it could not write.

    The instance is started once first so `state.yaml` already exists: the working
    directory is made read-only afterwards, so the refusal under test is the
    configuration write rather than the identity write that would otherwise fail
    first on a fresh directory.
    """
    server = sut.start()
    server.stop()
    (sut.dir / CONFIG_PATH).unlink()
    os.chmod(sut.dir, 0o555)
    try:
        failure = server.start_expecting_failure(create_media=False)
    finally:
        os.chmod(sut.dir, 0o755)

    assert failure.exit_code != 0, f"the server started instead of refusing:\n{failure.output}"
    assert CONFIG_PATH in failure.output, f"the refusal does not name the path:\n{failure.output}"
    assert not sut.exists(CONFIG_PATH), "a refused startup wrote the configuration file"


@error_handling
@scenario("SCN-CONFIGURATION-006-A")
@scenario("SCN-CONFIGURATION-006-C", TestType.SECURITY)
def test_a_rejected_file_value_names_the_setting_not_the_value(sut: Sut) -> None:
    """A value the setting does not accept stops startup, naming the setting and never its value."""
    sentinel = "sentinel-level"
    rejected = f'logging:\n  level: "security={sentinel}"\n'

    failure = _refused(sut, config_yaml=rejected)

    assert "logging.level" in failure.output, f"the refusal does not name the setting:\n{failure.output}"
    assert sentinel not in failure.output, f"the refusal carries the value:\n{failure.output}"
    assert not sut.exists(DATABASE_PATH), "a refused startup created the datastore"


@error_handling
@scenario("SCN-CONFIGURATION-006-B")
@scenario("SCN-CONFIGURATION-006-C", TestType.SECURITY)
def test_a_rejected_environment_value_names_the_variable_not_the_value(sut: Sut) -> None:
    """A variable holding a value its setting does not accept stops startup, naming the variable only."""
    sentinel = "sentinel-many"

    failure = _refused(sut, env={"MNEMORIUM_LOGGING_MAX_FILES": sentinel})

    assert "MNEMORIUM_LOGGING_MAX_FILES" in failure.output, f"the refusal does not name the variable:\n{failure.output}"
    assert sentinel not in failure.output, f"the refusal carries the value:\n{failure.output}"
    assert not sut.exists(DATABASE_PATH), "a refused startup created the datastore"
