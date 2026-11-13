"""Black-box proof of the `media-ingest` capability.

The library populates itself from the mounted root: the startup scan registers the media it finds, the observer
reconciles a file added, renamed or removed while the server runs, and neither ever writes the content it examines. This
module proves what an operator can see from outside the container: the records the datastore holds, the aggregate
counters the ingest reports to the log sink, the health and index responses, the files under the root and the bytes
they hold.

Every case starts its own container (`TEST-058`) and carries the three Allure labels (`TEST-054`). The startup-window
cases use the `sut` fixture directly, because they choose whether the root exists before the server starts.

A found fixture is registered without the resource row an upload creates, so nothing resolves it through the REST
surface yet; the record the scan stored is therefore asserted in the datastore the working directory holds, one of the
surfaces an operator observes. `test/system/file.py` proves the representation that record backs.

What this suite cannot observe, and where the proof lives instead:

- `REQ-MEDIA-INGEST-003`, scenario `SCN-MEDIA-INGEST-003-C` — the server accepts no request until a required root's
  wait decides. The refusal is proven here by `SCN-MEDIA-INGEST-002-C`; the harness observes a server through its API,
  so it cannot address one that has not bound its listener yet.
- `REQ-MEDIA-INGEST-006`, scenario `SCN-MEDIA-INGEST-006-D` — a probe failure registers neither the file record nor its
  media row. The container shows only that an undecodable file leaves no record; it cannot tell a rolled-back write
  from one that never started. The use case's own test drives the probe port's failure and asserts that neither row is
  committed (`scan_skips_an_image_the_probe_cannot_decode` in `ingest_media.rs`).
- `REQ-MEDIA-INGEST-013`, scenario `SCN-MEDIA-INGEST-013-C` — content modified in place is not registered again. The
  watcher drops a content modification before the ingest is consulted, so the black box can only wait and observe that
  nothing changes; the adapter's test asserts the drop (`ignores_a_content_modification` in
  `media_change_source.rs`) and the use case's test asserts the `changed` verdict writes nothing
  (`scan_reports_a_changed_file_without_writing`).
- `REQ-MEDIA-INGEST-015`, scenario `SCN-MEDIA-INGEST-015-B` — a failed removal retains the record and reports the file
  as missing. The container offers no way to make a committed removal fail, so the use case's test injects the
  repository failure (`a_failed_removal_retains_the_record_and_reports_missing`).
- `REQ-MEDIA-INGEST-016`, scenarios `SCN-MEDIA-INGEST-016-A`, `SCN-MEDIA-INGEST-016-B` and `SCN-MEDIA-INGEST-016-C` —
  the cascade a removed record triggers. The state the scenarios describe cannot be built through the container:
  attaching a scanned medium to an album cover or a gallery item needs the `View` decision on its file's resource, which
  the scan does not create, so no dependent exists to observe vanishing. The behaviour is owned by
  `IngestMedia::remove_registered` and is not proven here.
"""

import os
import re
import shutil
import sqlite3
import time
from datetime import UTC, datetime

import allure
from conftest import FIXTURE_DIRECTORY, add_to_gallery, create_gallery, upload
from harness import MEDIA_DIRECTORY, ROOT_ADMIN_USERNAME, Sut, link_of

IMAGE_CONTENT_TYPE = "image/png"
IMAGE_SOURCE = "cover.png"
VIDEO_SOURCE = "clip.mp4"
AUDIO_SOURCE = "track.flac"

# The prose the container prints when a required root never appears; the suite matches no message inline (`TEST-057`).
ROOT_ABSENT_REFUSAL = "the library root 'media' is absent"

# The log event carrying one ingest operation's aggregate counters.
INGEST_EVENT = 'event="ingest"'

# How long a case waits for the ingest to reach an outcome it starts.
WAIT_SECONDS = 20.0
POLL_SECONDS = 0.1

# A file that is not media content: a PDF the sniffer recognises as one and the ingest refuses.
PDF_BYTES = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n"

# The record fields every case reads, named once so a case can assert which of them moved.
FILE_QUERY = (
    "SELECT file_id AS id, path, user_id, mime_type_id AS mime_type, size_bytes,"
    " modified_at, uploaded_at, origin, is_public FROM file ORDER BY file_id"
)

MEDIA_TABLES = frozenset({"image", "audio", "video"})


def mp3_frame_bytes() -> bytes:
    """Build a short, decodable MPEG-1 Layer III stream: twenty silent frame headers with their payloads."""
    return (bytes([0xFF, 0xFB, 0x90, 0x00]) + bytes(413)) * 20


def wait_until(condition, description: str, *, timeout: float = WAIT_SECONDS) -> None:
    """Poll `condition` up to `timeout`, failing with `description` when it never holds."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if condition():
            return
        time.sleep(POLL_SECONDS)
    raise AssertionError(f"{description} within {timeout:g}s")


def records(server: Sut) -> list[dict]:
    """Every stored file record, read from the datastore the working directory holds."""
    database = server.dir / "mnemorium.db"
    if not database.exists():
        return []
    with sqlite3.connect(database) as connection:
        connection.row_factory = sqlite3.Row
        try:
            return [dict(row) for row in connection.execute(FILE_QUERY)]
        except sqlite3.OperationalError as error:
            # The datastore exists before its migrations ran: it holds no record yet, and nothing else is swallowed.
            if "no such table" not in str(error):
                raise
            return []


def record_at(server: Sut, path: str) -> dict | None:
    """The record stored for `path`, or `None` when the ingest has none."""
    return next((record for record in records(server) if record["path"] == path), None)


def wait_for_record(server: Sut, path: str) -> dict:
    """Wait until the ingest stored a record for `path` and return it."""
    wait_until(lambda: record_at(server, path) is not None, f"the ingest registered {path!r}")
    found = record_at(server, path)
    if found is None:  # pragma: no cover - the wait above guarantees a row
        raise AssertionError(f"the ingest registered no record for {path!r}")
    return found


def media_row(server: Sut, table: str, file_id: int) -> dict | None:
    """The typed row `table` holds for `file_id`, or `None` when it holds none."""
    if table not in MEDIA_TABLES:
        raise ValueError(f"{table!r} is not a typed media table")
    with sqlite3.connect(server.dir / "mnemorium.db") as connection:
        connection.row_factory = sqlite3.Row
        row = connection.execute(f"SELECT * FROM {table} WHERE file_id = ?1", (file_id,)).fetchone()
    return None if row is None else dict(row)


def ingest_reports(server: Sut) -> list[dict[str, int]]:
    """Read the aggregate counters of every ingest report the container has logged."""
    reports = []
    for line in server.logs().splitlines():
        if INGEST_EVENT in line:
            reports.append({name: int(value) for name, value in re.findall(r"(\w+)=(\d+)", line)})
    return reports


def wait_for_report(server: Sut, predicate, description: str) -> dict[str, int]:
    """Wait until a logged ingest report satisfies `predicate` and return it."""
    deadline = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < deadline:
        for report in ingest_reports(server):
            if predicate(report):
                return report
        time.sleep(POLL_SECONDS)
    raise AssertionError(f"{description} within {WAIT_SECONDS:g}s")


def first_report(server: Sut) -> dict[str, int]:
    """The startup scan's report, the first one the container logs."""
    return wait_for_report(server, lambda _report: True, "the startup scan reported its outcome")


def place(sut: Sut, files: dict[str, bytes], *, instant: int | None = None) -> None:
    """Write `files` under the root before the server starts, giving each one the chosen modification instant."""
    for name, data in files.items():
        path = sut.dir / MEDIA_DIRECTORY / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        if instant is not None:
            os.utime(path, (instant, instant))


def preplace(sut: Sut, files: dict[str, bytes], *, instant: int | None = None) -> Sut:
    """Start a server over a root that already holds `files`."""
    place(sut, files, instant=instant)
    return sut.start()


def login(server: Sut) -> str:
    """A bearer token for the Root Admin the instance bootstrapped."""
    return server.login(ROOT_ADMIN_USERNAME, server.root_password())


def file_bytes(server: Sut, path: str) -> bytes:
    """The bytes under the library root at `path`."""
    return (server.dir / MEDIA_DIRECTORY / path).read_bytes()


# --- the root -----------------------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-001")
@allure.story("SCN-MEDIA-INGEST-001-A")
def test_a_promoted_upload_is_addressed_under_the_root(server: Sut, root_token: str) -> None:
    """A file the server stored through upload records a path relative to the root that resolves under it."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()

    file = upload(server, root_token, "promoted.png", data)

    assert not file["path"].startswith("/"), "a stored path must be root-relative"
    assert not file["path"].startswith(f"{MEDIA_DIRECTORY}/"), "a stored path is relative to the root itself"
    assert file_bytes(server, file["path"]) == data


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-001")
@allure.story("SCN-MEDIA-INGEST-001-B")
def test_a_found_file_is_addressed_under_the_same_root(sut: Sut) -> None:
    """A file the scan found records a path relative to the same root that resolves to that file."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {"found.png": data})

    record = wait_for_record(server, "found.png")

    assert not record["path"].startswith("/"), "a found path must be root-relative"
    assert file_bytes(server, record["path"]) == data


# --- the startup window -------------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-002")
@allure.story("SCN-MEDIA-INGEST-002-A")
def test_a_present_root_is_examined_at_startup(sut: Sut) -> None:
    """The root exists at startup, so the scan registers the file under it without waiting."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {"ready.png": data})

    report = first_report(server)

    assert report["registered"] == 1, f"the scan registered nothing: {report}"
    assert record_at(server, "ready.png") is not None


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-002")
@allure.story("SCN-MEDIA-INGEST-002-B")
def test_a_root_that_appears_within_the_window_is_examined(sut: Sut) -> None:
    """A root that appears within the window is scanned, and no record is reconciled as removed for its absence."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    instant = 1_700_000_000
    server = preplace(sut, {"late.png": data}, instant=instant)
    wait_for_record(server, "late.png")
    server.stop()
    shutil.rmtree(server.dir / MEDIA_DIRECTORY)
    server.start(config={"library": {"startup": {"media_root_wait_seconds": 10}}}, create_media=False)
    path = server.dir / MEDIA_DIRECTORY / "late.png"
    path.parent.mkdir(parents=True)
    path.write_bytes(data)
    os.utime(path, (instant, instant))

    report = wait_for_report(server, lambda reported: reported["removed"] == 0, "the waiting scan reported")

    assert report["untouched"] == 1, f"the scan did not examine the restored file: {report}"
    assert record_at(server, "late.png") is not None


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-002")
@allure.story("SCN-MEDIA-INGEST-002-C")
def test_an_absent_required_root_refuses_startup(sut: Sut) -> None:
    """A root that never appears while it is required refuses startup and reconciles no record as removed."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {"kept.png": data})
    wait_for_record(server, "kept.png")
    server.stop()
    shutil.rmtree(server.dir / MEDIA_DIRECTORY)

    failure = server.start_expecting_failure(
        config={"library": {"startup": {"media_root_wait_seconds": 1, "fail_without_media_root": True}}},
        create_media=False,
    )

    assert failure.exit_code != 0, f"startup was not refused: {failure.exit_code}"
    assert ROOT_ABSENT_REFUSAL in failure.output, f"the refusal does not name the root: {failure.output}"
    assert record_at(server, "kept.png") is not None, "a refused startup reconciled a record as removed"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-002")
@allure.story("SCN-MEDIA-INGEST-002-D")
def test_an_absent_optional_root_starts_and_reconciles_empty(sut: Sut) -> None:
    """A root that never appears while it is not required starts the server and reconciles every record as removed."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {"gone.png": data})
    wait_for_record(server, "gone.png")
    server.stop()
    shutil.rmtree(server.dir / MEDIA_DIRECTORY)

    server.start(config={"library": {"startup": {"media_root_wait_seconds": 0}}}, create_media=False)
    report = first_report(server)

    assert report["removed"] == 1, f"the scan did not reconcile the record as removed: {report}"
    assert records(server) == []


# --- readiness ----------------------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-003")
@allure.story("SCN-MEDIA-INGEST-003-A")
def test_a_request_during_the_scan_is_answered(sut: Sut) -> None:
    """A request reaching a server that scans a populated root is answered, and the scan still registers every file."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {f"photo-{index}.png": data for index in range(12)})

    index = server.index()
    wait_until(lambda: len(records(server)) == 12, "the scan registered every file")

    assert index["_links"]["self"]["href"] == "/api/v1"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-003")
@allure.story("SCN-MEDIA-INGEST-003-B")
def test_an_absent_optional_root_does_not_delay_readiness(sut: Sut) -> None:
    """The root is absent and not required, so the server is healthy while the reconciliation runs without blocking."""
    server = sut.start(config={"library": {"startup": {"media_root_wait_seconds": 3}}}, create_media=False)

    responded = server.index()

    assert not server.exists(MEDIA_DIRECTORY), "readiness waited for a root it was not waiting for"
    assert responded["_links"]["self"]["href"] == "/api/v1"
    assert first_report(server)["removed"] == 0


# --- what is accepted ---------------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-004")
@allure.story("SCN-MEDIA-INGEST-004-A")
def test_the_content_decides_the_media_type(sut: Sut) -> None:
    """A file whose name suggests text is registered as the image its content is."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {"photo.txt": data})

    record = wait_for_record(server, "photo.txt")

    assert record["mime_type"] == IMAGE_CONTENT_TYPE
    assert media_row(server, "image", record["id"]) is not None


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-004")
@allure.story("SCN-MEDIA-INGEST-004-B")
def test_content_without_a_media_type_is_skipped(sut: Sut) -> None:
    """A file whose content yields no media type is skipped."""
    server = preplace(sut, {"opaque.bin": bytes(range(64))})

    report = first_report(server)

    assert report["skipped"] == 1, f"the file was not skipped: {report}"
    assert records(server) == []


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-005")
@allure.story("SCN-MEDIA-INGEST-005-A")
def test_a_file_that_is_not_media_is_skipped(sut: Sut) -> None:
    """A file whose content is not an image, a video or audio is skipped and the scan continues."""
    server = preplace(sut, {"document.pdf": PDF_BYTES, "photo.png": (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()})

    report = first_report(server)

    assert record_at(server, "document.pdf") is None
    assert record_at(server, "photo.png") is not None, "the scan stopped at the unsuitable file"
    assert report["skipped"] == 1, f"the unsuitable file was not skipped: {report}"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-005")
@allure.story("SCN-MEDIA-INGEST-005-B")
def test_an_unsupported_media_type_is_skipped(sut: Sut) -> None:
    """An audio file whose media type the server does not support is skipped while a supported one is registered."""
    data = (FIXTURE_DIRECTORY / AUDIO_SOURCE).read_bytes()
    server = preplace(sut, {"track.flac": data, "song.mp3": mp3_frame_bytes()})

    report = first_report(server)

    assert record_at(server, "track.flac") is None, "an unsupported media type was registered"
    assert record_at(server, "song.mp3") is not None, "the scan stopped at the unsupported type"
    assert report["skipped"] == 1, f"the unsupported type was not skipped: {report}"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-005")
@allure.story("SCN-MEDIA-INGEST-005-C")
def test_unsuitable_files_do_not_abort_the_scan(sut: Sut) -> None:
    """Several unsuitable files among suitable ones leave every suitable file registered."""
    image = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    video = (FIXTURE_DIRECTORY / VIDEO_SOURCE).read_bytes()
    server = preplace(
        sut,
        {
            "notes.txt": b"plain notes\n",
            "document.pdf": PDF_BYTES,
            "photo.png": image,
            "clip.mp4": video,
        },
    )

    report = first_report(server)

    assert report["registered"] == 2, f"the scan did not register both suitable files: {report}"
    assert report["skipped"] == 2, f"the scan did not skip both unsuitable files: {report}"


# --- what is recorded ---------------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-006")
@allure.story("SCN-MEDIA-INGEST-006-A")
def test_an_image_is_registered_with_its_media_row(sut: Sut) -> None:
    """An accepted image records its file row and an image row carrying the probed dimensions and orientation."""
    server = preplace(sut, {"photo.png": (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()})

    record = wait_for_record(server, "photo.png")
    image = media_row(server, "image", record["id"])

    assert image is not None, "the image row was not recorded"
    assert (image["width_px"], image["height_px"]) == (64, 64)
    assert image["orientation"] == "SQUARE"
    assert image["name"] == "photo.png"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-006")
@allure.story("SCN-MEDIA-INGEST-006-B")
def test_a_video_is_registered_with_its_media_row(sut: Sut) -> None:
    """An accepted video records its file row and a video row carrying the probed characteristics."""
    server = preplace(sut, {"clip.mp4": (FIXTURE_DIRECTORY / VIDEO_SOURCE).read_bytes()})

    record = wait_for_record(server, "clip.mp4")
    video = media_row(server, "video", record["id"])

    assert video is not None, "the video row was not recorded"
    assert video["duration_ms"] > 0
    assert video["codec"]
    assert video["width"] > 0 and video["height"] > 0
    assert video["scan_type"] in {"PROGRESSIVE", "INTERLACED", "MBAFF", "PAFF"}


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-006")
@allure.story("SCN-MEDIA-INGEST-006-C")
def test_an_audio_file_is_registered_with_absent_measurements(sut: Sut) -> None:
    """An accepted audio file records the reported codec, sample rate and duration, and the block size it omits."""
    server = preplace(sut, {"song.mp3": mp3_frame_bytes()})

    record = wait_for_record(server, "song.mp3")
    audio = media_row(server, "audio", record["id"])

    assert audio is not None, "the audio row was not recorded"
    assert audio["codec"]
    assert audio["sample_rate_hz"] == 44_100
    assert audio["duration_ms"] > 0
    assert audio["block_size"] is None, "a block size the content does not report was fabricated"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-007")
@allure.story("SCN-MEDIA-INGEST-007-A")
def test_a_registered_file_carries_its_size_and_instant(sut: Sut) -> None:
    """A registered file records the size its content has and the instant it was last modified."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    instant = 1_700_000_000
    server = preplace(sut, {"sized.png": data}, instant=instant)

    record = wait_for_record(server, "sized.png")

    assert record["size_bytes"] == len(data)
    assert record["modified_at"].startswith(datetime.fromtimestamp(instant, UTC).strftime("%Y-%m-%d %H:%M:%S"))


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-008")
@allure.story("SCN-MEDIA-INGEST-008-A")
def test_a_found_file_belongs_to_the_root_admin(sut: Sut) -> None:
    """A file the scan found is attributed to the Root Admin and to no other account."""
    server = preplace(sut, {"owned.png": (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()})

    record = wait_for_record(server, "owned.png")

    assert record["user_id"] == 0, "a found file must belong to the Root Admin"


# --- reconciliation -----------------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-009")
@allure.story("SCN-MEDIA-INGEST-009-A")
def test_an_unchanged_file_is_reported_untouched(sut: Sut) -> None:
    """A registered file whose size and instant match its record is reported untouched and nothing is written."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {"stable.png": data})
    before = wait_for_record(server, "stable.png")
    server.stop()

    server.start()
    report = first_report(server)

    assert report["untouched"] == 1, f"the unchanged file was not reported untouched: {report}"
    assert report["registered"] == 0
    assert record_at(server, "stable.png") == before


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-009")
@allure.story("SCN-MEDIA-INGEST-009-B")
def test_a_changed_file_is_reported_and_left_alone(sut: Sut) -> None:
    """A registered file whose size differs from its record is reported changed and its record is left as it was."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {"drifted.png": data})
    before = wait_for_record(server, "drifted.png")
    server.stop()
    with (server.dir / MEDIA_DIRECTORY / "drifted.png").open("ab") as content:
        content.write(b"more bytes")

    server.start()
    report = first_report(server)

    assert report["changed"] == 1, f"the changed file was not reported changed: {report}"
    assert report["registered"] == 0
    assert record_at(server, "drifted.png") == before


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-009")
@allure.story("SCN-MEDIA-INGEST-009-C")
def test_a_vanished_file_is_reconciled_as_removed(sut: Sut) -> None:
    """A registered file the scan no longer finds is reconciled as removed, not as untouched or changed."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    server = preplace(sut, {"vanished.png": data})
    wait_for_record(server, "vanished.png")
    server.stop()
    (server.dir / MEDIA_DIRECTORY / "vanished.png").unlink()

    server.start()
    report = first_report(server)

    assert report["removed"] == 1, f"the vanished file was not reconciled as removed: {report}"
    assert report["untouched"] == 0 and report["changed"] == 0
    assert records(server) == []


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-010")
@allure.story("SCN-MEDIA-INGEST-010-A")
def test_a_missing_media_row_is_completed(sut: Sut) -> None:
    """A registered file without the row its media type requires is probed and the missing row is recorded."""
    server = sut.start()
    token = login(server)
    uploaded = upload(server, token, "uploaded.png", (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes())
    assert media_row(server, "image", uploaded["id"]) is not None
    server.stop()
    with sqlite3.connect(server.dir / "mnemorium.db") as connection:
        connection.execute("DELETE FROM image WHERE file_id = ?1", (uploaded["id"],))
        connection.commit()

    server.start()
    report = wait_for_report(server, lambda reported: reported["completed"] > 0, "the scan completed the row")

    assert report["completed"] == 1
    assert report["untouched"] == 0
    completed = media_row(server, "image", uploaded["id"])
    assert completed is not None, "the missing image row was not recorded"
    assert completed["name"] == uploaded["path"].rsplit("/", 1)[-1]


# --- the read-only boundary ---------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-011")
@allure.story("SCN-MEDIA-INGEST-011-A")
def test_the_ingest_leaves_the_content_byte_identical(sut: Sut) -> None:
    """Every file under the root is byte-identical after the scan, and the scan added, moved and removed none."""
    image = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    video = (FIXTURE_DIRECTORY / VIDEO_SOURCE).read_bytes()
    files = {"photo.png": image, "clip.mp4": video, "unsorted.bin": bytes(range(32))}
    server = preplace(sut, files)
    before = {name: content for name, content in files.items()}

    report = first_report(server)

    assert report["registered"] == 2
    for name, content in before.items():
        assert file_bytes(server, name) == content, f"{name} was not left byte-identical"
    assert sorted(os.listdir(server.dir / MEDIA_DIRECTORY)) == sorted(files)


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-011")
@allure.story("SCN-MEDIA-INGEST-011-B")
def test_an_unreadable_file_does_not_damage_the_ingest(sut: Sut) -> None:
    """A file the ingest cannot read is skipped without being altered, and the remaining files are still registered."""
    image = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    place(sut, {"readable.png": image, "unreadable.png": image})
    unreadable = sut.dir / MEDIA_DIRECTORY / "unreadable.png"
    unreadable.chmod(0o000)
    try:
        server = sut.start()
        report = first_report(server)

        assert record_at(server, "readable.png") is not None, "the unreadable file stopped the scan"
        assert record_at(server, "unreadable.png") is None
        assert unreadable.stat().st_size == len(image), "the unreadable file was altered"
        assert report["skipped"] == 1, f"the unreadable file was not skipped: {report}"
    finally:
        unreadable.chmod(0o600)


# --- removing a reference -----------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-012")
@allure.story("SCN-MEDIA-INGEST-012-A")
@allure.story("SCN-MEDIA-INGEST-012-B")
def test_removing_a_gallery_leaves_the_content_alone(server: Sut, root_token: str) -> None:
    """Removing a gallery item and removing a whole gallery each leave the stored bytes and the file record."""
    removed = upload(server, root_token, "kept.svg", (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes())
    discarded = upload(server, root_token, "kept.png", (FIXTURE_DIRECTORY / "gallery.png").read_bytes())
    gallery = create_gallery(server, root_token, "Kept")
    item = add_to_gallery(server, root_token, gallery, removed["id"])
    second = create_gallery(server, root_token, "Kept Again")
    add_to_gallery(server, root_token, second, discarded["id"])

    removed_item = server.request("DELETE", link_of(item, "self"), token=root_token)
    removed_gallery = server.request("DELETE", link_of(second, "self"), token=root_token)

    assert removed_item.status_code == 204, f"item removal failed: {removed_item.status_code}"
    assert removed_gallery.status_code == 204, f"gallery removal failed: {removed_gallery.status_code}"
    assert file_bytes(server, removed["path"]) == (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes(), (
        "an item removal deleted the stored content"
    )
    assert file_bytes(server, discarded["path"]) == (FIXTURE_DIRECTORY / "gallery.png").read_bytes(), (
        "a gallery removal deleted the stored content"
    )
    assert sorted(os.listdir(server.dir / MEDIA_DIRECTORY)) == sorted([removed["path"], discarded["path"]]), (
        "a gallery removal added, moved or removed content under the root"
    )


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-014")
@allure.story("SCN-MEDIA-INGEST-014-A")
def test_a_renamed_file_keeps_its_record(server: Sut) -> None:
    """A file renamed while the server runs follows its content, keeping the record and the media row it carried."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    (server.dir / MEDIA_DIRECTORY / "before.png").write_bytes(data)
    before = wait_for_record(server, "before.png")
    image = media_row(server, "image", before["id"])
    assert image is not None, "the image row was not recorded"

    (server.dir / MEDIA_DIRECTORY / "before.png").rename(server.dir / MEDIA_DIRECTORY / "after.png")
    after = wait_for_record(server, "after.png")

    assert after["id"] == before["id"], "the rename stored a second record"
    assert after["path"] == "after.png", "the recorded location did not follow the file"
    assert after["size_bytes"] == len(data)
    assert record_at(server, "before.png") is None, "the previous location kept its record"
    assert len(records(server)) == 1, "a second record exists for the same content"
    preserved = media_row(server, "image", after["id"])
    assert preserved is not None, "the rename dropped the media row"
    assert preserved["image_id"] == image["image_id"], "the rename replaced the media row"


# --- the running observer -----------------------------------------------------


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-013")
@allure.story("SCN-MEDIA-INGEST-013-A")
def test_a_file_added_while_the_server_runs_is_registered(server: Sut) -> None:
    """A file added under the root while the server runs is registered without a restart."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    (server.dir / MEDIA_DIRECTORY / "added.png").write_bytes(data)

    record = wait_for_record(server, "added.png")

    assert record["mime_type"] == IMAGE_CONTENT_TYPE
    assert record["origin"] == "SCANNED"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-013")
@allure.story("SCN-MEDIA-INGEST-013-B")
def test_a_file_still_being_written_is_registered_once_complete(server: Sut) -> None:
    """A file whose content is still changing is not registered until it stops, then with the size it finally has."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    path = server.dir / MEDIA_DIRECTORY / "growing.png"
    with path.open("wb") as content:
        for offset in range(0, len(data), 10):
            content.write(data[offset : offset + 10])
            content.flush()
            time.sleep(0.05)
            if offset == 20:
                assert record_at(server, "growing.png") is None, "an unfinished file was registered"

    record = wait_for_record(server, "growing.png")

    assert record["size_bytes"] == len(data), "the record does not carry the size the file finally has"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-015")
@allure.story("SCN-MEDIA-INGEST-015-A")
def test_a_file_removed_while_the_server_runs_is_reconciled(server: Sut) -> None:
    """A registered file removed while the server runs has its record and its media row removed."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    (server.dir / MEDIA_DIRECTORY / "leaving.png").write_bytes(data)
    record = wait_for_record(server, "leaving.png")

    (server.dir / MEDIA_DIRECTORY / "leaving.png").unlink()
    report = wait_for_report(server, lambda reported: reported["removed"] == 1, "the removal was reconciled")

    assert report["removed"] == 1
    assert records(server) == []
    assert media_row(server, "image", record["id"]) is None, "the media row survived its file"


@allure.epic("Media ingest")
@allure.feature("REQ-MEDIA-INGEST-015")
@allure.story("SCN-MEDIA-INGEST-015-C")
def test_a_reappearing_file_is_a_new_record(server: Sut) -> None:
    """A file removed and then added again is a new record, not the restored one."""
    data = (FIXTURE_DIRECTORY / IMAGE_SOURCE).read_bytes()
    (server.dir / MEDIA_DIRECTORY / "cycled.png").write_bytes(data)
    (server.dir / MEDIA_DIRECTORY / "survivor.png").write_bytes(data)
    removed = wait_for_record(server, "cycled.png")
    survivor = wait_for_record(server, "survivor.png")

    (server.dir / MEDIA_DIRECTORY / "cycled.png").unlink()
    wait_until(lambda: record_at(server, "cycled.png") is None, "the removal was reconciled")
    (server.dir / MEDIA_DIRECTORY / "cycled.png").write_bytes(data)
    returned = wait_for_record(server, "cycled.png")

    assert returned["id"] != removed["id"], "the removed record was restored"
    assert record_at(server, "survivor.png")["id"] == survivor["id"]
