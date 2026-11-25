"""Collect the system suite's feature-area modules and share their fixtures and helpers."""

import base64
import hashlib
import hmac
import json
import shutil
import time
from collections.abc import Callable, Iterator
from fnmatch import fnmatch
from pathlib import Path

import pytest
from harness import (
    LOG_DIRECTORY,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    SEED_PROFILE_ENV,
    STAGING_DIRECTORY,
    WRONG_PASSWORD,
    Sut,
    UploadSession,
    hex64,
    link_of,
    payload,
    register,
)
from mnemorium_client.api.asset import (
    get_file,
    get_upload,
    list_files,
    post_upload,
    post_upload_complete,
    put_upload_chunk,
)
from mnemorium_client.api.identity import post_login
from mnemorium_client.api.library import post_gallery, post_gallery_item
from mnemorium_client.models import (
    GalleryItemType,
    LoginRequest,
    PostGalleryItemRequest,
    PostGalleryRequest,
    PostUploadRequest,
)
from mnemorium_client.types import File

HERE = Path(__file__).parent
# The API fixture's media speak for the schemas the fuzzer addresses; the system fixture's own bytes live beside this
# module, under the seed tree the system profile applies.
FIXTURE_DIRECTORY = HERE.parent / "api" / "seed" / "media"
SEED_MEDIA_DIRECTORY = HERE / "seed" / "media"

# The instance secrets are fixed for every feature module that mints a token or proves redaction, so a case can restart
# the same directory deterministically and a leak assertion is a substring search.
JWT_SECRET = hex64("a1")
PEPPER = hex64("b2")
INSTANCE_ENV = {"MNEMORIUM_JWT_SECRET": JWT_SECRET, "MNEMORIUM_PEPPER": PEPPER}
# The worker's shared instance boots the system profile: one browsable catalogue seeded once per worker, so a case
# reads a row it did not create instead of building its preconditions. An own instance stays unseeded unless the case
# names a profile itself.
SHARED_ENV = {**INSTANCE_ENV, SEED_PROFILE_ENV: "system"}
# The account the system seed profile creates, and the fixed password it shares with `SECRET_PASSWORD`.
SEED_ACCOUNT = "coverart"
SEED_PASSWORD = SECRET_PASSWORD
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

# The Root Admin's one-time password, its access token and the seeded identity's access token, cached per instance for
# the worker's life.
_ROOT_PASSWORDS: dict[int, str] = {}
_ROOT_TOKENS: dict[int, str] = {}
_SEED_TOKENS: dict[int, str] = {}


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
    """Attach the failing case's container evidence to the report for the JUnit XML."""
    report = yield
    sections = item.stash.get(EVIDENCE_KEY, None)
    if report.failed and sections is None:
        instance = item.funcargs.get("server") or item.funcargs.get("sut") or item.funcargs.get("_shared_server")
        if instance is not None:
            sections = _evidence(instance)
            item.stash[EVIDENCE_KEY] = sections
    if sections is not None:
        for title, content in sections:
            report.user_properties.append((title, content))
    return report


def pytest_collect_file(file_path: Path, parent: pytest.Collector) -> pytest.Collector | None:
    """Collect each unprefixed Python module of the system suite, exactly once.

    A module the invocation names explicitly (`pytest test/system/upload.py`) is collected by pytest itself, so
    collecting it here as well would run every case twice; a bare directory run leaves the set empty and this hook is
    the only collector of the unprefixed feature modules.
    """
    if file_path.suffix != ".py" or file_path.parent != HERE or file_path.name == "conftest.py":
        return None
    if file_path.resolve() in {Path(argument).resolve() for argument in parent.config.args}:
        return None
    if any(fnmatch(file_path.name, pattern) for pattern in parent.config.getini("python_files")):
        return None
    return pytest.Module.from_parent(parent, path=file_path)


@pytest.fixture(scope="session")
def _shared_server(image: str, tmp_path_factory: pytest.TempPathFactory) -> Iterator[Sut]:
    """The worker's one container, booted on first use and shared by every case that does not mark itself own."""
    server = Sut(image=image, tmp_root=tmp_path_factory.mktemp("shared"))
    server.start(env=SHARED_ENV, config=SHARED_CONFIG)
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
    return _live_token(
        server,
        _ROOT_TOKENS,
        lambda: login(server, ROOT_ADMIN_USERNAME, _ROOT_PASSWORDS.setdefault(id(server), server.root_password())),
    )


@pytest.fixture
def seed_token(server: Sut, request: pytest.FixtureRequest) -> str:
    """A bearer token for the system profile's seeded identity, cached per instance and re-minted after a `401`."""
    if request.node.get_closest_marker("own_instance") is not None:
        pytest.fail("an own-instance test authenticates against its own container, never the shared one")
    return _live_token(server, _SEED_TOKENS, lambda: login(server, SEED_ACCOUNT, SEED_PASSWORD))


def login(server: Sut, username: str, password: str) -> str:
    """Authenticate through the generated client and return the access token."""
    response = post_login.sync_detailed(
        client=server.client(),
        body=LoginRequest(password=password, identifier=username),
    )
    assert response.status_code == 200, f"login failed: {response.status_code} {response.content!r}"
    return json.loads(response.content)["access_token"]


def _live_token(server: Sut, cache: dict[int, str], mint: Callable[[], str]) -> str:
    """A cached token while the instance accepts it, otherwise a freshly minted one stored in `cache`."""
    token = cache.get(id(server))
    if token is not None and server.request("GET", server.href("current_user"), token=token).status_code != 401:
        return token
    token = mint()
    cache[id(server)] = token
    return token


def provision(server: Sut, root_token: str, username: str, role: str = "STANDARD") -> tuple[int, str]:
    """Create an account as the Root Admin and return its identifier and access token."""
    response = register(server, root_token, username, None, SECRET_PASSWORD, role=role)
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    return response.json()["id"], login(server, username, SECRET_PASSWORD)


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
    return post_login.sync_detailed(
        client=server.client(headers=headers),
        body=LoginRequest(password=WRONG_PASSWORD, identifier=username),
    )


def upload(
    server: Sut,
    token: str,
    file_name: str,
    data: bytes,
    content_type: str = "image/png",
) -> dict:
    """Upload one file to completion and return the file representation."""
    client = server.client(token)
    session = begin(server, token, file_name, data, content_type)
    chunk = store_chunk(server, token, session, 0, data)
    assert chunk.status_code == 200, f"chunk failed: {chunk.status_code} {chunk.content!r}"
    completed = post_upload_complete.sync_detailed(session.id, client=client)
    return completed_file(server, token, completed)


def linked_id(href: str) -> int:
    """The numeric identifier at the end of a HAL link."""
    return int(href.rsplit("/", 1)[1])


def create_gallery(server: Sut, token: str, name: str, is_public: bool = False) -> dict:
    """Create a gallery and return its representation."""
    response = post_gallery.sync_detailed(
        client=server.client(token),
        body=PostGalleryRequest(name=name, is_public=is_public),
    )
    assert response.status_code == 201, f"gallery failed: {response.status_code} {response.content!r}"
    return payload(response)


def add_to_gallery(server: Sut, token: str, gallery: dict, media_id: int) -> dict:
    """Add an image to a gallery and return the created item's representation."""
    response = post_gallery_item.sync_detailed(
        gallery["id"],
        client=server.client(token),
        body=PostGalleryItemRequest(media_id=media_id, type_=GalleryItemType.IMAGE),
    )
    assert response.status_code == 201, f"item failed: {response.status_code} {response.content!r}"
    return payload(response)


def file_at(server: Sut, token: str, path: str) -> dict | None:
    """The stored-file collection item at root-relative `path`, or `None` when the caller may discover none.

    The collection is the suite's only way to read a stored record: the path is the one a case wrote or the fixture
    committed, and the item is the single-file representation `get_file` serves.
    """
    response = list_files.sync_detailed(client=server.client(token), path=path)
    assert response.status_code == 200, f"file collection failed: {response.status_code} {response.content!r}"
    items = payload(response)["items"]
    assert len(items) <= 1, f"the exact-path filter returned {len(items)} items for {path!r}"
    return items[0] if items else None


def auth_path(server: Sut, token: str, kind: object, resource_id: int, user_id: int = 0) -> list[str]:
    """The authorization-parent kinds of one resource, from itself up to the instance root."""
    from mnemorium_client.api.identity import get_permission_summary
    from mnemorium_client.models import GetPermissionSummaryAction

    response = get_permission_summary.sync_detailed(
        client=server.client(token),
        user_id=user_id,
        resource_kind=kind,
        resource_id=resource_id,
        action=GetPermissionSummaryAction.VIEW,
    )
    assert response.status_code == 200, f"summary failed: {response.status_code} {response.content!r}"
    return [entry["kind"] for entry in json.loads(response.content)["decision"]["auth_path"]]


def ingest_report(record: dict[str, str]) -> dict[str, int]:
    """The aggregate counters one logged ingest event reported."""
    names = ("registered", "completed", "untouched", "changed", "missing", "removed", "skipped")
    absent = [name for name in names if name not in record]
    assert not absent, f"the ingest report carries no {absent}"
    return {name: int(record[name]) for name in names}


def wait_for_ingest(
    server: Sut,
    predicate: Callable[[dict[str, int]], bool] | None = None,
    description: str = "the ingest reported its outcome",
) -> dict[str, int]:
    """Wait until a logged ingest report satisfies `predicate` and return it.

    The report names no file, so a case waits for the one outcome its own change makes observable and reads the
    counters; the files themselves are polled through `file_at`.
    """
    matches = predicate or (lambda _report: True)

    def reports() -> list[dict[str, int]]:
        return [ingest_report(record) for record in server.events("ingest")]

    server.wait_until(lambda: any(matches(report) for report in reports()), description)
    return next(report for report in reports() if matches(report))


def begin(
    server: Sut,
    token: str,
    file_name: str,
    data: bytes,
    content_type: str = "image/png",
    *,
    file_size: int | None = None,
    integrity_hash: str | None = None,
) -> UploadSession:
    """Open an upload session from a declaration and read it from the representation the begin returns."""
    response = post_upload.sync_detailed(
        client=server.client(token),
        body=PostUploadRequest(
            content_type=content_type,
            file_name=file_name,
            file_size=len(data) if file_size is None else file_size,
            integrity_hash=integrity_hash if integrity_hash is not None else hashlib.sha256(data).hexdigest(),
        ),
    )
    assert response.status_code == 201, f"begin failed: {response.status_code} {response.content!r}"
    return UploadSession.from_body(payload(response))


def session_state(server: Sut, token: str, session: UploadSession) -> dict:
    """Read one upload session, asserting the read succeeds."""
    response = get_upload.sync_detailed(session.id, client=server.client(token))
    assert response.status_code == 200, f"session read failed: {response.status_code} {response.content!r}"
    return payload(response)


def staged_path(server: Sut, session: UploadSession) -> str:
    """The session's staged file, relative to the working directory."""
    return f"{STAGING_DIRECTORY}/{session.id}"


def store_chunk(
    server: Sut,
    token: str,
    session: UploadSession,
    number: int,
    data: bytes,
    *,
    start: int | None = None,
    total: int | None = None,
    digest: bytes | None = None,
    range_header: str | None = None,
    send_digest: bool = True,
):
    """Send one chunk with control over the headers a malformed case needs.

    `total` is the declared content length; it defaults to the chunk's own length, which is the whole file for a session
    whose content fits in one chunk. `send_digest=False` sends an empty `Content-Digest`, which the server refuses
    exactly as it refuses an absent one: the generated client cannot omit a header the document declares.
    """
    offset = number * len(data) if start is None else start
    declared = len(data) if total is None else total
    declared_range = f"bytes {offset}-{offset + len(data) - 1}/{declared}"
    baked = hashlib.sha256(data).digest() if digest is None else digest
    return put_upload_chunk.sync_detailed(
        session.id,
        number,
        client=server.client(token),
        body=File(payload=data),
        content_range=declared_range if range_header is None else range_header,
        content_digest=f"sha-256=:{base64.b64encode(baked).decode()}:" if send_digest else "",
    )


def completed_file(server: Sut, token: str, completed) -> dict:
    """Read the file a completion links, asserting both outcomes succeed."""
    assert completed.status_code == 200, f"completion failed: {completed.status_code} {completed.content!r}"
    response = get_file.sync_detailed(
        linked_id(link_of(payload(completed), "file")),
        client=server.client(token),
    )
    assert response.status_code == 200, f"file read failed: {response.status_code} {response.content!r}"
    return payload(response)
