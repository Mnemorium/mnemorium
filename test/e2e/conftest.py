import os
import re
import subprocess
import time

import pytest
import requests

_BASE_URL = "http://127.0.0.1:4080"
_BASE_URL_ENV = "MNEMORIUM_E2E_BASE_URL"
_CONTAINER = "mnemorium-e2e"
_HEALTH_PATH = "/health"
_HEALTH_TIMEOUT_S = 60
_POLL_INTERVAL_S = 0.5

_DEFAULT_PASSWORD_RE = re.compile(
    r"Root admin initialized; use the default password to authenticate and change it: '([^']+)'"
)


@pytest.fixture(scope="session")
def server_url() -> str:
    """Return the base URL of a running server, once it is healthy."""
    base_url = os.environ.get(_BASE_URL_ENV, _BASE_URL)

    deadline = time.monotonic() + _HEALTH_TIMEOUT_S
    while time.monotonic() < deadline:
        try:
            response = requests.get(f"{base_url}{_HEALTH_PATH}", timeout=1)
            if response.ok:
                return base_url
        except requests.ConnectionError:
            pass
        time.sleep(_POLL_INTERVAL_S)
    raise RuntimeError(
        f"server not healthy within {_HEALTH_TIMEOUT_S}s at {base_url}; start it first, e.g. with `devenv server`"
    )


@pytest.fixture(scope="session")
def default_password() -> str:
    """Return the root admin default password printed to the container stdout."""
    result = subprocess.run(
        ["docker", "logs", _CONTAINER],
        capture_output=True,
        text=True,
        check=True,
    )
    match = _DEFAULT_PASSWORD_RE.search(result.stdout)
    if match is None:
        raise RuntimeError("root admin default password not found in container stdout")
    return match.group(1)
