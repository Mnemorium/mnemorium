"""Harness for the black-box system suite: one container per worker, or one per test when it marks itself own.

Every test drives the image named by `MNEMORIUM_IMAGE` as a black box. The image is never built here: a missing image
is a configuration error and stops the run with a hint, not an implicit build (TEST-058). A case marked
`@pytest.mark.own_instance(reason=...)` gets its own container, its own writable working directory bind-mounted at
`/app`, and its own random loopback port. Every other case drives the one container its worker booted and shares
(`test/system/conftest.py`), so a shared case must never mutate state another case asserts on and never starts, stops
or restarts a container; no test touches the repository working tree.

Operator prose lives in this module only (TEST-057): a test matches a message through the constants below and never
inlines one. The structured `event=`/`component=` fields a lifecycle message carries are matched where the server emits
them.

A case drives a declared operation through the generated client (`Sut.client`), which `script/openapi_client.sh` builds
from the image's own `/doc/openapi.json`: the operation, its parameters and its body are the served document's, so a
case declares no route and no payload shape of its own. Three requests stay hand-built through `request`, and each says
why in the case: a link the scenario names is followed by the href the representation publishes (`link_of`, `follow`),
a request the document cannot declare is sent directly (a `HEAD` on a content route), and `/health` sits outside the
`/api/v1` base the client is bound to. A request the client cannot express is a missing declaration: add it to the
handler's `#[utoipa::path(...)]`/`ToSchema` rather than working around it here.
"""

import hashlib
import json
import os
import re
import shutil
import subprocess
import time
import uuid
from collections.abc import Callable, Iterator
from dataclasses import dataclass
from pathlib import Path
from stat import S_IMODE
from typing import Any
from urllib.parse import quote

import pytest
import requests
import yaml

IMAGE_ENV = "MNEMORIUM_IMAGE"
DEFAULT_IMAGE = "mnemorium:latest"
CONTAINER_PREFIX = os.environ.get("MNEMORIUM_CONTAINER_PREFIX", "mnemorium-config-")
CONTAINER_PORT = 4080
HEALTH_PATH = "/health"
API_PREFIX = "/api/v1"

CONFIG_PATH = "config.yaml"
STATE_PATH = "state.yaml"
DATABASE_PATH = "mnemorium.db"
LOG_DIRECTORY = "logs"
# The staging area an upload session's bytes are written to, relative to the working directory.
STAGING_DIRECTORY = "data/uploads"

# The library root is the operator's mount, relative to the server's working directory. The server never creates it, so
# the harness supplies it for every case: the upload cases need a writable root, and the remaining cases must observe a
# correctly deployed server rather than the absent-root refusal.
MEDIA_DIRECTORY = "media"
# The seed fixtures are data, not part of the image: a seeded instance reads them from the checkout. The tree mount
# supplies the scripts, and the system profile's media is mounted at the directory its `file` rows name, which is the
# writable `media/` root's own `system/` subdirectory — so a case never copies the fixture into its instance.
SEED_PROFILE_ENV = "MNEMORIUM_SEED_PROFILE"
TEST_ROOT = Path(__file__).resolve().parents[1]
SYSTEM_SEED = "system"

# Operator prose: the one home for messages asserted by the suite (TEST-057).
MSG_CONFIG_GENERATED = "the configuration file did not exist; wrote the built-in defaults"
MSG_ENV_OVERRIDES = "the environment overrode these configuration settings"
MSG_PASSWORD_LOGGED = re.compile(
    r"Root admin initialized; use the default password to authenticate and change it: '([^']+)'"
)
MSG_PASSWORD_HIDDEN = "the default password logging is disabled"
MSG_MEDIA_ROOT_ABSENT = f"the library root '{MEDIA_DIRECTORY}' is absent"
MSG_STATE_OVERSIZED = "the instance identity exceeds 4096 bytes"
MSG_STATE_UNSUPPORTED_VERSION = "declares unsupported format version"

SECRET_PASSWORD = "C0rrect!Horse"
WRONG_PASSWORD = "Wr0ng!pass"
ROOT_ADMIN_USERNAME = "root"
STARTUP_TIMEOUT_S = 20.0
POLL_INTERVAL_S = 0.1
REQUEST_TIMEOUT_S = 10
# How long a case waits for the server to reach an outcome it started.
WAIT_TIMEOUT_S = 20.0

# A container in any other state is still on its way: `created` and `restarting` are transient, and an empty read means
# `docker inspect` itself failed, so each is polled rather than read as a terminal outcome (TEST-058).
TERMINAL_STATUSES = frozenset({"exited", "dead", "removing"})

# One structured field of a tracing line: `key="quoted value"` or `key=bare_value`.
FIELD_PATTERN = re.compile(r'([A-Za-z_][A-Za-z0-9_]*)=(?:"((?:[^"\\]|\\.)*)"|(\S+))')


class StartupFailed(AssertionError):
    """A server that should have become healthy did not; carries why, its exit code and its logs."""

    def __init__(self, exit_code: int, output: str, reason: str | None = None) -> None:
        if reason is None:
            reason = f"server exited with code {exit_code} before it became healthy"
        super().__init__(f"{reason}\n{output}")
        self.reason = reason
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
        self._clients: dict[tuple[str | None, tuple[tuple[str, str], ...]], Any] = {}

    # lifecycle

    def start(
        self,
        *,
        env: dict[str, str] | None = None,
        config: dict | None = None,
        config_yaml: str | None = None,
        files: dict[str, str | bytes] | None = None,
        create_media: bool = True,
    ) -> Sut:
        """Write the pre-start state, run the container and wait until `/health` answers."""
        self._prepare(
            env=env,
            config=config,
            config_yaml=config_yaml,
            files=files,
            create_media=create_media,
        )
        reason = self._run(env)
        if reason is not None:
            raise StartupFailed(self._exit_code(), self.logs(), reason)

        deadline = time.monotonic() + STARTUP_TIMEOUT_S
        while time.monotonic() < deadline:
            status = self._status()
            if status in TERMINAL_STATUSES:
                raise StartupFailed(
                    self._exit_code(), self.logs(), f"server entered state {status!r} before it became healthy"
                )
            if self._healthy():
                return self
            time.sleep(POLL_INTERVAL_S)
        raise StartupFailed(
            self._exit_code(), self.logs(), f"server answered no health check within {STARTUP_TIMEOUT_S:g}s"
        )

    def start_expecting_failure(
        self,
        *,
        env: dict[str, str] | None = None,
        config: dict | None = None,
        config_yaml: str | None = None,
        files: dict[str, str | bytes] | None = None,
        create_media: bool = True,
    ) -> StartupFailure:
        """Run the container, wait for it to exit and return how it failed."""
        self._prepare(
            env=env,
            config=config,
            config_yaml=config_yaml,
            files=files,
            create_media=create_media,
        )
        self._run(env)

        deadline = time.monotonic() + STARTUP_TIMEOUT_S
        while time.monotonic() < deadline:
            status = self._status()
            if status in TERMINAL_STATUSES:
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
        holds every line the server wrote (TEST-056's observation contract, without a sleep).
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
        self._clients.clear()

    # observation

    def probe(self) -> bool:
        """One health request; the suite's liveness check between shared tests."""
        return self._healthy()

    def logs(self) -> str:
        """The container's stdout and stderr merged; safe while the server runs."""
        result = subprocess.run(["docker", "logs", self.name], capture_output=True, text=True, check=False)
        return result.stdout + result.stderr

    def log_text(self) -> str:
        """Every log file's contents; complete only after `stop()` has flushed the sink."""
        return "".join(self.read(f"{LOG_DIRECTORY}/{name}") for name in self.listing(LOG_DIRECTORY))

    def events(self, name: str) -> list[dict[str, str]]:
        """Every record the standard-output sink emitted for `event="name"`, parsed into its fields."""
        return _records(self.logs(), name)

    def written_events(self, name: str) -> list[dict[str, str]]:
        """Every record the file sink emitted for `event="name"`; complete only after `stop()`."""
        return _records(self.log_text(), name)

    def wait_for_events(self, name: str) -> list[dict[str, str]]:
        """Wait until the standard-output sink holds an `event="name"` record and return every such record."""
        self.wait_until(lambda: bool(self.events(name)), f"the sink recorded an {name!r} event")
        return self.events(name)

    def wait_until(self, condition: Callable[[], bool], description: str, *, timeout: float = WAIT_TIMEOUT_S) -> None:
        """Poll `condition` up to `timeout`, failing with `description` when it never holds."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if condition():
                return
            time.sleep(POLL_INTERVAL_S)
        raise AssertionError(f"{description} within {timeout:g}s")

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

    def client(self, token: str | None = None, *, headers: dict[str, str] | None = None) -> Any:
        """The generated client for this instance, authenticated when `token` is given.

        One client is cached per token and header set, so a case that logs in once reuses its connection pool.
        `client()` without a token talks to the operations that declare no security, which is how a case proves a
        missing token is refused; an unusable token gets its own cached client the same way. `headers` reach every
        request the client makes, which is how a case sets the operator-configured client-address header.
        """
        key = (token, tuple(sorted((headers or {}).items())))
        if key not in self._clients:
            from mnemorium_client import AuthenticatedClient, Client

            base_url = f"{self.base_url}{API_PREFIX}"
            self._clients[key] = (
                Client(base_url=base_url, timeout=REQUEST_TIMEOUT_S, headers=headers or {})
                if token is None
                else AuthenticatedClient(
                    base_url=base_url,
                    token=token,
                    timeout=REQUEST_TIMEOUT_S,
                    headers=headers or {},
                )
            )
        return self._clients[key]

    def request(
        self,
        method: str,
        path: str,
        *,
        token: str | None = None,
        json: dict | None = None,
        data: bytes | None = None,
        headers: dict[str, str] | None = None,
        allow_redirects: bool = True,
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
            allow_redirects=allow_redirects,
        )

    def index(self) -> dict:
        """Read the public API index; the suite's only `/api/v1` base literal."""
        response = self.request("GET", API_PREFIX)
        assert response.status_code == 200, f"index failed: {response.status_code} {response.text}"
        return response.json()

    def href(self, rel: str, **values: object) -> str:
        """The index's link named `rel`, with its URI-template variables expanded.

        The index is read on every call, so a case can never navigate through a
        stale copy. A relation the index does not publish, or a template variable
        without a value, raises an `AssertionError` naming the relation.
        """
        links = self.index().get("_links", {})
        if rel not in links:
            raise AssertionError(f"the API index declares no {rel!r} link")
        try:
            return expand_template(links[rel]["href"], **values)
        except AssertionError as error:
            raise AssertionError(f"the {rel!r} link cannot be expanded: {error}") from error

    def follow(self, representation: dict, rel: str, *, token: str | None = None) -> requests.Response:
        """GET the named link of `representation`."""
        return self.request("GET", link_of(representation, rel), token=token)

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
        create_media: bool,
    ) -> None:
        if config is not None and config_yaml is not None:
            raise ValueError("pass either `config` or `config_yaml`, not both")
        if config is not None:
            config_yaml = yaml.safe_dump(config)
        if config_yaml is not None:
            self.write(CONFIG_PATH, config_yaml)
        if create_media:
            (self.dir / MEDIA_DIRECTORY).mkdir(exist_ok=True)
        for relative, content in (files or {}).items():
            self.write(relative, content)

    def _run(self, env: dict[str, str] | None) -> str | None:
        """Start the container and wait for its published port; None once it has one, else why the wait ended."""
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
        # A seeded instance reads its fixture from the checkout, so the tree is mounted where the seed resolves it and,
        # for the system profile, the media at the directory its file rows declare. Both are read-only: the instance
        # directory stays the only writable tree, and a case never mutates the fixtures.
        profile = (env or {}).get(SEED_PROFILE_ENV)
        if profile is not None:
            args.extend(["-v", f"{TEST_ROOT}:/app/test:ro"])
        if profile == SYSTEM_SEED:
            seed_media = self.dir / MEDIA_DIRECTORY / SYSTEM_SEED
            seed_media.mkdir(parents=True, exist_ok=True)
            args.extend(["-v", f"{TEST_ROOT / SYSTEM_SEED / 'seed' / 'media'}:/app/{MEDIA_DIRECTORY}/{SYSTEM_SEED}:ro"])
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
                return None
            status = self._status()
            if status in TERMINAL_STATUSES:
                return f"server entered state {status!r} before it published a port"
            time.sleep(POLL_INTERVAL_S)
        return f"server published no port within {STARTUP_TIMEOUT_S:g}s"

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
            f"image {image!r} not found; build it once, e.g. `docker build -t {DEFAULT_IMAGE} .`",
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


def _records(text: str, name: str) -> list[dict[str, str]]:
    """Every record in `text` whose `event` field is `name`."""
    return [record for record in map(parse_record, text.splitlines()) if record.get("event") == name]


def parse_record(line: str) -> dict[str, str]:
    """The structured fields one tracing line carries, with quoted values unescaped.

    The message is free text; every structured field is the `key=value` form the server emits, so a case reads a
    field by name instead of matching the rendered line.
    """
    record = {}
    for match in FIELD_PATTERN.finditer(line):
        key, quoted, bare = match.group(1), match.group(2), match.group(3)
        record[key] = _unquote(quoted) if quoted is not None else bare
    return record


def _unquote(value: str) -> str:
    """Undo the escaping a quoted field value carries."""
    try:
        return json.loads(f'"{value}"')
    except json.JSONDecodeError:
        return value


def hex64(seed: str) -> str:
    """A 64-character hexadecimal value built by repeating `seed`."""
    if not re.fullmatch(r"[0-9a-f]+", seed):
        raise ValueError(f"{seed!r} is not hexadecimal")
    return (seed * 64)[:64]


def expand_template(template: str, **values: object) -> str:
    """Expand the RFC 6570 subset the API publishes.

    Two forms are supported: simple expansion (`{name}`) and query expansion
    (`{?a,b,c}`), both percent-encoded. Every named variable must have a value;
    the collection `search` templates stay unpublished to the suite, which
    appends its filter query to the linked URI instead.
    """

    def expand(match: re.Match[str]) -> str:
        expression = match.group(1)
        query = expression.startswith("?")
        names = expression[1:].split(",") if query else [expression]
        pairs = []
        for name in names:
            if name not in values:
                raise AssertionError(f"the URI template {template!r} has no value for {name!r}")
            if query:
                pairs.append(f"{quote(name, safe='')}={quote(str(values[name]), safe='')}")
            else:
                return quote(str(values[name]), safe="")
        return "?" + "&".join(pairs) if pairs else ""

    return re.sub(r"\{([^}]+)\}", expand, template)


def link_of(representation: dict, rel: str, *, templated: bool = False) -> str:
    """The href of the link named `rel` in `representation`, asserting it is present."""
    links = representation.get("_links")
    assert isinstance(links, dict), "the representation carries no _links object"
    if rel not in links:
        raise AssertionError(f"the representation declares no {rel!r} link")
    link = links[rel]
    href = link.get("href")
    assert isinstance(href, str), f"the {rel!r} link carries no href"
    if bool(link.get("templated", False)) != templated:
        expected = "a URI template" if templated else "a plain link"
        raise AssertionError(f"the {rel!r} link must be {expected}")
    return href


def payload(response: Any) -> dict:
    """The JSON object a response carried, read without the generated models.

    A generated model exposes the fields the document declares and drops the rest, so a case whose subject is the field
    set — a value the representation must omit, a name it must not publish — asserts on this mapping instead. A field
    the document declares keeps the model's own `None`/`UNSET` distinction, which is how an omitted optional value is
    told apart from a cleared one.
    """
    return json.loads(response.content)


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
        sut.href("register"),
        token=token,
        json={"username": username, "email": email, "password": password, "role": role},
    )
    assert response.status_code in {201, 409}, f"register failed: {response.status_code} {response.text}"
    return response


@dataclass(frozen=True)
class UploadSession:
    """An upload session's identifier, its links and the chunk size its `201` representation declared."""

    id: int
    chunk_template: str
    complete_href: str
    self_href: str
    chunk_size: int

    @classmethod
    def from_body(cls, body: dict) -> UploadSession:
        """Read a session from the `201` representation a begin returns.

        The begin reports the session's identifier, its links and the chunk size it fixed; the bitmap, the expiry and
        the chunk count belong to the session's own representation, which a case reads through the `self` link.
        """
        return cls(
            id=int(body["upload_id"]),
            chunk_template=link_of(body, "chunk", templated=True),
            complete_href=link_of(body, "complete"),
            self_href=link_of(body, "self"),
            chunk_size=int(body["chunk_size"]),
        )


def begin_upload(
    sut: Sut,
    token: str,
    file_name: str,
    data: bytes,
    content_type: str = "image/png",
) -> UploadSession:
    """Open a chunked upload for `data` and read the session from its `201` representation."""
    response = sut.request(
        "POST",
        sut.href("upload"),
        token=token,
        json={
            "content_type": content_type,
            "file_name": file_name,
            "file_size": len(data),
            "integrity_hash": hashlib.sha256(data).hexdigest(),
        },
    )
    assert response.status_code == 201, f"begin failed: {response.status_code} {response.text}"
    return UploadSession.from_body(response.json())
