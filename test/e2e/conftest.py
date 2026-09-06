import subprocess
import time

import pytest
import requests

_IMAGE = "mnemorium-e2e"
_CONTAINER = "mnemorium-e2e"
_CONTAINER_PORT = 4080
_HOST_PORT = 4080
_BASE_URL = f"http://127.0.0.1:{_HOST_PORT}"
_HEALTH_PATH = "/health"
_HEALTH_TIMEOUT_S = 60
_POLL_INTERVAL_S = 0.5


def _docker(*args: str) -> None:
    subprocess.run(["docker", *args], check=True)


@pytest.fixture(scope="session")
def server_url() -> str:
    """Build the image, start a container, wait for health, tear it down."""
    _docker("build", "-t", _IMAGE, ".")
    subprocess.run(["docker", "rm", "-f", _CONTAINER], capture_output=True, check=False)

    _docker(
        "run",
        "-d",
        "--rm",
        "--name",
        _CONTAINER,
        "-p",
        f"{_HOST_PORT}:{_CONTAINER_PORT}",
        _IMAGE,
    )

    try:
        deadline = time.monotonic() + _HEALTH_TIMEOUT_S
        while time.monotonic() < deadline:
            try:
                response = requests.get(f"{_BASE_URL}{_HEALTH_PATH}", timeout=1)
                if response.ok:
                    yield _BASE_URL
                    return
            except requests.ConnectionError:
                pass
            time.sleep(_POLL_INTERVAL_S)
        raise RuntimeError(f"server not healthy within {_HEALTH_TIMEOUT_S}s")
    finally:
        _docker("stop", _CONTAINER)
