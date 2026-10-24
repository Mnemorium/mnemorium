"""UC-004 Load Configuration and UC-012 Load Instance State, observed at boot.

The two startup use cases have no REST surface of their own, so the container's
boot lifecycle is their black-box interface: a fresh boot writes the identity,
the default configuration and the datastore; a restart preserves the identity;
an empty configuration file boots on the built-in defaults; and a datastore
without its identity refuses to start.
"""

from collections.abc import Callable

from conftest import BootContainer

STATE_PATH = "/app/state.yaml"
CONFIG_PATH = "/app/config.yaml"
DATABASE_PATH = "/app/mnemorium.db"


def test_fresh_boot_creates_state_configuration_and_datastore(
    boot_container: Callable[[], BootContainer],
) -> None:
    """A fresh instance writes the identity before the configuration and the datastore."""
    container = boot_container()

    container.start()
    container.wait_healthy()

    entries = container.exec("ls", "-1", "/app").splitlines()
    assert "state.yaml" in entries, f"a fresh boot must write the identity: {entries}"
    assert "config.yaml" in entries, f"a fresh boot must write the default configuration: {entries}"
    assert "mnemorium.db" in entries, f"a fresh boot must create the datastore: {entries}"


def test_created_identity_carries_secrets_and_the_configuration_does_not(
    boot_container: Callable[[], BootContainer],
) -> None:
    """The identity holds both secrets; the generated configuration holds neither."""
    container = boot_container()

    container.start()
    container.wait_healthy()

    identity = container.exec("cat", STATE_PATH)
    assert "pepper:" in identity, f"the identity must carry the pepper: {identity}"
    assert "jwt_secret:" in identity, f"the identity must carry the signing secret: {identity}"
    configuration = container.exec("cat", CONFIG_PATH)
    assert "pepper:" not in configuration, f"the configuration must not carry the pepper: {configuration}"
    assert "jwt_secret:" not in configuration, f"the configuration must not carry the secret: {configuration}"


def test_identity_is_private_to_the_server_account(
    boot_container: Callable[[], BootContainer],
) -> None:
    """Only the account the server runs as may read or write the identity."""
    container = boot_container()

    container.start()
    container.wait_healthy()

    mode = container.exec("stat", "-c", "%a", STATE_PATH).strip()
    assert mode == "600", f"the identity must be mode 0600, got {mode}"


def test_restart_preserves_the_identity(boot_container: Callable[[], BootContainer]) -> None:
    """A restart reuses the persisted identity instead of generating a new one."""
    container = boot_container()

    container.start()
    container.wait_healthy()
    before = container.exec("cat", STATE_PATH)

    container.restart()
    container.wait_healthy()
    after = container.exec("cat", STATE_PATH)

    assert after == before, "a restart must not replace the identity"


def test_empty_configuration_boots_on_the_defaults(
    boot_container: Callable[[], BootContainer],
) -> None:
    """An empty configuration file is valid: every setting has a default."""
    container = boot_container()

    container.start()
    container.wait_healthy()
    container.exec("sh", "-c", f": > {CONFIG_PATH}")

    container.restart()
    container.wait_healthy()

    assert container.exec("cat", CONFIG_PATH) == "", "an existing configuration file is never rewritten"


def test_datastore_without_identity_refuses_to_start(
    boot_container: Callable[[], BootContainer],
) -> None:
    """A datastore with no identity fails startup instead of regenerating secrets."""
    container = boot_container()

    container.start()
    container.wait_healthy()
    container.exec("rm", STATE_PATH)
    container.recreate()

    exit_code = container.wait_exited()

    assert exit_code != 0, "a missing identity for an existing datastore must fail startup"
    logs = container.logs()
    assert "state.yaml" in logs, f"the failure must name the missing identity file: {logs}"
