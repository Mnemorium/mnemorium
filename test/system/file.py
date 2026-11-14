"""Black-box proof of the `file` capability.

A stored file is the record of one content under the library root: the path relative to that root, the media type, the
size, the instant the content was last modified, the date it was registered, its origin and whether it is public. This
module proves that record for both origins — a file the server promoted from an upload and a file the startup scan
found — that a caller reads it by identifier with every field and no other, and that the representation publishes its
owner, its content route and the typed view it backs, and omits that view when the file backs none.

The declarative contract — the media types, the status of each operation, the representation's fields, its `_links` and
the request schema — is validated by Schemathesis from the served document. These cases prove only what a schema
cannot: which fields the record actually carries, which origin each intake stamps, that an unviewable file is
indistinguishable from a missing one, and that a typed link is present exactly when the row it names exists.

The suite keeps its single `/api/v1` base literal in `harness.Sut.index`; a stored file is reached through a
representation's own `_links.self`. The scan registers a found file without the resource row an upload creates, so no
caller can resolve it by identifier yet; the record the scan stored is therefore read from the datastore the working
directory holds, which is one of the surfaces an operator can observe.
"""

import os
import sqlite3
import time
from datetime import UTC, datetime

import allure
from conftest import FIXTURE_DIRECTORY, provision, upload
from harness import MEDIA_DIRECTORY, Sut, link_of

IMAGE_CONTENT_TYPE = "image/png"
AUDIO_CONTENT_TYPE = "audio/flac"
VIDEO_CONTENT_TYPE = "video/mp4"

# The record's field set: the whole representation, so an added field fails the case that reads it.
FILE_FIELDS = {
    "id",
    "is_public",
    "mime_type",
    "modified_at",
    "origin",
    "path",
    "size_bytes",
    "uploaded_at",
    "_links",
}

# A pre-placed fixture, copied under the root so the scan registers it, and its instant, so the record's modification
# instant is a value the case chose.
FOUND_SOURCE = "cover.png"
FOUND_PATH = "found.png"
FOUND_INSTANT = 1_700_000_000
WAIT_SECONDS = 20.0


def wait_until(condition, description: str) -> None:
    """Poll `condition` up to the suite's startup budget, failing with `description` when it never holds."""
    deadline = time.monotonic() + WAIT_SECONDS
    while time.monotonic() < deadline:
        if condition():
            return
        time.sleep(0.1)
    raise AssertionError(f"{description} within {WAIT_SECONDS:g}s")


def scanned_record(server: Sut, path: str) -> dict:
    """Return the record the scan stored for the file at `path`, read from the working directory's datastore."""
    query = (
        "SELECT file_id AS id, path, mime_type_id AS mime_type, size_bytes, modified_at,"
        " uploaded_at, origin, is_public FROM file WHERE path = ?1"
    )

    def record() -> dict | None:
        with sqlite3.connect(server.dir / "mnemorium.db") as connection:
            connection.row_factory = sqlite3.Row
            row = connection.execute(query, (path,)).fetchone()
        return None if row is None else dict(row)

    wait_until(lambda: record() is not None, f"the scan registered {path!r}")
    found = record()
    if found is None:  # pragma: no cover - the wait above guarantees a row
        raise AssertionError(f"the scan registered no record for {path!r}")
    return found


def file_route(file: dict, file_id: int) -> str:
    """The route of one stored file, derived from a representation's own `_links.self`."""
    return f"{link_of(file, 'self').rsplit('/', 1)[0]}/{file_id}"


def read_file(server: Sut, token: str, file: dict, file_id: int):
    """GET one stored file by identifier, deriving its route from `file`'s self link."""
    return server.request("GET", file_route(file, file_id), token=token)


def found_file(sut: Sut) -> tuple[Sut, dict]:
    """Start a server over a pre-placed fixture and return it and the record the scan gave that fixture."""
    data = (FIXTURE_DIRECTORY / FOUND_SOURCE).read_bytes()
    fixture = sut.dir / MEDIA_DIRECTORY / FOUND_PATH
    fixture.parent.mkdir(parents=True, exist_ok=True)
    fixture.write_bytes(data)
    os.utime(fixture, (FOUND_INSTANT, FOUND_INSTANT))
    server = sut.start()
    return server, scanned_record(server, FOUND_PATH)


# --- the record ---------------------------------------------------------------


@allure.epic("File")
@allure.feature("REQ-FILE-001")
@allure.story("SCN-FILE-001-A")
def test_a_promoted_upload_is_an_uploaded_file(server: Sut, root_token: str) -> None:
    """Content the server received records the `UPLOADED` origin at a root-relative path."""
    data = (FIXTURE_DIRECTORY / "cover.png").read_bytes()

    file = upload(server, root_token, "promoted.png", data, IMAGE_CONTENT_TYPE)

    assert file["origin"] == "UPLOADED"
    assert not file["path"].startswith("/"), "a stored path must be root-relative"
    assert (server.dir / MEDIA_DIRECTORY / file["path"]).read_bytes() == data


@allure.epic("File")
@allure.feature("REQ-FILE-001")
@allure.story("SCN-FILE-001-B")
def test_content_the_scan_found_is_a_scanned_file(sut: Sut) -> None:
    """Content the ingest found under the root records the `SCANNED` origin at a root-relative path."""
    _server, found = found_file(sut)

    assert found["origin"] == "SCANNED"
    assert found["path"] == FOUND_PATH
    assert not found["path"].startswith("/"), "a stored path must be root-relative"
    assert (sut.dir / MEDIA_DIRECTORY / found["path"]).read_bytes() == (FIXTURE_DIRECTORY / FOUND_SOURCE).read_bytes()


@allure.epic("File")
@allure.feature("REQ-FILE-001")
@allure.story("SCN-FILE-001-C")
def test_a_file_records_the_content_it_names(sut: Sut) -> None:
    """The record carries the content's media type, size and modification instant, and its registration date."""
    _server, found = found_file(sut)

    assert found["mime_type"] == IMAGE_CONTENT_TYPE
    assert found["size_bytes"] == (FIXTURE_DIRECTORY / FOUND_SOURCE).stat().st_size
    assert found["modified_at"].startswith(datetime.fromtimestamp(FOUND_INSTANT, UTC).strftime("%Y-%m-%d %H:%M:%S"))
    assert found["uploaded_at"] == datetime.now(UTC).date().isoformat()


# --- the read -----------------------------------------------------------------


@allure.epic("File")
@allure.feature("REQ-FILE-002")
@allure.story("SCN-FILE-002-A")
def test_a_file_of_an_existing_identifier_is_read(server: Sut, root_token: str) -> None:
    """The representation carries exactly the record's fields and the values the upload stored."""
    data = (FIXTURE_DIRECTORY / "gallery.png").read_bytes()
    uploaded = upload(server, root_token, "read.png", data, IMAGE_CONTENT_TYPE)

    response = read_file(server, root_token, uploaded, uploaded["id"])

    assert response.status_code == 200, f"file read failed: {response.status_code} {response.text}"
    file = response.json()
    assert set(file) == FILE_FIELDS
    assert file["id"] == uploaded["id"]
    assert file["path"] == uploaded["path"]
    assert file["mime_type"] == IMAGE_CONTENT_TYPE
    assert file["size_bytes"] == len(data)
    assert file["origin"] == "UPLOADED"
    assert file["is_public"] is False
    assert file["uploaded_at"] == datetime.now(UTC).date().isoformat()
    assert datetime.fromisoformat(file["modified_at"])
    assert link_of(file, "self") == link_of(uploaded, "self")


@allure.epic("File")
@allure.feature("REQ-FILE-002")
@allure.story("SCN-FILE-002-B")
def test_an_unknown_or_unviewable_file_answers_not_found(server: Sut, root_token: str) -> None:
    """A file that does not exist and one the caller may not view both answer `404`."""
    _alice_id, alice = provision(server, root_token, "alice")
    uploaded = upload(server, root_token, "private.png", (FIXTURE_DIRECTORY / "cover.png").read_bytes())

    missing = read_file(server, root_token, uploaded, uploaded["id"] + 1_000)
    unviewable = read_file(server, alice, uploaded, uploaded["id"])

    assert missing.status_code == 404, f"unknown file answered {missing.status_code}"
    assert unviewable.status_code == 404, f"unviewable file answered {unviewable.status_code}"
    assert unviewable.json()["error"] == missing.json()["error"], "the two 404s must be indistinguishable"


@allure.epic("File")
@allure.feature("REQ-FILE-002")
@allure.story("SCN-FILE-002-C")
def test_an_invalid_identifier_answers_bad_request(server: Sut, root_token: str) -> None:
    """An identifier that is not an identifier answers `400`."""
    uploaded = upload(server, root_token, "cover.png", (FIXTURE_DIRECTORY / "cover.png").read_bytes())

    response = server.request(
        "GET", f"{link_of(uploaded, 'self').rsplit('/', 1)[0]}/not-an-identifier", token=root_token
    )

    assert response.status_code == 400, f"invalid identifier answered {response.status_code}"


# --- the links ----------------------------------------------------------------


@allure.epic("File")
@allure.feature("REQ-FILE-003")
@allure.story("SCN-FILE-003-A")
def test_the_representation_links_its_owner_and_content(server: Sut, root_token: str) -> None:
    """`_links.owner` names the owning account and `_links.content` the file's content route."""
    data = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    uploaded = upload(server, root_token, "linked.png", data)

    owner = server.request("GET", link_of(uploaded, "owner"), token=root_token)
    content = server.request("GET", link_of(uploaded, "content"), token=root_token)

    assert owner.status_code == 200, f"owner read failed: {owner.status_code} {owner.text}"
    assert owner.json()["id"] == 0, "an uploaded file is the Root Admin's"
    assert link_of(uploaded, "content") == f"{link_of(uploaded, 'self')}/content"
    assert content.status_code == 200, f"content read failed: {content.status_code}"
    assert content.content == data


@allure.epic("File")
@allure.feature("REQ-FILE-003")
@allure.story("SCN-FILE-003-B")
def test_an_image_backed_file_links_its_image(server: Sut, root_token: str) -> None:
    """A file with an image row carries `_links.image` naming that image."""
    uploaded = upload(server, root_token, "cover.png", (FIXTURE_DIRECTORY / "cover.png").read_bytes())

    response = server.follow(uploaded, "image", token=root_token)

    assert response.status_code == 200, f"image read failed: {response.status_code} {response.text}"
    assert link_of(response.json(), "file") == link_of(uploaded, "self")


@allure.epic("File")
@allure.feature("REQ-FILE-003")
@allure.story("SCN-FILE-003-C")
def test_a_typed_media_backed_file_links_its_typed_view(server: Sut, root_token: str) -> None:
    """A file with an audio row and one with a video row carry `_links.audio` and `_links.video`."""
    audio = upload(
        server,
        root_token,
        "track.flac",
        (FIXTURE_DIRECTORY / "track.flac").read_bytes(),
        AUDIO_CONTENT_TYPE,
    )
    video = upload(server, root_token, "clip.mp4", (FIXTURE_DIRECTORY / "clip.mp4").read_bytes(), VIDEO_CONTENT_TYPE)

    typed_audio = server.follow(audio, "audio", token=root_token)
    typed_video = server.follow(video, "video", token=root_token)

    assert typed_audio.status_code == 200, f"audio read failed: {typed_audio.status_code}"
    assert typed_video.status_code == 200, f"video read failed: {typed_video.status_code}"
    assert link_of(typed_audio.json(), "file") == link_of(audio, "self")
    assert link_of(typed_video.json(), "file") == link_of(video, "self")


@allure.epic("File")
@allure.feature("REQ-FILE-003")
@allure.story("SCN-FILE-003-D")
def test_a_file_with_no_media_row_carries_no_typed_link(server: Sut, root_token: str) -> None:
    """A file that backs no typed asset carries none of `image`, `audio` and `video`."""
    uploaded = upload(server, root_token, "bare.png", (FIXTURE_DIRECTORY / "cover.png").read_bytes())
    with sqlite3.connect(server.dir / "mnemorium.db") as connection:
        connection.execute("DELETE FROM image WHERE file_id = ?1", (uploaded["id"],))
        connection.commit()

    response = read_file(server, root_token, uploaded, uploaded["id"])

    assert response.status_code == 200, f"file read failed: {response.status_code} {response.text}"
    links = response.json()["_links"]
    assert set(links).isdisjoint({"image", "audio", "video"}), "a bare file carried a typed link"
    assert set(links) == {"content", "owner", "self"}
