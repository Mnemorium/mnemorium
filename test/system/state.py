"""Black-box proof of the `state` capability."""

import os
import re

import pytest
import yaml
from conftest import login
from harness import (
    CONFIG_PATH,
    DATABASE_PATH,
    MSG_STATE_OVERSIZED,
    MSG_STATE_UNSUPPORTED_VERSION,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    STATE_PATH,
    Sut,
    hex64,
    register,
)
from labels import TestType, alternate_flow, error_handling, positive, recovery, scenario, security
from mnemorium_client.api.identity import post_login
from mnemorium_client.models import LoginRequest

# State owns its instances: every case pre-places `state.yaml`, restarts the container or observes a refused start.
pytestmark = pytest.mark.own_instance(reason="global_filesystem")

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


def _document(
    *,
    version: int = _STATE_VERSION,
    created_at: int = 1_700_000_000,
    extra: dict | None = None,
) -> str:
    """A valid state document, optionally at another version or with an extra field."""
    fields = {
        "created_at": created_at,
        "jwt_secret": SEED_B,
        "pepper": SEED_A,
        "version": version,
    }
    fields.update(extra or {})
    return yaml.safe_dump(fields)


def _placed(sut: Sut, document: str, *, mode: int = 0o600) -> str:
    """Place a raw state document at `mode` and return its digest."""
    sut.write(STATE_PATH, document)
    os.chmod(sut.dir / STATE_PATH, mode)
    return sut.sha256(STATE_PATH)


def _restored(sut: Sut, mode: int, *, created_at: int = 1_700_000_000) -> str:
    """Write a valid state document at `mode` and return its digest."""
    return _placed(sut, _document(created_at=created_at), mode=mode)


@scenario("SCN-STATE-001-A", TestType.POSITIVE)
@scenario("SCN-STATE-002-A", TestType.POSITIVE)
@scenario("SCN-STATE-003-A", TestType.POSITIVE)
@scenario("SCN-STATE-004-A", TestType.POSITIVE)
@scenario("SCN-STATE-008-A", TestType.SECURITY)
def test_seeds_are_applied_at_creation(sut: Sut) -> None:
    """The environment's seeds are persisted verbatim, versioned and readable only by the server's account."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    state = _state(sut)

    assert state["version"] == _STATE_VERSION
    assert state["pepper"] == SEED_A
    assert state["jwt_secret"] == SEED_B
    assert isinstance(state["created_at"], int)
    assert sut.mode(STATE_PATH) == 0o600


@positive
@scenario("SCN-STATE-004-B")
@scenario("SCN-STATE-006-B")
def test_secrets_are_generated_when_absent(sut: Sut) -> None:
    """An unseeded first start generates both secrets and creates the state before the datastore."""
    sut.start()

    state = _state(sut)

    assert re.fullmatch(r"[0-9a-f]{64}", state["pepper"])
    assert re.fullmatch(r"[0-9a-f]{64}", state["jwt_secret"])
    assert state["pepper"] != state["jwt_secret"]
    assert sut.exists(DATABASE_PATH)


@recovery
@scenario("SCN-STATE-003-B")
@scenario("SCN-STATE-005-B")
def test_a_persisted_identity_is_reused_silently(sut: Sut) -> None:
    """A restart over the same directory keeps the file byte-identical and starts healthy."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    before = sut.sha256(STATE_PATH)
    sut.stop()

    sut.write(CONFIG_PATH, "logging:\n  level: info\n")
    sut.restart(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.sha256(STATE_PATH) == before


@recovery
@scenario("SCN-STATE-002-B")
def test_the_signing_secret_survives_a_restart(sut: Sut) -> None:
    """A token issued before the restart still validates afterwards."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    token = login(sut, ROOT_ADMIN_USERNAME, sut.root_password())

    sut.restart()

    assert sut.request("GET", sut.href("current_user"), token=token).status_code == 200


@recovery
@scenario("SCN-STATE-001-B")
def test_the_pepper_survives_a_restart(sut: Sut) -> None:
    """A password hashed before the restart still verifies afterwards."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    token = login(sut, ROOT_ADMIN_USERNAME, sut.root_password())
    register(sut, token, "alice", "alice@example.com", SECRET_PASSWORD)

    sut.restart()

    assert login(sut, "alice", SECRET_PASSWORD)


@error_handling
@scenario("SCN-STATE-004-C")
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


@error_handling
@scenario("SCN-STATE-004-C")
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


@error_handling
@scenario("SCN-STATE-004-C")
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


@alternate_flow
@scenario("SCN-STATE-004-D")
def test_a_matching_seed_is_accepted(sut: Sut) -> None:
    """Seeds equal to the persisted ones let startup proceed and change nothing."""
    sut.start(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})
    before = sut.sha256(STATE_PATH)

    sut.restart(env={PEPPER_ENV: SEED_A, JWT_SECRET_ENV: SEED_B})

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.sha256(STATE_PATH) == before


@error_handling
@scenario("SCN-STATE-004-E")
@pytest.mark.parametrize("variable", [PEPPER_ENV, JWT_SECRET_ENV])
def test_a_malformed_seed_is_refused(sut: Sut, variable: str) -> None:
    """A seed that is not 64 hexadecimal characters stops startup, naming the variable and never the value."""
    value = "not-a-64-character-hexadecimal-seed"

    failure = sut.start_expecting_failure(env={variable: value})

    assert failure.exit_code != 0
    assert variable in failure.output
    assert value not in failure.output


@error_handling
@scenario("SCN-STATE-005-A")
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


@error_handling
@scenario("SCN-STATE-006-A")
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


@recovery
@scenario("SCN-STATE-006-C")
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
    assert (
        post_login.sync_detailed(
            client=sut.client(), body=LoginRequest(password=stale_password, identifier=ROOT_ADMIN_USERNAME)
        ).status_code
        == 401
    )
    assert login(sut, ROOT_ADMIN_USERNAME, sut.root_password())


@error_handling
@scenario("SCN-STATE-006-D")
def test_an_undecidable_datastore_presence_stops_startup(sut: Sut) -> None:
    """A datastore path that cannot be probed stops startup before a state is written."""
    sealed = sut.dir / "sealed"
    sealed.mkdir()
    (sealed / DATABASE_PATH).write_bytes(b"")
    os.chmod(sealed, 0o000)
    (sut.dir / DATABASE_PATH).symlink_to(f"sealed/{DATABASE_PATH}")
    try:
        failure = sut.start_expecting_failure()
    finally:
        os.chmod(sealed, 0o700)

    assert failure.exit_code != 0
    assert not sut.exists(STATE_PATH)
    assert not sut.exists(CONFIG_PATH)


@positive
@scenario("SCN-STATE-007-E")
def test_the_state_is_written_at_the_fixed_path(sut: Sut) -> None:
    """A created state lives at `state.yaml` in the working directory."""
    sut.start()

    assert sut.exists(STATE_PATH)
    assert _state(sut)["version"] == _STATE_VERSION


@error_handling
@scenario("SCN-STATE-007-A")
def test_a_corrupt_state_is_not_regenerated(sut: Sut) -> None:
    """A truncated state stops startup and is never replaced."""
    before = _placed(sut, f'version: {_STATE_VERSION}\ncreated_at: 1\npepper: "{SEED_A[:8]}')

    failure = sut.start_expecting_failure()

    assert failure.exit_code != 0
    assert sut.sha256(STATE_PATH) == before
    assert not sut.exists(CONFIG_PATH)
    assert not sut.exists(DATABASE_PATH)
    assert SEED_A not in failure.output


@error_handling
@scenario("SCN-STATE-007-B")
def test_an_unsupported_format_version_is_refused(sut: Sut) -> None:
    """A state declaring another format version stops startup naming that version."""
    version = _STATE_VERSION + 1
    before = _placed(sut, _document(version=version))

    failure = sut.start_expecting_failure()

    assert failure.exit_code != 0
    assert f"{MSG_STATE_UNSUPPORTED_VERSION} {version}" in failure.output
    assert sut.sha256(STATE_PATH) == before
    assert not sut.exists(CONFIG_PATH)
    assert not sut.exists(DATABASE_PATH)
    assert SEED_A not in failure.output


@error_handling
@scenario("SCN-STATE-007-C")
def test_an_oversized_state_is_refused(sut: Sut) -> None:
    """A valid state padded past 4 KiB stops startup on its size, before any decode."""
    document = f"{_document()}# {'padding' * 700}\n"
    assert len(document.encode()) > 4096
    before = _placed(sut, document)

    failure = sut.start_expecting_failure()

    assert failure.exit_code != 0
    assert MSG_STATE_OVERSIZED in failure.output
    assert sut.sha256(STATE_PATH) == before
    assert not sut.exists(CONFIG_PATH)
    assert not sut.exists(DATABASE_PATH)
    assert SEED_A not in failure.output


@error_handling
@scenario("SCN-STATE-007-D")
def test_an_unknown_state_field_is_refused(sut: Sut) -> None:
    """A state carrying an undeclared field stops startup and is not replaced."""
    before = _placed(sut, _document(extra={"instance_id": 7}))

    failure = sut.start_expecting_failure()

    assert failure.exit_code != 0
    assert sut.sha256(STATE_PATH) == before
    assert not sut.exists(CONFIG_PATH)
    assert not sut.exists(DATABASE_PATH)
    assert SEED_A not in failure.output


@error_handling
@scenario("SCN-STATE-007-F")
def test_a_failed_state_write_leaves_no_partial_file(sut: Sut) -> None:
    """A temp file that cannot be created stops startup without writing `state.yaml`."""
    (sut.dir / f"{STATE_PATH}.tmp").mkdir()

    failure = sut.start_expecting_failure()

    assert failure.exit_code != 0
    assert not sut.exists(STATE_PATH)
    assert not sut.exists(CONFIG_PATH)
    assert not sut.exists(DATABASE_PATH)


@security
@scenario("SCN-STATE-008-B")
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


@security
@scenario("SCN-STATE-008-C")
def test_a_private_restored_state_is_accepted(sut: Sut) -> None:
    """A restored state already private to the server account starts and is reused unchanged."""
    before = _restored(sut, 0o600)

    sut.start()

    assert sut.request("GET", sut.href("health")).status_code == 200
    assert sut.mode(STATE_PATH) == 0o600
    assert sut.sha256(STATE_PATH) == before


@security
@scenario("SCN-STATE-008-E")
def test_a_state_path_that_is_not_a_regular_file_is_refused(sut: Sut) -> None:
    """A state path that is not a regular file stops startup and is left unchanged."""
    state_path = sut.dir / STATE_PATH
    state_path.mkdir()
    os.chmod(state_path, 0o750)

    failure = sut.start_expecting_failure()

    assert failure.exit_code != 0
    assert state_path.is_dir()
    assert sut.mode(STATE_PATH) == 0o750
    assert not sut.exists(CONFIG_PATH)
    assert not sut.exists(DATABASE_PATH)
