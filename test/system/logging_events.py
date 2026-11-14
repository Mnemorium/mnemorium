"""Black-box proof of the `logging` capability.

Every case starts its own container from the image under test through the shared `harness` plugin and reads only what an
operator can: standard output and the log directory. `Sut.logs()` is safe while the server runs; `Sut.log_text()` is
complete only after `stop()` drops the non-blocking writer's guard, so an assertion on file content always stops first.

No case waits for a rotation: the period is observed through the active file's name, and pruning happens when the
appender is built, so a pre-seeded `logs/` needs no clock.
"""

import re
import shutil
import time
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

import allure
import pytest
import yaml
from conftest import (
    FIXTURE_DIRECTORY,
    INSTANCE_ENV,
    JWT_SECRET,
    PEPPER,
    attempt,
    create_gallery,
    limiter_config,
    mint_token,
    provision,
    security_events,
    start_instance,
)
from harness import (
    LOG_DIRECTORY,
    MEDIA_DIRECTORY,
    MSG_CONFIG_GENERATED,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    STATE_PATH,
    WRONG_PASSWORD,
    Sut,
    begin_upload,
    complete_upload,
    link_of,
    put_chunk,
    register,
)

_LEVEL_ENV = "MNEMORIUM_LOGGING_LEVEL"
_ROTATION_NEVER_FILE = "mnemorium.log"

# Five sessions old enough that any pruning order keeps the count predictable: the appender keeps `max_files - 1` of
# them and then creates today's file.
_SEEDED_LOGS = {f"logs/mnemorium.log.2020-01-0{day}": "old\n" for day in range(1, 6)}


@contextmanager
def _control_server(image: str, tmp_root: Path, **start) -> Iterator[Sut]:
    """A second server for a paired control run, removed with its working directory."""
    server = Sut(image=image, tmp_root=tmp_root)
    tmp_root.mkdir(parents=True, exist_ok=True)
    try:
        yield server.start(**start)
    finally:
        server.stop()
        shutil.rmtree(server.dir, ignore_errors=True)


def _wait_for(server: Sut, name: str, *, timeout: float = 20.0) -> list[str]:
    """Return the lines carrying event `name` once one reaches standard output, polling up to `timeout`."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        lines = security_events(server, name)
        if lines:
            return lines
        time.sleep(0.1)
    raise AssertionError(f'no event="{name}" reached standard output within {timeout:g}s')


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-001")
@allure.story("SCN-LOGGING-001-A")
def test_the_file_sink_holds_the_event(sut: Sut) -> None:
    """An accepted credential appears in `logs/mnemorium.log`."""
    server = start_instance(sut)

    server.login(ROOT_ADMIN_USERNAME, server.root_password())
    server.stop()

    written = [line for line in server.log_text().splitlines() if 'event="authn_succeeded"' in line]
    assert len(written) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-001")
@allure.story("SCN-LOGGING-001-B")
def test_the_standard_output_sink_holds_the_event(sut: Sut) -> None:
    """An accepted credential appears on standard output."""
    server = start_instance(sut)

    server.login(ROOT_ADMIN_USERNAME, server.root_password())

    assert len(security_events(server, "authn_succeeded")) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-001")
@allure.story("SCN-LOGGING-001-C")
@pytest.mark.slow
def test_only_standard_output_is_coloured(image: str, sut: Sut, tmp_path: Path) -> None:
    """Enabling colour colours standard output only; a disabled configuration colours neither sink."""
    server = start_instance(sut, config={"logging": {"ansi": True}})
    server.login(ROOT_ADMIN_USERNAME, server.root_password())
    stdout = server.logs()
    server.stop()
    written = server.log_text()

    assert "\x1b[" in stdout
    assert "\x1b[" not in written

    with _control_server(
        image,
        tmp_path / "control",
        env=INSTANCE_ENV,
        config={"logging": {"ansi": False}},
    ) as control:
        assert "\x1b[" not in control.logs()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-002")
@allure.story("SCN-LOGGING-002-A")
@pytest.mark.slow
def test_rotation_replaces_the_active_file(sut: Sut) -> None:
    """A period change starts a dated active file holding the further event and keeps the previous file."""
    server = start_instance(sut, config={"logging": {"rotation": "NEVER"}})
    server.stop()

    assert server.listing(LOG_DIRECTORY) == [_ROTATION_NEVER_FILE]

    server.start(env=INSTANCE_ENV, config={"logging": {"rotation": "DAILY"}})
    names = server.listing(LOG_DIRECTORY)
    dated = [name for name in names if name.startswith(f"{_ROTATION_NEVER_FILE}.")]
    assert _ROTATION_NEVER_FILE in names, "the previous active file must be retained"
    assert len(dated) == 1, f"expected one dated active file, found {names}"

    server.stop()
    assert 'event="lifecycle"' in server.read(f"{LOG_DIRECTORY}/{dated[0]}")


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-002")
@allure.story("SCN-LOGGING-002-B")
@pytest.mark.slow
def test_pruning_bounds_the_retained_files(sut: Sut) -> None:
    """A maximum of two keeps one seeded file plus today's."""
    sut.start(config={"logging": {"max_files": 2}}, files=_SEEDED_LOGS)
    sut.stop()

    assert len(sut.listing(LOG_DIRECTORY)) == 2


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-002")
@allure.story("SCN-LOGGING-002-C")
@pytest.mark.slow
def test_a_zero_maximum_keeps_every_file(sut: Sut) -> None:
    """A maximum of zero retains the five seeded files and today's."""
    sut.start(config={"logging": {"max_files": 0}}, files=_SEEDED_LOGS)
    sut.stop()

    assert len(sut.listing(LOG_DIRECTORY)) == 6


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-003")
@allure.story("SCN-LOGGING-003-A")
def test_an_empty_verbosity_is_refused(sut: Sut) -> None:
    """An empty verbosity fails startup naming `logging.level`."""
    failure = sut.start_expecting_failure(config={"logging": {"level": ""}})

    assert failure.exit_code != 0
    assert "logging.level" in failure.output


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-003")
@allure.story("SCN-LOGGING-003-B")
@pytest.mark.parametrize("value", ["off", "none"])
def test_disabling_logging_is_refused(sut: Sut, value: str) -> None:
    """A whole-level `off` or `none` fails startup naming the setting and never echoing the value."""
    failure = sut.start_expecting_failure(env={_LEVEL_ENV: value})

    assert failure.exit_code != 0
    assert "logging.level" in failure.output
    assert f"={value}" not in failure.output
    assert f"'{value}'" not in failure.output


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-003")
@allure.story("SCN-LOGGING-003-C")
def test_targeting_the_reserved_target_is_refused(sut: Sut) -> None:
    """A directive naming the reserved `security` target fails startup naming the setting, never the value."""
    value = "debug,security=debug"
    failure = sut.start_expecting_failure(env={_LEVEL_ENV: value})

    assert failure.exit_code != 0
    assert "logging.level" in failure.output
    assert value not in failure.output


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-003")
@allure.story("SCN-LOGGING-003-D")
@pytest.mark.slow
def test_the_value_tunes_operational_verbosity(image: str, sut: Sut, tmp_path: Path) -> None:
    """A quiet verbosity drops an operator message that a louder one keeps."""
    server = start_instance(sut, env={_LEVEL_ENV: "error"})
    server.stop()

    assert MSG_CONFIG_GENERATED not in server.log_text()

    with _control_server(image, tmp_path / "control", env={_LEVEL_ENV: "warn"}) as control:
        control.stop()

        assert MSG_CONFIG_GENERATED in control.log_text()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-004")
@allure.story("SCN-LOGGING-004-A")
def test_a_success_survives_a_fault_only_verbosity(sut: Sut) -> None:
    """With a verbosity admitting only `error`, a login succeeds and its `info` event reaches the sinks."""
    server = start_instance(sut, env={_LEVEL_ENV: "error"})

    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    assert token
    assert len(security_events(server, "authn_succeeded")) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-004")
@allure.story("SCN-LOGGING-004-B")
def test_the_lowest_declared_severity_survives(sut: Sut) -> None:
    """With a verbosity admitting only `error`, a catalogued `debug` event still reaches the sinks."""
    server = start_instance(sut, env={_LEVEL_ENV: "error"})
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    response = server.request("GET", server.href("current_user"), token=token)

    assert response.status_code == 200
    assert len(security_events(server, "sensitive_data_access")) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-015")
@allure.story("SCN-LOGGING-015-A")
def test_startup_records_a_lifecycle_event(sut: Sut) -> None:
    """Installing logging records the component and the outcome, even at a fault-only verbosity."""
    server = start_instance(sut, env={_LEVEL_ENV: "error"})
    server.stop()

    installed = [
        line
        for line in server.log_text().splitlines()
        if 'event="lifecycle"' in line and 'component="logging"' in line and 'outcome="initialized"' in line
    ]
    assert len(installed) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-006")
@allure.story("SCN-LOGGING-006-C")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-B")
def test_a_rejected_login_is_recorded_once(sut: Sut) -> None:
    """A wrong password and an unknown username each emit one `authn_failed` with the same reason."""
    server = start_instance(sut)
    server.root_password()

    assert server.attempt_login(ROOT_ADMIN_USERNAME, WRONG_PASSWORD).status_code == 401
    assert server.attempt_login("no-such-account", WRONG_PASSWORD).status_code == 401

    emitted = security_events(server, "authn_failed")
    assert len(emitted) == 2
    assert all('reason="invalid_credentials"' in line for line in emitted)


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-006")
@allure.story("SCN-LOGGING-006-A")
@allure.feature("REQ-LOGGING-013")
@allure.story("SCN-LOGGING-013-A")
def test_a_denial_is_recorded_once(sut: Sut) -> None:
    """A denied view records exactly one `authz_failed` naming the actor, the action and the target."""
    server = start_instance(sut)
    root_token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    gallery = create_gallery(server, root_token, "private-gallery", is_public=False)
    alice_id, alice = provision(server, root_token, "alice")

    response = server.request("GET", link_of(gallery, "self"), token=alice)

    assert response.status_code == 404
    emitted = security_events(server, "authz_failed")
    assert len(emitted) == 1
    assert f"actor={alice_id}" in emitted[0]
    assert 'action="view"' in emitted[0]
    assert re.search(r"target=GALLERY:\d+", emitted[0]), emitted[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-013")
@allure.story("SCN-LOGGING-013-B")
def test_filtering_a_listing_records_no_denial(sut: Sut) -> None:
    """A listing that drops an undiscoverable item is not a denial."""
    server = start_instance(sut)
    root_token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    create_gallery(server, root_token, "private-gallery", is_public=False)
    _, alice = provision(server, root_token, "alice")

    response = server.request("GET", server.href("gallery"), token=alice)

    assert response.status_code == 200
    assert security_events(server, "authz_failed") == []


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-013")
@allure.story("SCN-LOGGING-013-C")
def test_an_out_of_order_action_is_suspicious(sut: Sut) -> None:
    """Completing an upload before its chunks arrive records one `suspicious_business_logic`."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    data = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    session = begin_upload(server, token, "out-of-order.png", data)

    response = complete_upload(server, token, session)

    assert response.status_code == 422
    emitted = security_events(server, "suspicious_business_logic")
    assert len(emitted) == 1
    assert "actor=0" in emitted[0]
    assert "action=complete" in emitted[0]
    assert "reason=not_complete" in emitted[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-007")
@allure.story("SCN-LOGGING-007-A")
def test_a_query_string_is_withheld(sut: Sut) -> None:
    """A traced request carries the path and not the query string."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    sentinel = "LEAK_SENTINEL"

    response = server.request("GET", f"{server.href('gallery')}?name={sentinel}", token=token)

    assert response.status_code == 200
    logs = server.logs()
    assert "path=/api/v1/library/gallery" in logs
    assert sentinel not in logs


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-007")
@allure.story("SCN-LOGGING-007-B")
@allure.feature("REQ-LOGGING-009")
@allure.story("SCN-LOGGING-009-B")
def test_a_file_name_is_withheld_from_a_storage_fault(sut: Sut) -> None:
    """A failed promotion records its classification at `error` and never the user-supplied name."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    data = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    sentinel = "leak-sentinel.png"
    session = begin_upload(server, token, sentinel, data)
    assert put_chunk(server, token, session, 0, data, len(data)).status_code == 200
    shutil.rmtree(server.dir / MEDIA_DIRECTORY)

    response = complete_upload(server, token, session)

    assert response.status_code == 503
    emitted = security_events(server, "port_fault")
    assert len(emitted) == 1
    assert "ERROR" in emitted[0]
    assert 'kind="destination_missing"' in emitted[0]
    assert 'operation="promote"' in emitted[0]
    assert sentinel not in emitted[0]
    assert sentinel not in server.logs()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-008")
@allure.story("SCN-LOGGING-008-A")
def test_login_secrets_stay_out_of_every_sink(sut: Sut) -> None:
    """Neither the submitted password nor the issued token reaches standard output or a log file."""
    server = start_instance(sut)
    password = server.root_password()
    token = server.login(ROOT_ADMIN_USERNAME, password)
    new_password = "N3w!Secret"

    response = server.request(
        "PATCH",
        server.href("credential", id=1),
        token=token,
        json={"password": new_password},
    )

    assert response.status_code == 204
    stdout = server.logs()
    server.stop()
    written = server.log_text()
    assert token not in stdout
    assert token not in written
    assert new_password not in stdout
    assert new_password not in written
    assert password not in written


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-008")
@allure.story("SCN-LOGGING-008-B")
@pytest.mark.parametrize("seeded", [True, False])
def test_instance_secrets_stay_out_of_every_sink(sut: Sut, seeded: bool) -> None:
    """Neither the seeded nor the generated pepper and signing secret reaches a sink."""
    if seeded:
        server = start_instance(sut)
        secrets = [PEPPER, JWT_SECRET]
    else:
        server = sut.start()
        state = yaml.safe_load(server.read(STATE_PATH))
        secrets = [state["pepper"], state["jwt_secret"]]
    server.root_password()
    stdout = server.logs()
    server.stop()
    written = server.log_text()

    for secret in secrets:
        assert secret not in stdout
        assert secret not in written


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-009")
@allure.story("SCN-LOGGING-009-A")
def test_an_expected_rejection_is_not_an_error(sut: Sut) -> None:
    """A rejected credential is recorded at `warn`, never at `error`."""
    server = start_instance(sut)
    server.root_password()

    assert attempt(server, ROOT_ADMIN_USERNAME).status_code == 401

    emitted = security_events(server, "authn_failed")
    assert len(emitted) == 1
    assert "WARN" in emitted[0]
    assert "ERROR" not in emitted[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-011")
@allure.story("SCN-LOGGING-011-A")
def test_a_control_character_is_escaped(sut: Sut) -> None:
    """A newline in a claimed identity is escaped so the record stays on one line."""
    server = start_instance(sut)
    server.root_password()
    claimed = "evil\nforged"

    assert server.attempt_login(claimed, WRONG_PASSWORD).status_code == 401

    emitted = security_events(server, "authn_failed")
    assert len(emitted) == 1
    assert "claimed_identity=evil\\nforged" in emitted[0]
    assert not any(line.strip() == "forged" for line in server.logs().splitlines())


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-011")
@allure.story("SCN-LOGGING-011-B")
def test_the_identity_is_bounded(sut: Sut) -> None:
    """An over-long claimed identity is truncated to the declared bound."""
    server = start_instance(sut)
    server.root_password()

    assert server.attempt_login("a" * 300, WRONG_PASSWORD).status_code == 401

    emitted = security_events(server, "authn_failed")
    assert len(emitted) == 1
    match = re.search(r"claimed_identity=(\S+)", emitted[0])
    assert match is not None
    assert len(match.group(1)) == 256
    assert "a" * 257 not in emitted[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-A")
def test_a_success_names_the_actor(sut: Sut) -> None:
    """One accepted credential emits exactly one `authn_succeeded` naming the actor."""
    server = start_instance(sut)

    server.login(ROOT_ADMIN_USERNAME, server.root_password())

    emitted = security_events(server, "authn_succeeded")
    assert len(emitted) == 1
    assert "actor=0" in emitted[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-C")
def test_token_rejections_name_their_reason(sut: Sut) -> None:
    """A missing, invalid and expired token each emit one `session_validation_failed`."""
    server = start_instance(sut)
    server.root_password()

    rejected = (
        server.request("GET", server.href("current_user")),
        server.request("GET", server.href("current_user"), token=mint_token(JWT_SECRET, "0", tampered=True)),
        server.request("GET", server.href("current_user"), token=mint_token(JWT_SECRET, "0", expires_in=-1)),
    )
    assert [response.status_code for response in rejected] == [401, 401, 401]

    emitted = security_events(server, "session_validation_failed")
    assert len(emitted) == 3
    assert sum('reason="missing_authorization_header"' in line for line in emitted) == 1
    assert sum('reason="invalid_or_expired_token"' in line for line in emitted) == 2


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-D")
def test_a_throttled_request_names_its_route(sut: Sut) -> None:
    """A rate-limited login records one `rate_limit_exceeded` with its route and reason."""
    server = start_instance(sut, config=limiter_config())
    server.root_password()

    assert attempt(server, "ghost-a").status_code == 401
    assert attempt(server, "ghost-b").status_code == 429

    emitted = security_events(server, "rate_limit_exceeded")
    assert len(emitted) == 1
    assert 'route="identity.login"' in emitted[0]
    assert 'reason="quota_exceeded"' in emitted[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-E")
def test_regeneration_and_rotation_name_the_actor(sut: Sut) -> None:
    """A token regeneration and a credential rotation each record one event naming the actor."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    rotated = server.request(
        "PATCH",
        server.href("credential", id=1),
        token=token,
        json={"password": "R0tated!Pass"},
    )
    regenerated = server.request("POST", server.href("token"), token=token)

    assert rotated.status_code == 204
    assert regenerated.status_code == 200
    credentials = security_events(server, "credential_rotated")
    tokens = security_events(server, "token_regenerated")
    assert len(credentials) == 1
    assert len(tokens) == 1
    assert "actor=0" in credentials[0]
    assert "actor=0" in tokens[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-015")
@allure.story("SCN-LOGGING-015-B")
def test_identity_creation_is_reported(sut: Sut) -> None:
    """A first start creates the identity and reports it once per generated secret; reuse reports nothing."""
    sut.start()
    sut.stop()
    first = sut.log_text()

    objects = [line for line in first.splitlines() if 'event="system_object"' in line]
    identity = [line for line in objects if "object=instance_state" in line]
    assert len(identity) == 1
    assert "action=create" in identity[0]
    assert len([line for line in objects if "object=root_admin" in line]) == 1

    secrets = [line for line in first.splitlines() if 'event="secret_initialized"' in line]
    assert len(secrets) == 2
    assert sum("component=pepper" in line for line in secrets) == 1
    assert sum("component=jwt_secret" in line for line in secrets) == 1

    sut.start()
    sut.stop()
    reused = sut.log_text()[len(first) :]

    assert 'event="system_object"' not in reused
    assert 'event="secret_initialized"' not in reused


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-015")
@allure.story("SCN-LOGGING-015-D")
def test_user_administration_names_actor_and_target(sut: Sut) -> None:
    """Creating a user records one `user_admin` naming the actor, the action and the target."""
    server = start_instance(sut)
    root_token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)

    assert created.status_code == 201
    emitted = security_events(server, "user_admin")
    assert len(emitted) == 1
    assert "actor=0" in emitted[0]
    assert "action=create" in emitted[0]
    assert "target=user" in emitted[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-016")
@allure.story("SCN-LOGGING-016-A")
def test_an_ingest_reports_counts_only(sut: Sut) -> None:
    """A startup scan records the seven counts and never a file name."""
    sentinel = "incoming-sentinel.png"
    sut.write(f"{MEDIA_DIRECTORY}/{sentinel}", (FIXTURE_DIRECTORY / "cover.png").read_bytes())

    server = sut.start()
    emitted = _wait_for(server, "ingest")
    line = emitted[0]

    assert "registered=1" in line
    for counter in ("completed", "untouched", "changed", "missing", "removed", "skipped"):
        assert f"{counter}=0" in line, f"missing the {counter} counter"
    assert sentinel not in line


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-016")
@allure.story("SCN-LOGGING-016-B")
def test_a_degraded_observer_is_reported(sut: Sut) -> None:
    """An unreadable library root degrades the observer and records `ingest_fault` with its kind."""
    media = sut.dir / MEDIA_DIRECTORY
    media.mkdir()
    media.chmod(0o000)
    try:
        server = sut.start()

        emitted = _wait_for(server, "ingest_fault")
        assert "kind=watch_registration_failed" in emitted[0]
        assert 'operation="media_change_source"' in emitted[0]
    finally:
        media.chmod(0o700)


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-016")
@allure.story("SCN-LOGGING-016-C")
def test_a_skipped_provider_reports_its_classification(sut: Sut) -> None:
    """A lookup naming an unconfigured provider records only the provider and the classification."""
    config = {"library": {"metadata": {"video": {"providers": [{"name": "tmdb", "index": 1, "api_key": "test-key"}]}}}}
    server = start_instance(sut, config=config)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    reference = "tt0111161"

    response = server.request("GET", f"{server.href('movie')}/identify/omdb/{reference}", token=token)

    assert response.status_code in {200, 404}
    emitted = security_events(server, "metadata_provider_unconfigured")
    assert len(emitted) == 1
    assert "provider=omdb" in emitted[0]
    assert reference not in emitted[0]


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-017")
@allure.story("SCN-LOGGING-017-A")
def test_an_upload_lifecycle_is_recorded(sut: Sut) -> None:
    """An accepted, written, completed and cancelled upload each record one event with its outcome."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    data = (FIXTURE_DIRECTORY / "cover.png").read_bytes()

    session = begin_upload(server, token, "lifecycle-sentinel.png", data)
    assert put_chunk(server, token, session, 0, data, len(data)).status_code == 200
    assert complete_upload(server, token, session).status_code == 200
    cancelled = begin_upload(server, token, "cancelled-sentinel.png", data)
    assert server.request("DELETE", cancelled.self_href, token=token).status_code == 204

    emitted = security_events(server, "upload")
    outcomes = [re.search(r"outcome=(\w+)", line).group(1) for line in emitted]
    assert outcomes == ["accepted", "chunk_written", "completed", "accepted", "cancelled"]
    assert all("actor=0" in line for line in emitted)
    assert all("lifecycle-sentinel.png" not in line for line in emitted)


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-017")
@allure.story("SCN-LOGGING-017-B")
@pytest.mark.slow
def test_a_reaping_run_reports_counts_only(sut: Sut) -> None:
    """A reaping run records the three counts and never a file name or path."""
    server = start_instance(sut, config={"asset": {"upload": {"reap_interval_seconds": 1}}})

    emitted = _wait_for(server, "upload_reap")

    for line in emitted:
        assert "expired=" in line
        assert "orphaned=" in line
        assert "unsealed=" in line
        assert MEDIA_DIRECTORY not in line
        assert ".log" not in line


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-018")
@allure.story("SCN-LOGGING-018-A")
@allure.story("SCN-LOGGING-018-B")
@allure.feature("REQ-LOGGING-008")
@allure.story("SCN-LOGGING-008-D")
def test_a_personal_data_read_is_recorded(sut: Sut) -> None:
    """Reading personal data records the actor and the object and no personal value."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    response = server.request("GET", server.href("current_user"), token=token)

    assert response.status_code == 200
    emitted = security_events(server, "sensitive_data_access")
    assert len(emitted) == 1
    assert "actor=0" in emitted[0]
    assert "object=current_user" in emitted[0]
    assert ROOT_ADMIN_USERNAME not in emitted[0]
    assert "@" not in emitted[0]
