"""Black-box proof of the `upload` capability.

An upload is a resumable chunked session: it begins from a declared file, receives chunks at fixed offsets, verifies a
whole-file integrity hash, promotes the staged content under the library root and registers the typed asset the declared
content type requires. This module proves the lifecycle an operator observes — the session and its staged content, the
chunk protocol's effects, expiry, cancellation, reaping, promotion and attribution — and nothing the served document
already declares. Every declared status, media type, response header, envelope shape and `_links` field belongs to the
API fuzz test (`script/api_fuzz.sh`); a case here asserts only the effect a schema cannot observe, except where the
requirement itself makes the outcome observable only as a status (an expired, finished or absent-root refusal).

The completion group uploads the real `test/seed/media` fixtures, because the container's `ffprobe` is what turns the
bytes into an image, a video or an audio row. The `audiovisual.mp4` fixture carries both an H.264 video track and an AAC
audio track, so a declared audio type can prove the declaration is authoritative over the probe.

Every route comes from the API index or a representation's links, so the suite keeps its single `/api/v1` base literal
in `harness.Sut.index`.
"""

import base64
import hashlib
import shutil
import time

import allure
from audio import ONE_SECOND, wav
from conftest import FIXTURE_DIRECTORY, linked_id, provision, upload
from harness import (
    ROOT_ADMIN_USERNAME,
    Sut,
    UploadSession,
    begin_upload,
    complete_upload,
    expand_template,
    link_of,
    put_chunk,
)

IMAGE = FIXTURE_DIRECTORY / "cover.png"
VIDEO = FIXTURE_DIRECTORY / "clip.mp4"
AUDIO = FIXTURE_DIRECTORY / "track.flac"
AUDIOVISUAL = FIXTURE_DIRECTORY / "audiovisual.mp4"

IMAGE_CONTENT_TYPE = "image/png"
VIDEO_CONTENT_TYPE = "video/mp4"
AUDIO_CONTENT_TYPE = "audio/flac"
AUDIO_MP4_CONTENT_TYPE = "audio/mp4"

STAGING_DIRECTORY = "data/uploads"
MEDIA_DIRECTORY = "media"

VIDEO_CODEC = "h264"
VIDEO_DURATION_MS = 200.0
VIDEO_DIMENSIONS = (16, 16)
AUDIO_CODEC = "flac"
AUDIO_SAMPLE_RATE_HZ = 44_100

# A WAV the probe reports without a bit depth or a block size, so the completion
# must record both as absent rather than fabricate one.
WIDE_WAV = wav(64, ONE_SECOND)


def _begin(
    server: Sut,
    token: str,
    *,
    content_type: str = IMAGE_CONTENT_TYPE,
    file_name: str = "cover.png",
    data: bytes = IMAGE.read_bytes(),
    file_size: int | None = None,
    integrity_hash: str | None = None,
):
    """Post a begin request without requiring it to succeed."""
    return server.request(
        "POST",
        server.href("upload"),
        token=token,
        json={
            "content_type": content_type,
            "file_name": file_name,
            "file_size": len(data) if file_size is None else file_size,
            "integrity_hash": integrity_hash if integrity_hash is not None else hashlib.sha256(data).hexdigest(),
        },
    )


def _open(
    server: Sut,
    token: str,
    *,
    content_type: str = IMAGE_CONTENT_TYPE,
    file_name: str = "cover.png",
    data: bytes = IMAGE.read_bytes(),
    integrity_hash: str | None = None,
) -> tuple[UploadSession, int]:
    """Begin a session and return it with the chunk size the begin reported."""
    response = _begin(
        server,
        token,
        content_type=content_type,
        file_name=file_name,
        data=data,
        integrity_hash=integrity_hash,
    )
    assert response.status_code == 201, f"begin failed: {response.status_code} {response.text}"
    body = response.json()
    session = UploadSession(
        id=int(body["upload_id"]),
        chunk_template=link_of(body, "chunk", templated=True),
        complete_href=link_of(body, "complete"),
        self_href=link_of(body, "self"),
    )
    return session, int(body["chunk_size"])


def _session(server: Sut, token: str, session: UploadSession):
    """Read one upload session."""
    return server.request("GET", session.self_href, token=token)


def _state(server: Sut, token: str, session: UploadSession) -> dict:
    """Read one upload session, asserting the read succeeds."""
    response = _session(server, token, session)
    assert response.status_code == 200, f"session read failed: {response.status_code} {response.text}"
    return response.json()


def _cancel(server: Sut, token: str, session: UploadSession):
    """Cancel one upload session."""
    return server.request("DELETE", session.self_href, token=token)


def _staged(server: Sut, session: UploadSession) -> str:
    """The staged file of one session, relative to the working directory."""
    return f"{STAGING_DIRECTORY}/{session.id}"


def _staged_exists(server: Sut, session: UploadSession) -> bool:
    """Whether the session's staged content exists in the working directory."""
    return server.exists(_staged(server, session))


def _staged_digest(server: Sut, session: UploadSession) -> str:
    """The digest of the session's staged content."""
    return server.sha256(_staged(server, session))


def _staged_bytes(server: Sut, session: UploadSession) -> bytes:
    """The session's staged content."""
    return (server.dir / _staged(server, session)).read_bytes()


def _chunk(
    server: Sut,
    token: str,
    session: UploadSession,
    number: int,
    data: bytes,
    *,
    start: int | None = None,
    digest: bytes | None = None,
    range_header: str | None = None,
    send_digest: bool = True,
):
    """Send one chunk with control over the headers a malformed case needs."""
    offset = number * len(data) if start is None else start
    headers = {
        "Content-Type": "application/octet-stream",
        "Content-Range": f"bytes {offset}-{offset + len(data) - 1}/{session_total(server, token, session)}"
        if range_header is None
        else range_header,
    }
    if send_digest:
        baked = hashlib.sha256(data).digest() if digest is None else digest
        headers["Content-Digest"] = f"sha-256=:{base64.b64encode(baked).decode()}:"
    return server.request(
        "PUT",
        expand_template(session.chunk_template, chunk_number=number),
        token=token,
        data=data,
        headers=headers,
    )


def session_total(server: Sut, token: str, session: UploadSession) -> int:
    """The session's total chunk count, read from its representation."""
    return int(_state(server, token, session)["total_chunks"])


def _complete(server: Sut, token: str, session: UploadSession):
    """Complete one upload session."""
    return complete_upload(server, token, session)


def _upload_one(server: Sut, token: str, session: UploadSession, data: bytes):
    """Store `data` as a session's only chunk."""
    chunk = put_chunk(server, token, session, 0, data, len(data))
    assert chunk.status_code == 200, f"chunk failed: {chunk.status_code} {chunk.text}"
    return chunk


def _completed_file(server: Sut, token: str, completed) -> dict:
    """Read the file a completion links."""
    assert completed.status_code == 200, f"completion failed: {completed.status_code} {completed.text}"
    response = server.request("GET", link_of(completed.json(), "file"), token=token)
    assert response.status_code == 200, f"file read failed: {response.status_code} {response.text}"
    return response.json()


def _file_id(completed) -> int:
    """The identifier of the file a completion links."""
    return linked_id(link_of(completed.json(), "file"))


def _media_files(server: Sut) -> list[str]:
    """The library root's entries."""
    return server.listing(MEDIA_DIRECTORY)


# REQ-UPLOAD-001: a session begins from a declared file.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-001")
@allure.story("SCN-UPLOAD-001-A")
def test_a_valid_request_creates_a_session(server: Sut, root_token: str) -> None:
    """A valid declaration creates a readable session and its staged content."""
    data = IMAGE.read_bytes()

    session = begin_upload(server, root_token, "cover.png", data)

    assert _staged_exists(server, session), "a valid begin must create the staged content"
    assert (server.dir / _staged(server, session)).stat().st_size == len(data)
    assert _state(server, root_token, session)["bitmap"] == "0"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-001")
@allure.story("SCN-UPLOAD-001-B")
def test_an_unsafe_file_name_creates_nothing(server: Sut, root_token: str) -> None:
    """A name that is empty, dot-leading or carries a separator is refused with no effect."""
    for name in ("", ".hidden.png", "../escape.png", "sub/cover.png"):
        response = _begin(server, root_token, file_name=name)

        assert response.status_code != 201, f"unsafe name {name!r} was accepted"
        assert not response.headers.get("Location")
        assert server.listing(STAGING_DIRECTORY) == []


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-001")
@allure.story("SCN-UPLOAD-001-C")
def test_an_unsupported_content_type_creates_nothing(server: Sut, root_token: str) -> None:
    """A declared type outside the supported set creates no session."""
    response = _begin(server, root_token, content_type="application/x-not-a-media-type")

    assert response.status_code != 201, "an unsupported content type was accepted"
    assert server.listing(STAGING_DIRECTORY) == []


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-001")
@allure.story("SCN-UPLOAD-001-D")
def test_a_zero_file_size_creates_nothing(server: Sut, root_token: str) -> None:
    """A declared size of zero creates no session."""
    response = _begin(server, root_token, file_size=0, integrity_hash=hashlib.sha256(b"").hexdigest())

    assert response.status_code != 201, "a zero-sized upload was accepted"
    assert server.listing(STAGING_DIRECTORY) == []


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-001")
@allure.story("SCN-UPLOAD-001-F")
def test_a_malformed_integrity_hash_creates_nothing(server: Sut, root_token: str) -> None:
    """A hash that is not a 64-character lowercase digest creates no session."""
    for malformed in ("", "not-a-digest", "A" * 64, "a" * 63):
        response = _begin(server, root_token, integrity_hash=malformed)

        assert response.status_code != 201, f"malformed hash {malformed!r} was accepted"
        assert server.listing(STAGING_DIRECTORY) == []


# REQ-UPLOAD-002: a session and its staged content begin together.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-002")
@allure.story("SCN-UPLOAD-002-A")
def test_a_failed_begin_leaves_nothing(server: Sut, root_token: str) -> None:
    """A refused begin persists no session and leaves no staged content."""
    response = _begin(server, root_token, content_type="application/x-not-a-media-type")
    assert response.status_code != 201

    # The next session takes the first identifier a committed row would have
    # taken, so no earlier session exists and no staged file survived the failure.
    session = begin_upload(server, root_token, "cover.png", IMAGE.read_bytes())

    assert session.id == 1
    assert server.listing(STAGING_DIRECTORY) == [str(session.id)]


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-002")
@allure.story("SCN-UPLOAD-002-B")
def test_a_leftover_staging_file_is_reclaimed(server: Sut, root_token: str) -> None:
    """A staged file planted for the next identifier is reclaimed by the begin."""
    server.write(f"{STAGING_DIRECTORY}/1", b"stale")
    data = IMAGE.read_bytes()

    session = begin_upload(server, root_token, "cover.png", data)

    assert session.id == 1
    assert _staged_bytes(server, session) == b"\x00" * len(data), "the stale content was not reclaimed"


# REQ-UPLOAD-006: the session state is readable.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-006")
@allure.story("SCN-UPLOAD-006-A")
def test_a_known_session_is_read(server: Sut, root_token: str) -> None:
    """A known session reports its bitmap, count, expiry, finish flag, file and links."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, session, data)

    body = _state(server, root_token, session)

    assert body["bitmap"] == "1"
    assert body["total_chunks"] == 1
    assert body["is_finished"] is False
    assert body["file_id"] is None
    assert body["expires_at"]
    assert link_of(body, "chunk", templated=True) == session.chunk_template
    assert link_of(body, "complete") == session.complete_href
    assert link_of(body, "self") == session.self_href


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-006")
@allure.story("SCN-UPLOAD-006-B")
def test_an_unknown_session_is_not_found(server: Sut, root_token: str) -> None:
    """A session that no longer exists answers `404` and discloses nothing about its owner."""
    session = begin_upload(server, root_token, "cover.png", IMAGE.read_bytes())
    assert _cancel(server, root_token, session).status_code == 204

    response = _session(server, root_token, session)

    assert response.status_code == 404
    assert "cover.png" not in response.text


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-006")
@allure.story("SCN-UPLOAD-006-C")
def test_a_finished_session_reports_a_complete_bitmap(server: Sut, root_token: str) -> None:
    """A finished session reports every chunk as received and the caller's file."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, session, data)
    completed = _complete(server, root_token, session)

    body = _state(server, root_token, session)

    assert body["bitmap"] == "1" * body["total_chunks"]
    assert body["is_finished"] is True
    assert body["file_id"] == _file_id(completed)


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-006")
@allure.story("SCN-UPLOAD-006-D")
def test_no_collection_of_sessions_exists(server: Sut, root_token: str) -> None:
    """The upload entry point is an operation, not a collection that lists sessions."""
    begin_upload(server, root_token, "cover.png", IMAGE.read_bytes())

    response = server.request("GET", server.href("upload"), token=root_token)

    assert response.status_code != 200, "the upload collection must not be readable"
    assert "upload_id" not in response.text


# REQ-UPLOAD-013: a session can be cancelled.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-013")
@allure.story("SCN-UPLOAD-013-A")
def test_a_pending_session_is_cancelled(server: Sut, root_token: str) -> None:
    """Cancelling a pending session removes it and its staged content."""
    session = begin_upload(server, root_token, "cover.png", IMAGE.read_bytes())
    assert _staged_exists(server, session)

    response = _cancel(server, root_token, session)

    assert response.status_code == 204
    assert not _staged_exists(server, session)
    assert _session(server, root_token, session).status_code == 404
    replacement = begin_upload(server, root_token, "replacement.png", IMAGE.read_bytes())
    assert _state(server, root_token, replacement)["bitmap"] == "0"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-013")
@allure.story("SCN-UPLOAD-013-B")
def test_a_finished_session_cannot_be_cancelled(server: Sut, root_token: str) -> None:
    """Cancelling a finished session is refused and leaves its file readable."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, session, data)
    completed = _complete(server, root_token, session)

    response = _cancel(server, root_token, session)

    assert response.status_code == 409
    file = server.request("GET", link_of(completed.json(), "file"), token=root_token)
    assert file.status_code == 200, "the completed file was removed by the refused cancellation"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-013")
@allure.story("SCN-UPLOAD-013-C")
def test_an_unknown_session_cannot_be_cancelled(server: Sut, root_token: str) -> None:
    """Cancelling a session that does not exist answers `404`."""
    session = begin_upload(server, root_token, "cover.png", IMAGE.read_bytes())
    assert _cancel(server, root_token, session).status_code == 204

    response = _cancel(server, root_token, session)

    assert response.status_code == 404


# REQ-UPLOAD-014: a finished session keeps its identity.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-014")
@allure.story("SCN-UPLOAD-014-A")
def test_completing_a_finished_session_returns_the_same_file(server: Sut, root_token: str) -> None:
    """A retried completion returns the same file, whose bitmap is complete."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, session, data)
    first = _complete(server, root_token, session)

    again = _complete(server, root_token, session)

    assert again.status_code == 200
    assert _file_id(again) == _file_id(first)
    assert _state(server, root_token, session)["bitmap"] == "1"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-014")
@allure.story("SCN-UPLOAD-014-B")
def test_a_finished_identifier_is_not_reused(server: Sut, root_token: str) -> None:
    """A session created after a finished one carries a different identifier."""
    data = IMAGE.read_bytes()
    finished = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, finished, data)
    _complete(server, root_token, finished)

    later = begin_upload(server, root_token, "cover.png", data)

    assert later.id != finished.id


# REQ-UPLOAD-004: a chunk is written at its declared offset.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-004")
@allure.story("SCN-UPLOAD-004-A")
def test_a_valid_chunk_is_stored_at_its_offset(server: Sut, root_token: str) -> None:
    """A valid chunk is stored and reported in the bitmap."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)

    response = _upload_one(server, root_token, session, data)

    body = response.json()
    assert body["bitmap"] == "1"
    assert _staged_bytes(server, session) == data


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-004")
@allure.story("SCN-UPLOAD-004-B")
def test_a_resent_chunk_changes_nothing(server: Sut, root_token: str) -> None:
    """Re-sending a recorded chunk keeps the stored content and bitmap unchanged."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, session, data)
    before = _staged_digest(server, session)

    response = _chunk(server, root_token, session, 0, data)

    assert response.status_code == 200
    assert _staged_digest(server, session) == before
    assert _state(server, root_token, session)["bitmap"] == "1"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-004")
@allure.story("SCN-UPLOAD-004-C")
def test_a_digest_mismatch_stores_no_bytes(server: Sut, root_token: str) -> None:
    """A chunk whose digest differs from the header stores no bytes and records nothing."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    before = _staged_digest(server, session)

    response = _chunk(server, root_token, session, 0, data, digest=hashlib.sha256(b"different").digest())

    assert response.status_code != 200
    assert _staged_digest(server, session) == before
    assert _state(server, root_token, session)["bitmap"] == "0"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-004")
@allure.story("SCN-UPLOAD-004-D")
def test_a_range_mismatch_stores_no_bytes(server: Sut, root_token: str) -> None:
    """A range that does not match the chunk stores no bytes and records nothing."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    before = _staged_digest(server, session)

    response = _chunk(server, root_token, session, 0, data, start=1)

    assert response.status_code != 200
    assert _staged_digest(server, session) == before
    assert _state(server, root_token, session)["bitmap"] == "0"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-004")
@allure.story("SCN-UPLOAD-004-E")
def test_a_malformed_chunk_header_stores_no_bytes(server: Sut, root_token: str) -> None:
    """A present but malformed range or digest header stores no bytes."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    before = _staged_digest(server, session)

    malformed_range = _chunk(server, root_token, session, 0, data, range_header="bytes nonsense")
    malformed_digest = server.request(
        "PUT",
        expand_template(session.chunk_template, chunk_number=0),
        token=root_token,
        data=data,
        headers={
            "Content-Type": "application/octet-stream",
            "Content-Range": f"bytes 0-{len(data) - 1}/{len(data)}",
            "Content-Digest": "sha-256=:not-base64:",
        },
    )

    assert malformed_range.status_code != 200
    assert malformed_digest.status_code != 200
    assert _staged_digest(server, session) == before
    assert _state(server, root_token, session)["bitmap"] == "0"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-004")
@allure.story("SCN-UPLOAD-004-F")
def test_a_missing_header_stores_no_bytes(server: Sut, root_token: str) -> None:
    """A missing range, a missing digest or an unsupported digest algorithm stores no bytes."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    before = _staged_digest(server, session)

    missing_digest = _chunk(server, root_token, session, 0, data, send_digest=False)
    unsupported_digest = server.request(
        "PUT",
        expand_template(session.chunk_template, chunk_number=0),
        token=root_token,
        data=data,
        headers={
            "Content-Type": "application/octet-stream",
            "Content-Range": f"bytes 0-{len(data) - 1}/{len(data)}",
            "Content-Digest": "sha-512=:not-relevant:",
        },
    )
    missing_range = server.request(
        "PUT",
        expand_template(session.chunk_template, chunk_number=0),
        token=root_token,
        data=data,
        headers={"Content-Type": "application/octet-stream"},
    )

    assert missing_digest.status_code != 200
    assert unsupported_digest.status_code != 200
    assert missing_range.status_code != 200
    assert _staged_digest(server, session) == before
    assert _state(server, root_token, session)["bitmap"] == "0"


# REQ-UPLOAD-005: a chunk is refused when the session is not writable.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-005")
@allure.story("SCN-UPLOAD-005-B")
def test_a_chunk_for_a_finished_session_is_refused(server: Sut, root_token: str) -> None:
    """A chunk for a finished session is a conflict and stores no bytes."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, session, data)
    completed = _complete(server, root_token, session)
    file = _completed_file(server, root_token, completed)
    before = server.sha256(f"{MEDIA_DIRECTORY}/{file['path']}")

    response = _chunk(server, root_token, session, 0, data)

    assert response.status_code == 409
    assert server.sha256(f"{MEDIA_DIRECTORY}/{file['path']}") == before


# REQ-UPLOAD-007: completion verifies the whole-file hash.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-007")
@allure.story("SCN-UPLOAD-007-A")
def test_a_matching_hash_completes(server: Sut, root_token: str) -> None:
    """A session whose staged content matches its hash completes."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    assert completed.status_code == 200
    assert _file_id(completed) > 0


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-007")
@allure.story("SCN-UPLOAD-007-B")
def test_a_hash_mismatch_registers_nothing(server: Sut, root_token: str) -> None:
    """A recomputed hash that differs from the declared one registers nothing and stays retryable."""
    data = IMAGE.read_bytes()
    session, _ = _open(server, root_token, data=data, integrity_hash=hashlib.sha256(b"something else").hexdigest())
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    assert completed.status_code != 200
    assert "file" not in completed.json().get("_links", {})
    assert _media_files(server) == []
    state = _state(server, root_token, session)
    assert state["file_id"] is None
    assert state["is_finished"] is False
    assert _staged_exists(server, session), "a failed verification must leave the session retryable"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-007")
@allure.story("SCN-UPLOAD-007-C")
def test_an_incomplete_session_registers_nothing(server: Sut, root_token: str) -> None:
    """Completing a session missing a chunk registers nothing and leaves it readable."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)

    completed = _complete(server, root_token, session)

    assert completed.status_code != 200
    assert _media_files(server) == []
    state = _state(server, root_token, session)
    assert state["bitmap"] == "0"
    assert state["file_id"] is None
    assert state["is_finished"] is False


# REQ-UPLOAD-009: completion promotes and registers the content together.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-009")
@allure.story("SCN-UPLOAD-009-A")
def test_an_image_is_completed(server: Sut, root_token: str) -> None:
    """A valid image completes to a file and its image row."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data)
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    file = _completed_file(server, root_token, completed)
    assert "image" in file["_links"]
    image = server.follow(file, "image", token=root_token)
    assert image.status_code == 200, f"image read failed: {image.status_code} {image.text}"
    body = image.json()
    assert body["file_id"] == file["id"]
    assert body["width_px"] == 64
    assert body["height_px"] == 64
    assert body["orientation"] == "SQUARE"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-009")
@allure.story("SCN-UPLOAD-009-B")
def test_a_video_is_completed(server: Sut, root_token: str) -> None:
    """A valid video completes to a file and its video row with the probed values."""
    data = VIDEO.read_bytes()
    session = begin_upload(server, root_token, "clip.mp4", data, content_type=VIDEO_CONTENT_TYPE)
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    file = _completed_file(server, root_token, completed)
    assert "video" in file["_links"]
    video = server.follow(file, "video", token=root_token)
    assert video.status_code == 200, f"video read failed: {video.status_code} {video.text}"
    body = video.json()
    assert body["file_id"] == file["id"]
    assert body["codec"].lower() == VIDEO_CODEC
    assert abs(body["duration_ms"] - VIDEO_DURATION_MS) <= 1.0
    assert (body["width"], body["height"]) == VIDEO_DIMENSIONS
    assert body["scan_type"] == "PROGRESSIVE"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-009")
@allure.story("SCN-UPLOAD-009-C")
def test_an_audio_file_is_completed(server: Sut, root_token: str) -> None:
    """A valid audio file completes to a file and its audio row with the probed values."""
    data = AUDIO.read_bytes()
    session = begin_upload(server, root_token, "track.flac", data, content_type=AUDIO_CONTENT_TYPE)
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    file = _completed_file(server, root_token, completed)
    assert "audio" in file["_links"]
    audio = server.follow(file, "audio", token=root_token)
    assert audio.status_code == 200, f"audio read failed: {audio.status_code} {audio.text}"
    body = audio.json()
    assert body["file_id"] == file["id"]
    assert body["codec"].lower() == AUDIO_CODEC
    assert body["sample_rate_hz"] == AUDIO_SAMPLE_RATE_HZ
    assert body["duration_ms"] > 0.0


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-009")
@allure.story("SCN-UPLOAD-009-D")
def test_unreported_audio_values_stay_absent(server: Sut, root_token: str) -> None:
    """A completion records an unreported bit depth or block size as absent."""
    session = begin_upload(server, root_token, "wide.wav", WIDE_WAV, content_type=AUDIO_CONTENT_TYPE)
    _upload_one(server, root_token, session, WIDE_WAV)

    completed = _complete(server, root_token, session)

    file = _completed_file(server, root_token, completed)
    audio = server.follow(file, "audio", token=root_token)
    assert audio.status_code == 200, f"audio read failed: {audio.status_code} {audio.text}"
    body = audio.json()
    assert "bit_depth" not in body
    assert "block_size" not in body
    assert body["codec"].lower() == "pcm_s64le"


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-009")
@allure.story("SCN-UPLOAD-009-E")
def test_content_of_the_wrong_family_registers_nothing(server: Sut, root_token: str) -> None:
    """Content with no stream of the declared family registers neither row."""
    data = AUDIO.read_bytes()
    session = begin_upload(server, root_token, "track.flac", data, content_type=IMAGE_CONTENT_TYPE)
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    assert completed.status_code != 200
    assert _media_files(server) == []


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-009")
@allure.story("SCN-UPLOAD-009-F")
def test_unprobeable_content_registers_nothing(server: Sut, root_token: str) -> None:
    """Content that is not a supported media file registers neither row."""
    data = b"definitely not an image"
    session = begin_upload(server, root_token, "not-an-image.png", data, content_type=IMAGE_CONTENT_TYPE)
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    assert completed.status_code != 200
    assert _media_files(server) == []


# REQ-UPLOAD-010: the declared content type is authoritative.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-010")
@allure.story("SCN-UPLOAD-010-A")
def test_a_declared_audio_type_over_a_video_track_is_stored_as_declared(server: Sut, root_token: str) -> None:
    """An audio declaration over content with both tracks is stored as audio."""
    data = AUDIOVISUAL.read_bytes()
    session = begin_upload(server, root_token, "audiovisual.mp4", data, content_type=AUDIO_MP4_CONTENT_TYPE)
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    file = _completed_file(server, root_token, completed)
    assert file["mime_type"] == AUDIO_MP4_CONTENT_TYPE
    assert "audio" in file["_links"]
    assert "video" not in file["_links"]


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-010")
@allure.story("SCN-UPLOAD-010-B")
def test_content_without_the_declared_family_is_rejected(server: Sut, root_token: str) -> None:
    """An audio declaration over content with no audio stream is rejected."""
    data = IMAGE.read_bytes()
    session = begin_upload(server, root_token, "cover.png", data, content_type=AUDIO_MP4_CONTENT_TYPE)
    _upload_one(server, root_token, session, data)

    completed = _complete(server, root_token, session)

    assert completed.status_code != 200
    assert _media_files(server) == []


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-010")
@allure.story("SCN-UPLOAD-010-C")
def test_a_declared_image_type_is_stored_unchanged(server: Sut, root_token: str) -> None:
    """An image declaration over image content is stored unchanged."""
    session = begin_upload(server, root_token, "cover.png", IMAGE.read_bytes(), content_type=IMAGE_CONTENT_TYPE)
    _upload_one(server, root_token, session, IMAGE.read_bytes())

    completed = _complete(server, root_token, session)

    file = _completed_file(server, root_token, completed)
    assert file["mime_type"] == IMAGE_CONTENT_TYPE
    assert "image" in file["_links"]


# REQ-UPLOAD-011: completion reuses the caller's identical content.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-011")
@allure.story("SCN-UPLOAD-011-A")
def test_a_caller_reuses_its_own_content(server: Sut, root_token: str) -> None:
    """Completing the same bytes twice returns the first file and leaves the second session unfinished."""
    data = IMAGE.read_bytes()
    first = upload(server, root_token, "same.png", data, IMAGE_CONTENT_TYPE)
    second_session = begin_upload(server, root_token, "same.png", data)
    _upload_one(server, root_token, second_session, data)

    second = _complete(server, root_token, second_session)

    assert second.status_code == 200
    assert _file_id(second) == first["id"]
    assert _media_files(server) == [first["path"]], "a duplicate completion stored a second copy"
    state = _state(server, root_token, second_session)
    assert state["is_finished"] is False
    assert state["file_id"] == first["id"]


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-011")
@allure.story("SCN-UPLOAD-011-B")
def test_another_caller_receives_its_own_copy(server: Sut, root_token: str) -> None:
    """A different caller owning the same bytes receives its own copy."""
    first = upload(server, root_token, "same.png", IMAGE.read_bytes(), IMAGE_CONTENT_TYPE)
    _, other = provision(server, root_token, "uploader")

    second = upload(server, other, "same.png", IMAGE.read_bytes(), IMAGE_CONTENT_TYPE)

    assert second["id"] != first["id"]
    assert second["path"] != first["path"]
    assert len(_media_files(server)) == 2


# REQ-UPLOAD-012: a session expires at a fixed instant.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-005")
@allure.story("SCN-UPLOAD-005-A")
@allure.feature("REQ-UPLOAD-012")
@allure.story("SCN-UPLOAD-012-A")
@allure.story("SCN-UPLOAD-012-B")
@allure.story("SCN-UPLOAD-012-C")
@allure.story("SCN-UPLOAD-012-D")
def test_an_expired_session_is_gone(sut: Sut) -> None:
    """An expired session refuses a chunk and a completion, is not readable, and activity did not extend it."""
    server = sut.start(config={"asset": {"upload": {"expiry_seconds": 1}}})
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    data = IMAGE.read_bytes()
    chunk_session = begin_upload(server, token, "cover.png", data)
    complete_session = begin_upload(server, token, "cover.png", data)
    status_session = begin_upload(server, token, "cover.png", data)
    active_session = begin_upload(server, token, "cover.png", data)
    # Activity before the expiry instant must not move it.
    assert put_chunk(server, token, active_session, 0, data, len(data)).status_code == 200

    time.sleep(2)

    chunk = put_chunk(server, token, chunk_session, 0, data, len(data))
    completed = complete_upload(server, token, complete_session)
    read = server.request("GET", status_session.self_href, token=token)
    active = server.request("GET", active_session.self_href, token=token)

    assert chunk.status_code == 410
    assert completed.status_code == 410
    assert read.status_code == 404
    assert active.status_code == 404, "a chunk written before expiry must not extend the session"
    for session in (chunk_session, complete_session, status_session, active_session):
        assert not _staged_exists(server, session), "an expired session's staged content must be removed"
    replacement = begin_upload(server, token, "replacement.png", data)
    assert _state(server, token, replacement)["bitmap"] == "0"


# REQ-UPLOAD-015: expired, stale and orphaned sessions are reaped.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-015")
@allure.story("SCN-UPLOAD-015-A")
@allure.story("SCN-UPLOAD-015-C")
@allure.story("SCN-UPLOAD-015-D")
@allure.story("SCN-UPLOAD-015-E")
def test_the_reaper_removes_expired_and_orphaned_content(sut: Sut) -> None:
    """A reaping run removes an expired session and an orphaned staging file, reports counts, and delays nothing."""
    server = sut.start(config={"asset": {"upload": {"expiry_seconds": 1, "reap_interval_seconds": 1}}})
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    session = begin_upload(server, token, "cover.png", IMAGE.read_bytes())
    server.write(f"{STAGING_DIRECTORY}/9999", b"orphan")

    # A request served while the reaper is running is answered normally.
    assert server.request("GET", server.href("health")).status_code == 200

    time.sleep(2.5)

    assert not _staged_exists(server, session)
    assert server.listing(STAGING_DIRECTORY) == [], "the orphaned staging file survived the reaper"
    assert server.request("GET", session.self_href, token=token).status_code == 404
    replacement = begin_upload(server, token, "replacement.png", IMAGE.read_bytes())
    assert _state(server, token, replacement)["bitmap"] == "0"

    server.stop()
    reports = [line for line in server.log_text().splitlines() if 'event="upload_reap"' in line]
    assert reports, "a reaping run must report its counts"
    for report in reports:
        assert "expired=" in report
        assert "orphaned=" in report
        assert "unsealed=" in report
        assert STAGING_DIRECTORY not in report
        assert "cover.png" not in report


# REQ-UPLOAD-016: an absent library root refuses uploads.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-016")
@allure.story("SCN-UPLOAD-016-A")
@allure.story("SCN-UPLOAD-016-B")
@allure.story("SCN-UPLOAD-016-C")
def test_an_absent_library_root_refuses_uploads(sut: Sut) -> None:
    """An absent root refuses beginning and completing a session while every other route keeps serving."""
    server = sut.start()
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    data = IMAGE.read_bytes()
    session = begin_upload(server, token, "cover.png", data)
    _upload_one(server, token, session, data)
    shutil.rmtree(server.dir / MEDIA_DIRECTORY, ignore_errors=True)

    begin = _begin(server, token)
    completed = _complete(server, token, session)

    assert begin.status_code == 503
    assert completed.status_code == 503
    assert server.listing(STAGING_DIRECTORY) == [str(session.id)]
    assert not server.exists(MEDIA_DIRECTORY)
    assert server.request("GET", server.href("health")).status_code == 200


# REQ-UPLOAD-017: a completed file belongs to its uploader.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-017")
@allure.story("SCN-UPLOAD-017-A")
@allure.story("SCN-UPLOAD-017-B")
@allure.story("SCN-UPLOAD-017-C")
def test_a_completed_file_is_attributed_to_its_uploader(server: Sut, root_token: str) -> None:
    """A completed file is the caller's, private and uploaded, with the content's size, instant and unique path."""
    uploader_id, uploader = provision(server, root_token, "uploader")
    data = IMAGE.read_bytes()

    file = upload(server, uploader, "cover.png", data, IMAGE_CONTENT_TYPE)

    assert link_of(file, "owner").endswith(f"/{uploader_id}")
    assert file["is_public"] is False
    assert file["origin"] == "UPLOADED"
    assert file["size_bytes"] == len(data)
    assert file["modified_at"]
    assert not file["path"].startswith("/")
    assert not file["path"].startswith(f"{MEDIA_DIRECTORY}/")
    assert ".." not in file["path"]
    assert (server.dir / MEDIA_DIRECTORY / file["path"]).read_bytes() == data
    other = upload(server, uploader, "other.png", IMAGE.read_bytes() + b"\x00", IMAGE_CONTENT_TYPE)
    assert other["path"] != file["path"]


# REQ-UPLOAD-003: the chunk size is fixed for the session.


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-003")
@allure.story("SCN-UPLOAD-003-B")
def test_a_new_chunk_size_applies_to_sessions_created_after_a_restart(sut: Sut) -> None:
    """A restart applies a new configured chunk size to new sessions and leaves an existing session's geometry."""
    server = sut.start(config={"asset": {"upload": {"chunk_size_bytes": 1024}}})
    password = server.root_password()
    token = server.login(ROOT_ADMIN_USERNAME, password)
    old, old_chunk_size = _open(server, token, file_name="old.png", data=b"\x01" * 2048)

    server.restart(config={"asset": {"upload": {"chunk_size_bytes": 2048}}})
    token = server.login(ROOT_ADMIN_USERNAME, password)
    new, new_chunk_size = _open(server, token, file_name="new.png", data=b"\x02" * 2048)

    assert old_chunk_size == 1024
    assert new_chunk_size == 2048
    assert put_chunk(server, token, old, 0, b"\x01" * 1024, 2048).status_code == 200
    assert put_chunk(server, token, new, 0, b"\x02" * 2048, 2048).status_code == 200


@allure.epic("Upload")
@allure.feature("REQ-UPLOAD-001")
@allure.story("SCN-UPLOAD-001-E")
def test_a_file_over_the_configured_maximum_creates_nothing(sut: Sut) -> None:
    """A declared size above the configured maximum creates no session and no staged content."""
    server = sut.start(config={"asset": {"upload": {"max_file_size_bytes": 2048}}})
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    response = _begin(server, token, data=b"\x00" * 4096)

    assert response.status_code != 201, "an oversized upload was accepted"
    assert server.listing(STAGING_DIRECTORY) == []
