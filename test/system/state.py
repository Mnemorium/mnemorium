"""Black-box proof of the `state` capability.

Every case starts its own container from the image under test through the shared `harness` plugin and reads only what an
operator can: HTTP responses, standard output, the working directory and the `state.yaml` artifact. `sut` is used
directly because each case starts, stops or restarts its own container; the `server` fixture would start one implicitly.

`/health` and the current-user route are resolved through the API index (`sut.href`) and representation links, so the
system suite keeps its single base literal in `harness.Sut.index`. The state document's format belongs to the adapter;
the decode-level scenarios are proven by the Rust adapter tests.
"""

import os
import re

import allure
import pytest
import yaml
from harness import (
    CONFIG_PATH,
    DATABASE_PATH,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    STATE_PATH,
    Sut,
    hex64,
    register,
)

PEPPER_ENV = "MNEMORIUM_PEPPER"
JWT_SECRET_ENV = "MNEMORIUM_JWT_SECRET"

# The state's format version this build writes.
_STATE_VERSION = 1

SEED_A = hex64("a")
SEED_B = hex64("b")
SEED_C = hex64("c")


def _state(sut: Sut) -> dict:
    """The persisted state document."""
    return yaml.safe_load(sut.read(STATE_PATH))


def _restored(sut: Sut, mode: int, *, created_at: int = 1_700_000_000) -> str:
    """Write a valid state document at `mode` and return its digest."""
    sut.write(
        STATE_PATH,
        yaml.safe_dump(
            {
                "created_at": created_at,
                "jwt_secret": SEED_B,
                "pepper": SEED_A,
                "version": _STATE_VERSION,
            }
        ),
    )
    os.chmod(sut.dir / STATE_PATH, mode)
    return sut.sha256(STATE_PATH)


@allure.epic("State")
@allure.feature("REQ-STATE-001")
@allure.story("SCN-STATE-001-A")
@allure.feature("REQ-STATE-002")
@allure.story("SCN-STATE-002-A")
@allure.feature("REQ-STATE-003")
@allure.story("SCN-STATE-003-A")
@allure.feature("REQ-STATE-004")
@allure.story("SCN-STATE-004-A")
@allure.feature("REQ-STATE-008")
@allure.story("SCN-STATE-008-A")
def test_seeds_are_applied_at_creation(sut: Sut) -> None:
    """The environment's seeds are persisted verbatim, versioned and readable only by the server's account."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    state = _state(sut)

    assert state["version"] == _STATE_VERSION
    assert state["pepper"] == SEED_A
    assert state["jwt_secret"] == SEED_B
    assert isinstance(state["created_at"], int)
    assert sut.mode(STATE_PATH) == 0o600


@allure.epic("State")
@allure.feature("REQ-STATE-004")
@allure.story("SCN-STATE-004-B")
@allure.feature("REQ-STATE-006")
@allure.story("SCN-STATE-006-B")
def test_secrets_are_generated_when_absent(sut: Sut) -> None:
    """An unseeded first start generates both secrets and creates the state before the datastore."""
    sut.start()

    state = _state(sut)

    assert re.fullmatch(r"[0-9a-f]{64}", state["pepper"])
    assert re.fullmatch(r"[0-9a-f]{64}", state["jwt_secret"])
    assert state["pepper"] != state["jwt_secret"]
    assert sut.exists(DATABASE_PATH)


@allure.epic("State")
@allure.feature("REQ-STATE-003")
@allure.story("SCN-STATE-003-B")
@allure.feature("REQ-STATE-005")
@allure.story("SCN-STATE-005-B")
def test_a_persisted_identity_is_reused_silently(sut: Sut) -> None:
    """A restart over the same directory keeps the file byte-identical and starts healthy."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    before = sut.sha256(STATE_PATH)
    sut.stop()

    sut.write(CONFIG_PATH, "logging:\n  level: info\n")
    sut.restart(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.sha256(STATE_PATH) == before


@allure.epic("State")
@allure.feature("REQ-STATE-002")
@allure.story("SCN-STATE-002-B")
def test_the_signing_secret_survives_a_restart(sut: Sut) -> None:
    """A token issued before the restart still validates afterwards."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    token = sut.login(ROOT_ADMIN_USERNAME, sut.root_password())

    sut.restart()

    assert sut.request("GET", sut.href("current_user"), token=token).status_code == 200


@allure.epic("State")
@allure.feature("REQ-STATE-001")
@allure.story("SCN-STATE-001-B")
def test_the_pepper_survives_a_restart(sut: Sut) -> None:
    """A password hashed before the restart still verifies afterwards."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    token = sut.login(ROOT_ADMIN_USERNAME, sut.root_password())
    register(sut, token, "alice", "alice@example.com", SECRET_PASSWORD)

    sut.restart()

    assert sut.login("alice", SECRET_PASSWORD)


@allure.epic("State")
@allure.feature("REQ-STATE-004")
@allure.story("SCN-STATE-004-C")
def test_a_changed_pepper_is_refused(sut: Sut) -> None:
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


@allure.epic("State")
@allure.feature("REQ-STATE-004")
@allure.story("SCN-STATE-004-C")
def test_a_changed_signing_secret_is_refused(sut: Sut) -> None:
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


@allure.epic("State")
@allure.feature("REQ-STATE-004")
@allure.story("SCN-STATE-004-C")
def test_both_changed_seeds_are_refused(sut: Sut) -> None:
    """Two changed seeds fail startup without either value reaching the output."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    sut.stop()
    before = sut.sha256(STATE_PATH)

    failure = sut.start_expecting_failure(env={PEPPER_ENV: SEED_C, JWT_SECRET_ENV: SEED_C})

    assert failure.exit_code != 0
    assert SEED_A not in failure.output
    assert SEED_C not in failure.output
    assert sut.sha256(STATE_PATH) == before


@allure.epic("State")
@allure.feature("REQ-STATE-004")
@allure.story("SCN-STATE-004-D")
def test_a_matching_seed_is_accepted(sut: Sut) -> None:
    """Seeds equal to the persisted ones let startup proceed and change nothing."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    before = sut.sha256(STATE_PATH)

    sut.restart(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.sha256(STATE_PATH) == before


@allure.epic("State")
@allure.feature("REQ-STATE-004")
@allure.story("SCN-STATE-004-E")
@pytest.mark.parametrize("variable", [PEPPER_ENV, JWT_SECRET_ENV])
def test_a_malformed_seed_is_refused(sut: Sut, variable: str) -> None:
    """A seed that is not 64 hexadecimal characters stops startup, naming the variable and never the value."""
    value = "not-a-64-character-hexadecimal-seed"

    failure = sut.start_expecting_failure(env={variable: value})

    assert failure.exit_code != 0
    assert variable in failure.output
    assert value not in failure.output


@allure.epic("State")
@allure.feature("REQ-STATE-005")
@allure.story("SCN-STATE-005-A")
def test_an_unreadable_identity_stops_startup_early(sut: Sut) -> None:
    """An unreadable identity stops startup before any datastore file exists."""
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
    assert not sut.exists(DATABASE_PATH)


@allure.epic("State")
@allure.feature("REQ-STATE-006")
@allure.story("SCN-STATE-006-A")
def test_a_datastore_without_an_identity_is_refused(sut: Sut) -> None:
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


@allure.epic("State")
@allure.feature("REQ-STATE-006")
@allure.story("SCN-STATE-006-C")
def test_an_identity_without_a_datastore_is_reused(sut: Sut) -> None:
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

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.sha256(STATE_PATH) == before
    assert sut.exists(DATABASE_PATH)

    # A fresh datastore seeds its own Root Admin: the old credential is gone, the new one works.
    assert sut.attempt_login(ROOT_ADMIN_USERNAME, stale_password).status_code == 401
    assert sut.login(ROOT_ADMIN_USERNAME, sut.root_password())


@allure.epic("State")
@allure.feature("REQ-STATE-007")
@allure.story("SCN-STATE-007-E")
def test_the_state_is_written_at_the_fixed_path(sut: Sut) -> None:
    """A created state lives at `state.yaml` in the working directory."""
    sut.start()

    assert sut.exists(STATE_PATH)
    assert _state(sut)["version"] == _STATE_VERSION


@allure.epic("State")
@allure.feature("REQ-STATE-008")
@allure.story("SCN-STATE-008-B")
def test_a_restored_state_with_a_wider_mode_is_repaired(sut: Sut) -> None:
    """A restored state readable by another account is tightened to `0600` before it is read and reused on restart."""
    before = _restored(sut, 0o644)

    sut.start()

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.mode(STATE_PATH) == 0o600
    assert sut.sha256(STATE_PATH) == before
    sut.stop()

    sut.start()

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.sha256(STATE_PATH) == before
    assert sut.mode(STATE_PATH) == 0o600


@allure.epic("State")
@allure.feature("REQ-STATE-008")
@allure.story("SCN-STATE-008-C")
def test_a_private_restored_state_is_accepted(sut: Sut) -> None:
    """A restored state already private to the server account starts and is reused unchanged."""
    before = _restored(sut, 0o600)

    sut.start()

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.mode(STATE_PATH) == 0o600
    assert sut.sha256(STATE_PATH) == before
