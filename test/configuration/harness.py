"""Harness for the configuration test suite: one containerised server per test.

Every test drives the image named by `MNEMORIUM_IMAGE` as a black box. The image is never built here: a missing image
is a configuration error and stops the run with a hint, not an implicit build (TEST-060). Each test gets its own
container, its own writable working directory bind-mounted at `/app`, and its own random loopback port, so no test
observes another test's server, the `test/e2e` container, or the repository working tree.

Operator prose lives in this module only (TEST-059): a test matches a message through the constants below and never
inlines one. The structured `event=`/`component=` fields a lifecycle message carries are matched where the server emits
them.

The module is loaded as the `harness` pytest plugin (`pytest.ini`), not as a `conftest.py`: a second module named
`conftest` would shadow `test/e2e/conftest.py` in `sys.modules`, and the E2E modules import their helpers from it by
name.
"""

import base64
import hashlib
import os
import re
import shutil
import subprocess
import time
import uuid
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path
from stat import S_IMODE

import pytest
import requests
import yaml

IMAGE_ENV = "MNEMORIUM_IMAGE"
DEFAULT_IMAGE = "mnemorium:latest"
CONTAINER_PREFIX = "mnemorium-config-"
CONTAINER_PORT = 4080
HEALTH_PATH = "/health"
API_PREFIX = "/api/v1"
LOGIN_PATH = f"{API_PREFIX}/identity/login"
REGISTER_PATH = f"{API_PREFIX}/identity/register"
ME_PATH = f"{API_PREFIX}/user/me"
UPLOAD_PATH = f"{API_PREFIX}/asset/upload"

CONFIG_PATH = "config.yaml"
STATE_PATH = "state.yaml"
DATABASE_PATH = "mnemorium.db"
LOG_DIRECTORY = "logs"

# Operator prose: the one home for messages asserted by the suite (TEST-059).
MSG_CONFIG_GENERATED = "the configuration file did not exist; wrote the built-in defaults"
MSG_ENV_OVERRIDES = "the environment overrode these configuration settings"
MSG_PASSWORD_LOGGED = re.compile(
    r"Root admin initialized; use the default password to authenticate and change it: '([^']+)'"
)
MSG_PASSWORD_HIDDEN = "the default password logging is disabled"
MSG_STATE_MISSING = "restore state.yaml"

SECRET_PASSWORD = "C0rrect!Horse"
ROOT_ADMIN_USERNAME = "root"
STARTUP_TIMEOUT_S = 20.0
POLL_INTERVAL_S = 0.1
REQUEST_TIMEOUT_S = 10


class StartupFailed(AssertionError):
    """A server that should have become healthy exited first; carries its exit code and logs."""

    def __init__(self, exit_code: int, output: str) -> None:
        super().__init__(f"server exited with code {exit_code} before it became healthy\n{output}")
        self.exit_code = exit_code
        self.output = output


@dataclass(frozen=True)
class StartupFailure:
    """The observed outcome of a startup the suite expects to refuse."""

    exit_code: int
    output: str


class Sut:
    """One containerised server with its own writable working directory."""

    def __init__(self, image: str, tmp_root: Path) -> None:
        self.image = image
        self.dir = Path(tmp_root)
        self.name = f"{CONTAINER_PREFIX}{uuid.uuid4().hex[:8]}"
        self.base_url = ""
        self._created = False

    # lifecycle

    def start(
        self,
        *,
        env: dict[str, str] | None = None,
        config: dict | None = None,
        config_yaml: str | None = None,
        files: dict[str, str | bytes] | None = None,
    ) -> Sut:
        """Write the pre-start state, run the container and wait until `/health` answers."""
        self._prepare(env=env, config=config, config_yaml=config_yaml, files=files)
        self._run(env)
        if not self.base_url:
            raise StartupFailed(self._exit_code(), self.logs())

        deadline = time.monotonic() + STARTUP_TIMEOUT_S
        while time.monotonic() < deadline:
            if self._status() != "running":
                raise StartupFailed(self._exit_code(), self.logs())
            if self._healthy():
                return self
            time.sleep(POLL_INTERVAL_S)
        raise StartupFailed(self._exit_code(), self.logs())

    def start_expecting_failure(
        self,
        *,
        env: dict[str, str] | None = None,
        config: dict | None = None,
        config_yaml: str | None = None,
        files: dict[str, str | bytes] | None = None,
    ) -> StartupFailure:
        """Run the container, wait for it to exit and return how it failed."""
        self._prepare(env=env, config=config, config_yaml=config_yaml, files=files)
        self._run(env)

        deadline = time.monotonic() + STARTUP_TIMEOUT_S
        while time.monotonic() < deadline:
            status = self._status()
            if status in {"exited", "dead", "removing"}:
                return StartupFailure(self._exit_code(), self.logs())
            time.sleep(POLL_INTERVAL_S)
        pytest.fail(
            f"{self.name} was still {status!r} after {STARTUP_TIMEOUT_S}s; expected the startup to be refused\n"
            f"{self.logs()}"
        )

    def restart(self, *, env: dict[str, str] | None = None, config: dict | None = None) -> Sut:
        """Stop the server and start it again against the same working directory."""
        self.stop()
        return self.start(env=env, config=config)

    def stop(self) -> None:
        """Stop the container, then remove it; idempotent.

        The graceful SIGTERM is what lets the server drain and drop its `WorkerGuard`, so a log file read after `stop()`
        holds every line the server wrote (TEST-058's observation contract, without a sleep).
        """
        if not self._created:
            return
        subprocess.run(
            ["docker", "stop", "-t", "10", self.name],
            capture_output=True,
            text=True,
            check=False,
        )
        subprocess.run(["docker", "rm", "-f", self.name], capture_output=True, text=True, check=False)
        self._created = False
        self.base_url = ""

    # observation

    def logs(self) -> str:
        """The container's stdout and stderr merged; safe while the server runs."""
        result = subprocess.run(["docker", "logs", self.name], capture_output=True, text=True, check=False)
        return result.stdout + result.stderr

    def read(self, relative: str) -> str:
        return (self.dir / relative).read_text(encoding="utf-8")

    def write(self, relative: str, text: str | bytes) -> None:
        path = self.dir / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(text, bytes):
            path.write_bytes(text)
        else:
            path.write_text(text, encoding="utf-8")

    def exists(self, relative: str) -> bool:
        return (self.dir / relative).exists()

    def mode(self, relative: str) -> int:
        """The file mode bits of a path inside the working directory, e.g. `0o600`."""
        return S_IMODE((self.dir / relative).stat().st_mode)

    def listing(self, relative: str) -> list[str]:
        return sorted(os.listdir(self.dir / relative))

    def sha256(self, relative: str) -> str:
        return hashlib.sha256((self.dir / relative).read_bytes()).hexdigest()

    # interaction

    def request(
        self,
        method: str,
        path: str,
        *,
        token: str | None = None,
        json: dict | None = None,
        data: bytes | None = None,
        headers: dict[str, str] | None = None,
    ) -> requests.Response:
        request_headers = dict(headers or {})
        if token is not None:
            request_headers["Authorization"] = f"Bearer {token}"
        return requests.request(
            method,
            f"{self.base_url}{path}",
            headers=request_headers,
            json=json,
            data=data,
            timeout=REQUEST_TIMEOUT_S,
        )

    def login(self, username: str, password: str) -> str:
        response = self.request("POST", LOGIN_PATH, json={"username": username, "password": password})
        assert response.status_code == 200, f"login failed: {response.status_code} {response.text}"
        return response.json()["access_token"]

    def root_password(self) -> str:
        """The one-time Root Admin password the server prints to stdout on first boot."""
        match = MSG_PASSWORD_LOGGED.search(self.logs())
        if match is None:
            raise AssertionError(f"root admin default password not found in the logs of {self.name}")
        return match.group(1)

    def peer_address(self) -> str:
        """The address the server sees a request from: the container's gateway on the host."""
        result = subprocess.run(
            [
                "docker",
                "inspect",
                "-f",
                '{{range .NetworkSettings.Networks}}{{.Gateway}}{{"\\n"}}{{end}}',
                self.name,
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        gateways = [line for line in result.stdout.splitlines() if line.strip()]
        if not gateways:
            raise AssertionError(f"no gateway reported for {self.name}: {result.stderr.strip()}")
        return gateways[0]

    # internals

    def _prepare(
        self,
        *,
        env: dict[str, str] | None,
        config: dict | None,
        config_yaml: str | None,
        files: dict[str, str | bytes] | None,
    ) -> None:
        if config is not None and config_yaml is not None:
            raise ValueError("pass either `config` or `config_yaml`, not both")
        if config is not None:
            config_yaml = yaml.safe_dump(config)
        if config_yaml is not None:
            self.write(CONFIG_PATH, config_yaml)
        for relative, content in (files or {}).items():
            self.write(relative, content)

    def _run(self, env: dict[str, str] | None) -> None:
        args = [
            "docker",
            "run",
            "-d",
            "--name",
            self.name,
            "--user",
            f"{os.getuid()}:{os.getgid()}",
            "-v",
            f"{self.dir}:/app",
            "-p",
            f"127.0.0.1::{CONTAINER_PORT}",
        ]
        for key, value in (env or {}).items():
            args.extend(["-e", f"{key}={value}"])
        args.append(self.image)

        self.base_url = ""
        subprocess.run(args, capture_output=True, text=True, check=True)
        self._created = True

        deadline = time.monotonic() + STARTUP_TIMEOUT_S
        while time.monotonic() < deadline:
            port = self._published_port()
            if port is not None:
                self.base_url = f"http://127.0.0.1:{port}"
                return
            if self._status() != "running":
                return
            time.sleep(POLL_INTERVAL_S)

    def _healthy(self) -> bool:
        try:
            return requests.get(f"{self.base_url}{HEALTH_PATH}", timeout=1).ok
        except requests.ConnectionError:
            return False

    def _status(self) -> str:
        result = subprocess.run(
            ["docker", "inspect", "-f", "{{.State.Status}}", self.name],
            capture_output=True,
            text=True,
            check=False,
        )
        return result.stdout.strip()

    def _exit_code(self) -> int:
        result = subprocess.run(
            ["docker", "inspect", "-f", "{{.State.ExitCode}}", self.name],
            capture_output=True,
            text=True,
            check=False,
        )
        value = result.stdout.strip()
        return int(value) if value.lstrip("-").isdigit() else -1

    def _published_port(self) -> int | None:
        result = subprocess.run(
            ["docker", "port", self.name, f"{CONTAINER_PORT}/tcp"],
            capture_output=True,
            text=True,
            check=False,
        )
        output = result.stdout.strip()
        if not output:
            return None
        return int(output.splitlines()[0].rsplit(":", 1)[1])


@pytest.fixture(scope="session")
def image() -> str:
    """The image under test, resolved once; a missing image stops the run instead of building."""
    image = os.environ.get(IMAGE_ENV, DEFAULT_IMAGE)
    if subprocess.run(["docker", "image", "inspect", image], capture_output=True, check=False).returncode != 0:
        pytest.exit(
            f"image {image!r} not found; build it once, e.g. `docker build -t {DEFAULT_IMAGE} .` or `./script/e2e.sh`",
            returncode=1,
        )
    return image


@pytest.fixture
def sut(image: str, tmp_path: Path) -> Iterator[Sut]:
    """A server that has not started yet, plus the working directory it will run against."""
    server = Sut(image=image, tmp_root=tmp_path)
    try:
        yield server
    finally:
        server.stop()
        shutil.rmtree(server.dir, ignore_errors=True)


def hex64(seed: str) -> str:
    """A 64-character hexadecimal value built by repeating `seed`."""
    if not re.fullmatch(r"[0-9a-f]+", seed):
        raise ValueError(f"{seed!r} is not hexadecimal")
    return (seed * 64)[:64]


def register(
    sut: Sut,
    token: str,
    username: str,
    email: str | None,
    password: str,
    role: str = "STANDARD",
) -> requests.Response:
    """Register a user as the Root Admin; a reused name is tolerated."""
    response = sut.request(
        "POST",
        REGISTER_PATH,
        token=token,
        json={"username": username, "email": email, "password": password, "role": role},
    )
    assert response.status_code in {201, 409}, f"register failed: {response.status_code} {response.text}"
    return response


def begin_upload(
    sut: Sut,
    token: str,
    file_name: str,
    data: bytes,
    content_type: str = "image/png",
) -> int:
    """Open a chunked upload for `data` and return its upload session identifier."""
    response = sut.request(
        "POST",
        UPLOAD_PATH,
        token=token,
        json={
            "content_type": content_type,
            "file_name": file_name,
            "file_size": len(data),
            "integrity_hash": hashlib.sha256(data).hexdigest(),
        },
    )
    assert response.status_code == 201, f"begin failed: {response.status_code} {response.text}"
    return int(response.json()["upload_id"])


def put_chunk(
    sut: Sut,
    token: str,
    upload_id: int,
    chunk_number: int,
    data: bytes,
    total: int,
) -> requests.Response:
    """Send one chunk of an upload session, with its size and digest headers."""
    start = chunk_number * len(data)
    end = start + len(data) - 1
    digest = base64.b64encode(hashlib.sha256(data).digest()).decode()
    return sut.request(
        "PUT",
        f"{UPLOAD_PATH}/{upload_id}/chunk/{chunk_number}",
        token=token,
        data=data,
        headers={
            "Content-Type": "application/octet-stream",
            "Content-Range": f"bytes {start}-{end}/{total}",
            "Content-Digest": f"sha-256=:{digest}:",
        },
    )


def complete_upload(sut: Sut, token: str, upload_id: int) -> requests.Response:
    """Complete an upload session, registering its medium."""
    return sut.request("POST", f"{UPLOAD_PATH}/{upload_id}/complete", token=token)
