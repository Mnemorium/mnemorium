"""Behavioural cases for the instance identity (`state.yaml`).

The identity is the non-derivable part of the instance: the site pepper, the token signing secret and the instant it was
established. The cases cover the lifecycle the spec at `openspec/specs/instance/state/spec.md` describes — seeded,
generated, reused verbatim across restarts, refused when a seed disagrees with the persisted file, and never leaked.

Seeds are literal so a leak assertion is a substring search, and every refusal case asserts its effect (`TEST-061`): the
persisted file stays byte-identical, no replacement identity is written, and no configuration or datastore file appears.
"""

import os
import re

import pytest
import yaml
from harness import (
    CONFIG_PATH,
    DATABASE_PATH,
    HEALTH_PATH,
    ME_PATH,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    STATE_PATH,
    hex64,
    register,
)

PEPPER_ENV = "MNEMORIUM_PEPPER"
JWT_SECRET_ENV = "MNEMORIUM_JWT_SECRET"

# The identity's format version this build writes.
_STATE_VERSION = 1

SEED_A = hex64("a")
SEED_B = hex64("b")
SEED_C = hex64("c")

RESOLUTION = "Identity is resolved before configuration and the datastore"
SECRETS = "Identity holds only non-derivable secrets"
SEEDS = "Environment seeds apply only at creation"
MISSING = "A missing identity for an existing datastore fails startup"
PERSISTENCE = "Identity persistence is atomic, versioned and private"
REPORT = "Identity creation is reported and secrets never leak"

# Every scenario of openspec/specs/instance/state/spec.md, mapped to the test that asserts it.
# A value is a test function name in this module, or "rust:<test fn>" when the scenario is
# decode-level and owned by the Rust store tests (TEST-057).
STATE_SCENARIOS: dict[str, str] = {
    f"{RESOLUTION} / Unreadable identity stops startup early": "test_the_identity_is_read_before_configuration",
    f"{RESOLUTION} / Identity is read on every start": "test_a_persisted_identity_is_reused_silently",
    f"{SECRETS} / Identity carries the two secrets": "test_seeds_are_applied_at_creation",
    f"{SECRETS} / Configuration changes do not alter identity": "test_a_persisted_identity_is_reused_silently",
    f"{SEEDS} / Seed is used at creation": "test_seeds_are_applied_at_creation",
    f"{SEEDS} / Secrets are generated when not seeded": "test_secrets_are_generated_when_absent",
    f"{SEEDS} / A differing seed fails startup": "test_a_changed_signing_secret_is_refused",
    f"{SEEDS} / A matching seed is accepted": "test_a_matching_seed_is_accepted",
    f"{MISSING} / Existing datastore without identity": "test_a_datastore_without_an_identity_is_refused",
    f"{MISSING} / Fresh instance": "test_secrets_are_generated_when_absent",
    f"{MISSING} / Identity without a datastore": "test_an_identity_without_a_datastore_is_reused",
    f"{PERSISTENCE} / Corrupt identity is not regenerated": (
        "rust:file_instance_state_store_truncated_document_is_rejected"
    ),
    f"{PERSISTENCE} / Newer format is refused": "rust:file_instance_state_store_unsupported_version_is_named",
    f"{PERSISTENCE} / Identity is private": "test_seeds_are_applied_at_creation",
    f"{PERSISTENCE} / No partially written identity": (
        "rust:file_instance_state_store_never_replaces_an_existing_identity"
    ),
    f"{REPORT} / Creation is reported": "test_secrets_are_generated_when_absent",
    f"{REPORT} / A seeded secret reports no generation": "test_a_seeded_secret_is_not_reported_as_generated",
    f"{REPORT} / Reuse emits no creation event": "test_a_persisted_identity_is_reused_silently",
    f"{REPORT} / Secrets never reach a sink": "test_no_secret_reaches_a_sink",
}


def _state(sut) -> dict:
    """The persisted identity document."""
    return yaml.safe_load(sut.read(STATE_PATH))


def _creation_events(logs: str) -> int:
    """How many times the logs report the identity's creation."""
    return logs.count("object=instance_state action=create")


def _secret_events(logs: str, component: str) -> int:
    """How many times the logs report a generated `component` secret."""
    return logs.count(f'event="secret_initialized" component={component}')


@pytest.mark.state_scenario(f"{SEEDS} / Seed is used at creation")
@pytest.mark.state_scenario(f"{SECRETS} / Identity carries the two secrets")
@pytest.mark.state_scenario(f"{PERSISTENCE} / Identity is private")
def test_seeds_are_applied_at_creation(sut):
    """The environment's secrets are persisted verbatim, versioned and readable only by the server's account."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    state = _state(sut)

    assert state["version"] == _STATE_VERSION
    assert state["pepper"] == SEED_A
    assert state["jwt_secret"] == SEED_B
    assert isinstance(state["created_at"], int)
    assert sut.mode(STATE_PATH) == 0o600


@pytest.mark.state_scenario(f"{SEEDS} / Secrets are generated when not seeded")
@pytest.mark.state_scenario(f"{REPORT} / Creation is reported")
@pytest.mark.state_scenario(f"{MISSING} / Fresh instance")
def test_secrets_are_generated_when_absent(sut):
    """An unseeded first start generates both secrets and reports the creation once."""
    sut.start()

    state = _state(sut)

    assert re.fullmatch(r"[0-9a-f]{64}", state["pepper"])
    assert re.fullmatch(r"[0-9a-f]{64}", state["jwt_secret"])
    assert state["pepper"] != state["jwt_secret"]
    assert sut.exists(DATABASE_PATH)

    sut.stop()
    logs = sut.log_text()

    assert _creation_events(logs) == 1
    assert _secret_events(logs, "pepper") == 1
    assert _secret_events(logs, "jwt_secret") == 1


@pytest.mark.state_scenario(f"{REPORT} / A seeded secret reports no generation")
def test_a_seeded_secret_is_not_reported_as_generated(sut):
    """Only the secret this run generated is reported as initialized."""
    sut.start(env={PEPPER_ENV: SEED_A})
    sut.stop()
    logs = sut.log_text()

    assert logs.count('event="secret_initialized"') == 1
    assert _secret_events(logs, "jwt_secret") == 1
    assert _secret_events(logs, "pepper") == 0


@pytest.mark.state_scenario(f"{RESOLUTION} / Identity is read on every start")
@pytest.mark.state_scenario(f"{SECRETS} / Configuration changes do not alter identity")
@pytest.mark.state_scenario(f"{REPORT} / Reuse emits no creation event")
def test_a_persisted_identity_is_reused_silently(sut):
    """A restart over the same directory keeps the file byte-identical and emits no creation event."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    before = sut.sha256(STATE_PATH)
    sut.stop()
    first_run = sut.log_text()

    sut.write(CONFIG_PATH, "logging:\n  level: info\n")
    sut.restart(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    assert sut.request("GET", HEALTH_PATH).status_code == 200
    assert sut.sha256(STATE_PATH) == before

    sut.stop()
    second_run = sut.log_text()[len(first_run) :]

    assert _creation_events(second_run) == 0
    assert 'event="secret_initialized"' not in second_run


@pytest.mark.state_scenario(f"{RESOLUTION} / Identity is read on every start")
def test_the_signing_secret_survives_a_restart(sut):
    """A token issued before the restart still validates afterwards."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    token = sut.login(ROOT_ADMIN_USERNAME, sut.root_password())

    sut.restart()

    assert sut.request("GET", ME_PATH, token=token).status_code == 200


@pytest.mark.state_scenario(f"{RESOLUTION} / Identity is read on every start")
def test_the_pepper_survives_a_restart(sut):
    """A password hashed before the restart still verifies afterwards."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    token = sut.login(ROOT_ADMIN_USERNAME, sut.root_password())
    register(sut, token, "alice", "alice@example.com", SECRET_PASSWORD)

    sut.restart()

    assert sut.login("alice", SECRET_PASSWORD)


@pytest.mark.state_scenario(f"{SEEDS} / A differing seed fails startup")
def test_a_changed_pepper_is_refused(sut):
    """A pepper that disagrees with the persisted one stops startup and leaves the file untouched."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    sut.stop()
    before = sut.sha256(STATE_PATH)

    failure = sut.start_expecting_failure(env={PEPPER_ENV: SEED_C, JWT_SECRET_ENV: SEED_B})

    assert failure.exit_code != 0
    assert PEPPER_ENV in failure.output
    assert SEED_A not in failure.output
    assert SEED_C not in failure.output
    assert sut.sha256(STATE_PATH) == before


@pytest.mark.state_scenario(f"{SEEDS} / A differing seed fails startup")
def test_a_changed_signing_secret_is_refused(sut):
    """A signing secret that disagrees with the persisted one stops startup, naming its variable."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    sut.stop()
    before = sut.sha256(STATE_PATH)

    failure = sut.start_expecting_failure(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_C})

    assert failure.exit_code != 0
    assert JWT_SECRET_ENV in failure.output
    assert SEED_A not in failure.output
    assert SEED_C not in failure.output
    assert sut.sha256(STATE_PATH) == before


@pytest.mark.state_scenario(f"{SEEDS} / A differing seed fails startup")
def test_both_changed_seeds_are_refused(sut):
    """Two changed seeds fail startup without either value reaching the output."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    sut.stop()
    before = sut.sha256(STATE_PATH)

    failure = sut.start_expecting_failure(env={PEPPER_ENV: SEED_C, JWT_SECRET_ENV: SEED_C})

    assert failure.exit_code != 0
    assert SEED_A not in failure.output
    assert SEED_C not in failure.output
    assert sut.sha256(STATE_PATH) == before


@pytest.mark.state_scenario(f"{SEEDS} / A matching seed is accepted")
def test_a_matching_seed_is_accepted(sut):
    """Seeds equal to the persisted ones let startup proceed and change nothing."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    before = sut.sha256(STATE_PATH)

    sut.restart(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    assert sut.request("GET", HEALTH_PATH).status_code == 200
    assert sut.sha256(STATE_PATH) == before


@pytest.mark.state_scenario(f"{MISSING} / Existing datastore without identity")
def test_a_datastore_without_an_identity_is_refused(sut):
    """A datastore with no identity stops startup: no identity is written and the datastore is untouched."""
    sut.start()
    sut.stop()
    database = sut.sha256(DATABASE_PATH)
    (sut.dir / STATE_PATH).unlink()

    failure = sut.start_expecting_failure()

    assert failure.exit_code != 0
    assert STATE_PATH in failure.output
    assert not sut.exists(STATE_PATH)
    assert sut.sha256(DATABASE_PATH) == database


@pytest.mark.state_scenario(f"{MISSING} / Identity without a datastore")
def test_an_identity_without_a_datastore_is_reused(sut):
    """A persisted identity outlives its datastore: the file is reused and the datastore recreated."""
    sut.start()
    stale_password = sut.root_password()
    before = sut.sha256(STATE_PATH)
    sut.stop()

    for name in (DATABASE_PATH, f"{DATABASE_PATH}-wal", f"{DATABASE_PATH}-shm"):
        path = sut.dir / name
        if path.exists():
            path.unlink()

    sut.start()

    assert sut.request("GET", HEALTH_PATH).status_code == 200
    assert sut.sha256(STATE_PATH) == before
    assert sut.exists(DATABASE_PATH)

    # A fresh datastore seeds its own Root Admin: the old credential is gone, the new one works.
    assert sut.attempt_login(ROOT_ADMIN_USERNAME, stale_password).status_code == 401
    assert sut.login(ROOT_ADMIN_USERNAME, sut.root_password())


@pytest.mark.state_scenario(f"{RESOLUTION} / Unreadable identity stops startup early")
def test_the_identity_is_read_before_configuration(sut):
    """An unreadable identity stops startup before any configuration or datastore file exists."""
    sut.write(
        STATE_PATH,
        yaml.safe_dump(
            {
                "created_at": 0,
                "jwt_secret": SEED_B,
                "pepper": SEED_A,
                "version": _STATE_VERSION,
            }
        ),
    )
    os.chmod(sut.dir / STATE_PATH, 0o000)
    try:
        failure = sut.start_expecting_failure()
    finally:
        os.chmod(sut.dir / STATE_PATH, 0o600)

    assert failure.exit_code != 0
    assert not sut.exists(CONFIG_PATH)
    assert not sut.exists(DATABASE_PATH)


@pytest.mark.parametrize("arrangement", ["seeded", "generated", "refused_pepper", "refused_jwt"])
@pytest.mark.state_scenario(f"{REPORT} / Secrets never reach a sink")
def test_no_secret_reaches_a_sink(sut, arrangement):
    """Neither the persisted secrets nor a refused seed appear on stdout or in the log files."""
    if arrangement == "seeded":
        sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
        secrets = [SEED_A, SEED_B]
        stdout = sut.logs()
    elif arrangement == "generated":
        sut.start()
        state = _state(sut)
        secrets = [state["pepper"], state["jwt_secret"]]
        stdout = sut.logs()
    else:
        sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
        sut.stop()
        env = (
            {PEPPER_ENV: SEED_C, JWT_SECRET_ENV: SEED_B}
            if arrangement == "refused_pepper"
            else {PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_C}
        )
        failure = sut.start_expecting_failure(env=env)
        secrets = [SEED_A, SEED_B, SEED_C]
        stdout = failure.output

    sut.stop()
    written = sut.log_text()

    for secret in secrets:
        assert secret not in stdout
        assert secret not in written
