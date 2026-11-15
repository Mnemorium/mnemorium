"""Collect the system suite's feature-area modules and share their fixtures and helpers.

The suite names one module per feature area — `authentication.py` and `upload.py` among them — but the three global
`python_files` patterns (`test_*.py`, `*_test.py`, `*_*.py`) match none of those single-word names. This hook collects
every module directly in this folder that those patterns leave out, so a feature area needs no prefix and no entry in
`pyproject.toml`.

A case that does not mark itself own drives the one instance its worker booted and shares, so the session fixture, the
shared configuration, the health probe and the cached Root Admin token live here rather than in each module. A case
marked `@pytest.mark.own_instance(reason=...)` drives its own container through the `sut` fixture instead. The
seed-media directory and the request helpers that more than one operation needs are shared here the same way; every
route those helpers address comes from the API index or from a representation's links, never from a path built in this
suite.

`/api/v1` is the system suite's only base literal, and it lives in `harness.Sut.index`.
"""

import base64
import hashlib
import hmac
import json
import os
import shutil
import tempfile
import time
from collections.abc import Iterator
from fnmatch import fnmatch
from pathlib import Path

import allure
import pytest
from harness import (
    CONTAINER_LEDGER_ENV,
    LOG_DIRECTORY,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    WRONG_PASSWORD,
    Sut,
    begin_upload,
    complete_upload,
    hex64,
    link_of,
    put_chunk,
    register,
)

HERE = Path(__file__).parent
FIXTURE_DIRECTORY = HERE.parent / "seed" / "media"

# The instance secrets are fixed for every feature module that mints a token or proves redaction, so a case can restart
# the same directory deterministically and a leak assertion is a substring search.
JWT_SECRET = hex64("a1")
PEPPER = hex64("b2")
INSTANCE_ENV = {"MNEMORIUM_JWT_SECRET": JWT_SECRET, "MNEMORIUM_PEPPER": PEPPER}
# The closed set of reasons an own instance may cite; `pytest_collection_modifyitems` rejects anything else.
REASONS = frozenset({"first_boot", "restart", "global_filesystem", "log_sink", "exact_collection", "process_state"})

# A failing case's evidence is truncated at this many characters per source, so one runaway log cannot drown the report.
EVIDENCE_LIMIT = 20_000

# The evidence a failing case collected, kept on the item so every later report phase carries it: `junitxml` writes a
# test case's properties from its teardown report.
EVIDENCE_KEY = pytest.StashKey[list[tuple[str, str]]]()

# The shared instance outlives every case of a worker, so its login limiter is raised out of the way: the suite's own
# volume must never throttle it, and `authentication.py` keeps the default limiter honest on an own instance.
SHARED_CONFIG = {
    "security": {
        "rate_limit": {
            "burst_size": 1_000_000,
            "client_ip_header": "x-forwarded-for",
            "period_seconds": 1,
            "trusted_proxies": "",
        }
    }
}

# The Root Admin's one-time password and its access token, cached per instance for the worker's life.
_ROOT_PASSWORDS: dict[int, str] = {}
_ROOT_TOKENS: dict[int, str] = {}


def pytest_configure(config: pytest.Config) -> None:
    """Start the suite's clock and, on the controller, open the ledger every worker writes to."""
    config._system_suite_started = time.monotonic()
    if hasattr(config, "workerinput"):
        return
    ledger = Path(tempfile.mkdtemp(prefix="mnemorium-containers-"))
    os.environ[CONTAINER_LEDGER_ENV] = str(ledger)
    config._container_ledger = ledger


def pytest_terminal_summary(terminalreporter: pytest.TerminalReporter, config: pytest.Config) -> None:
    """Report the suite's wall clock and the containers it started, one session per worker included."""
    started = getattr(config, "_system_suite_started", None)
    ledger = getattr(config, "_container_ledger", None)
    if started is None or ledger is None:
        return
    kinds = [
        line.split("\t", 1)[0]
        for path in Path(ledger).glob("*.tsv")
        for line in path.read_text(encoding="utf-8").splitlines()
        if line
    ]
    shared = kinds.count("shared")
    terminalreporter.write_sep("-", "system suite")
    terminalreporter.write_line(f"containers: {len(kinds)} (shared {shared}, own {len(kinds) - shared})")
    terminalreporter.write_line(f"wall clock: {time.monotonic() - started:.1f}s")
    shutil.rmtree(ledger, ignore_errors=True)


def pytest_collection_modifyitems(items: list[pytest.Item]) -> None:
    """Keep the own-instance marker and the `sut` fixture in agreement, and the reason inside the closed set."""
    problems = []
    for item in items:
        marker = item.get_closest_marker("own_instance")
        own = "sut" in item.fixturenames
        if marker and not own:
            problems.append(f"{item.nodeid}: marked own_instance but requests no own container")
        if own and not marker:
            problems.append(f"{item.nodeid}: starts its own container without own_instance(reason=...)")
        if marker and marker.kwargs.get("reason") not in REASONS:
            problems.append(f"{item.nodeid}: reason {marker.kwargs.get('reason')!r} is outside the set")
    if problems:
        raise pytest.UsageError("\n".join(problems))


def _evidence(server: Sut) -> list[tuple[str, str]]:
    """The container's logs, its `/app` listing and each log file, truncated to the evidence limit."""
    sections = []
    try:
        sections.append(("container logs", server.logs()))
        sections.append(("/app", "\n".join(server.listing("."))))
        for name in server.listing(LOG_DIRECTORY) if server.exists(LOG_DIRECTORY) else []:
            sections.append((f"logs/{name}", server.read(f"{LOG_DIRECTORY}/{name}")))
    except OSError:
        pass
    return [(title, content[:EVIDENCE_LIMIT]) for title, content in sections]


@pytest.hookimpl(wrapper=True)
def pytest_runtest_makereport(item: pytest.Item, call: pytest.CallInfo):
    """Attach the failing case's container evidence to the report for the JUnit XML and the Allure result."""
    report = yield
    sections = item.stash.get(EVIDENCE_KEY, None)
    if report.failed and sections is None:
        instance = item.funcargs.get("server") or item.funcargs.get("sut") or item.funcargs.get("_shared_server")
        if instance is not None:
            sections = _evidence(instance)
            item.stash[EVIDENCE_KEY] = sections
            for title, content in sections:
                allure.attach(content, name=title, attachment_type=allure.attachment_type.TEXT)
    if sections is not None:
        for title, content in sections:
            report.user_properties.append((title, content))
    return report


def pytest_collect_file(file_path: Path, parent: pytest.Collector) -> pytest.Collector | None:
    """Collect each unprefixed Python module of the system suite."""
    if file_path.suffix != ".py" or file_path.parent != HERE or file_path.name == "conftest.py":
        return None
    if any(fnmatch(file_path.name, pattern) for pattern in parent.config.getini("python_files")):
        return None
    return pytest.Module.from_parent(parent, path=file_path)


@pytest.fixture(scope="session")
def _shared_server(image: str, tmp_path_factory: pytest.TempPathFactory) -> Iterator[Sut]:
    """The worker's one container, booted on first use and shared by every case that does not mark itself own."""
    server = Sut(image=image, tmp_root=tmp_path_factory.mktemp("shared"), shared=True)
    server.start(env=INSTANCE_ENV, config=SHARED_CONFIG)
    try:
        yield server
    finally:
        server.stop()
        shutil.rmtree(server.dir, ignore_errors=True)


@pytest.fixture
def server(_shared_server: Sut, request: pytest.FixtureRequest) -> Sut:
    """The worker's shared instance, probed once before the case; an unhealthy one stops the session."""
    if not _shared_server.probe():
        try:
            pytest.fail(f"the shared instance is not healthy\n{_shared_server.logs()}")
        finally:
            request.session.shouldstop = "the worker's shared instance is not healthy"
    return _shared_server


@pytest.fixture
def root_token(server: Sut, request: pytest.FixtureRequest) -> str:
    """A bearer token for the Root Admin, cached per instance and re-minted after a `401`."""
    if request.node.get_closest_marker("own_instance") is not None:
        pytest.fail("an own-instance test authenticates against its own container, never the shared one")
    token = _ROOT_TOKENS.get(id(server))
    if token is not None and server.request("GET", server.href("current_user"), token=token).status_code != 401:
        return token
    password = _ROOT_PASSWORDS.setdefault(id(server), server.root_password())
    token = server.login(ROOT_ADMIN_USERNAME, password)
    _ROOT_TOKENS[id(server)] = token
    return token


def provision(server: Sut, root_token: str, username: str, role: str = "STANDARD") -> tuple[int, str]:
    """Create an account as the Root Admin and return its identifier and access token."""
    response = register(server, root_token, username, None, SECRET_PASSWORD, role=role)
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    return response.json()["id"], server.login(username, SECRET_PASSWORD)


def start_instance(sut: Sut, *, config: dict | None = None, env: dict[str, str] | None = None) -> Sut:
    """Start a container whose signing secret and site pepper are fixed for every case."""
    return sut.start(env={**INSTANCE_ENV, **(env or {})}, config=config)


def limiter_config(*, trusted: str | None = None) -> dict:
    """A login limiter with one attempt per address and a long refill period."""
    rate_limit = {
        "burst_size": 1,
        "client_ip_header": "x-real-ip",
        "period_seconds": 60,
    }
    if trusted is not None:
        rate_limit["trusted_proxies"] = trusted
    return {"security": {"rate_limit": rate_limit}}


def _base64url(value: bytes) -> str:
    """Base64url-encode `value` without padding, as a JWT segment."""
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


def mint_token(secret: str, subject: str, *, expires_in: int = 3600, tampered: bool = False) -> str:
    """Mint one HS256 token, optionally with a corrupted signature or a past expiry."""
    header = _base64url(json.dumps({"alg": "HS256", "typ": "JWT"}, separators=(",", ":")).encode())
    payload = _base64url(json.dumps({"exp": int(time.time()) + expires_in, "sub": subject}).encode())
    signature = hmac.new(secret.encode(), f"{header}.{payload}".encode(), hashlib.sha256).digest()
    if tampered:
        signature = bytes([signature[0] ^ 0xFF, *signature[1:]])
    return f"{header}.{payload}.{_base64url(signature)}"


def attempt(server: Sut, username: str, address: str | None = None):
    """A login attempt the limiter counts; the credentials cannot succeed."""
    headers = {"x-real-ip": address} if address is not None else None
    return server.attempt_login(username, WRONG_PASSWORD, headers=headers)


def upload(
    server: Sut,
    token: str,
    file_name: str,
    data: bytes,
    content_type: str = "image/png",
) -> dict:
    """Upload one file to completion and return the file representation."""
    session = begin_upload(server, token, file_name, data, content_type)
    chunk = put_chunk(server, token, session, 0, data, len(data))
    assert chunk.status_code == 200, f"chunk failed: {chunk.status_code} {chunk.text}"
    completed = complete_upload(server, token, session)
    assert completed.status_code == 200, f"completion failed: {completed.status_code} {completed.text}"
    response = server.request("GET", link_of(completed.json(), "file"), token=token)
    assert response.status_code == 200, f"file read failed: {response.status_code} {response.text}"
    return response.json()


def linked_id(href: str) -> int:
    """The numeric identifier at the end of a HAL link."""
    return int(href.rsplit("/", 1)[1])


def create_gallery(server: Sut, token: str, name: str, is_public: bool = False) -> dict:
    """Create a gallery and return its representation."""
    response = server.request(
        "POST",
        server.href("gallery"),
        token=token,
        json={"name": name, "is_public": is_public},
    )
    assert response.status_code == 201, f"gallery failed: {response.status_code} {response.text}"
    return response.json()


def add_to_gallery(server: Sut, token: str, gallery: dict, media_id: int) -> dict:
    """Add an image to a gallery and return the created item's representation."""
    response = server.request(
        "POST",
        link_of(gallery, "items"),
        token=token,
        json={"type": "image", "media_id": media_id},
    )
    assert response.status_code == 201, f"item failed: {response.status_code} {response.text}"
    return response.json()
