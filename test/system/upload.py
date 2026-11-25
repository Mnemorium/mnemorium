"""Black-box proof of the `upload` capability."""

import hashlib
import json
import shutil
import time
from itertools import count

import pytest
from conftest import (
    SEED_MEDIA_DIRECTORY,
    begin,
    completed_file,
    login,
    provision,
    session_state,
    staged_path,
    start_instance,
    store_chunk,
)
from harness import ROOT_ADMIN_USERNAME, STAGING_DIRECTORY, Sut
from labels import TestType, business_rule, error_handling, negative, positive, recovery, scenario
from mnemorium_client.api.asset import (
    delete_upload,
    get_audio,
    get_image,
    get_video,
    list_files,
    post_upload,
    post_upload_complete,
    put_upload_chunk,
)
from mnemorium_client.api.user import get_me
from mnemorium_client.models import PostUploadRequest
from mnemorium_client.types import File

IMAGE = (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes()
VIDEO = (SEED_MEDIA_DIRECTORY / "file-video.mp4").read_bytes()
AUDIO = (SEED_MEDIA_DIRECTORY / "file-audio.flac").read_bytes()
AUDIOVISUAL = (SEED_MEDIA_DIRECTORY / "upload-audiovisual.mp4").read_bytes()
UNMEASURED_AUDIO = (SEED_MEDIA_DIRECTORY / "audio-wide.wav").read_bytes()

IMAGE_CONTENT_TYPE = "image/png"
VIDEO_CONTENT_TYPE = "video/mp4"
AUDIO_CONTENT_TYPE = "audio/flac"
AUDIO_MP4_CONTENT_TYPE = "audio/mp4"

MEDIA_DIRECTORY = "media"

VIDEO_CODEC = "h264"
VIDEO_DURATION_MS = 200.0
VIDEO_DIMENSIONS = (16, 16)
AUDIO_CODEC = "flac"
AUDIO_SAMPLE_RATE_HZ = 44_100

_ACCOUNTS = count()


@positive
@scenario("SCN-UPLOAD-001-A")
def test_a_valid_request_creates_a_session(server: Sut, root_token: str) -> None:
    """A valid declaration creates a readable session and its staged content."""
    session = begin(server, root_token, "cover.png", IMAGE)

    assert server.exists(staged_path(server, session)), "a valid begin must create the staged content"
    assert (server.dir / staged_path(server, session)).stat().st_size == len(IMAGE)
    assert session_state(server, root_token, session)["bitmap"] == "0"


@negative
@scenario("SCN-UPLOAD-001-B")
@scenario("SCN-UPLOAD-001-C")
@scenario("SCN-UPLOAD-001-D")
@scenario("SCN-UPLOAD-001-E")
@scenario("SCN-UPLOAD-001-F")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_invalid_declaration_creates_nothing(sut: Sut) -> None:
    """Unsafe names, an unsupported type, a zero or oversized size and a malformed hash create nothing."""
    server = sut.start(config={"asset": {"upload": {"max_file_size_bytes": 2048}}})
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    declarations = [
        {"file_name": ""},
        {"file_name": ".hidden.png"},
        {"file_name": "../escape.png"},
        {"file_name": "sub/cover.png"},
        {"content_type": "application/x-not-a-media-type"},
        {"file_size": 0, "integrity_hash": hashlib.sha256(b"").hexdigest()},
        {"data": b"\x00" * 4096},
        {"integrity_hash": ""},
        {"integrity_hash": "not-a-digest"},
        {"integrity_hash": "A" * 64},
        {"integrity_hash": "a" * 63},
    ]

    for declaration in declarations:
        data = declaration.get("data", IMAGE)
        response = post_upload.sync_detailed(
            client=server.client(token),
            body=PostUploadRequest(
                content_type=declaration.get("content_type", IMAGE_CONTENT_TYPE),
                file_name=declaration.get("file_name", "cover.png"),
                file_size=declaration.get("file_size", len(data)),
                integrity_hash=declaration.get("integrity_hash", hashlib.sha256(data).hexdigest()),
            ),
        )

        assert response.status_code != 201, f"the declaration was accepted: {declaration!r}"

    assert server.listing(STAGING_DIRECTORY) == [], "a refused declaration staged content"
    assert json.loads(list_files.sync_detailed(client=server.client(token)).content)["items"] == []


@recovery
@scenario("SCN-UPLOAD-002-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_failed_begin_leaves_nothing(sut: Sut) -> None:
    """A refused begin persists no session and leaves no staged content."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    refused = post_upload.sync_detailed(
        client=server.client(token),
        body=PostUploadRequest(
            content_type="application/x-not-a-media-type",
            file_name="cover.png",
            file_size=len(IMAGE),
            integrity_hash=hashlib.sha256(IMAGE).hexdigest(),
        ),
    )
    assert refused.status_code != 201

    # The next session takes the first identifier a committed row would have taken, so no earlier session exists and no
    # staged file survived the failure.
    session = begin(server, token, "cover.png", IMAGE)

    assert session.id == 1
    assert server.listing(STAGING_DIRECTORY) == [str(session.id)]


@recovery
@scenario("SCN-UPLOAD-002-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_leftover_staging_file_is_reclaimed(sut: Sut) -> None:
    """A staged file planted for the next identifier is reclaimed by the begin."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.write(f"{STAGING_DIRECTORY}/1", b"stale")

    session = begin(server, token, "cover.png", IMAGE)

    assert session.id == 1
    assert (server.dir / staged_path(server, session)).read_bytes() == b"\x00" * len(IMAGE), (
        "the stale content was not reclaimed"
    )


@recovery
@scenario("SCN-UPLOAD-003-B")
@pytest.mark.own_instance(reason="restart")
def test_a_new_chunk_size_applies_to_sessions_created_after_a_restart(sut: Sut) -> None:
    """A restart applies a new configured chunk size to new sessions and leaves an existing session's geometry."""
    server = sut.start(config={"asset": {"upload": {"chunk_size_bytes": 1024}}})
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    old = begin(server, token, "old.png", b"\x01" * 2048)

    server.restart(config={"asset": {"upload": {"chunk_size_bytes": 2048}}})
    token = login(server, ROOT_ADMIN_USERNAME, password)
    new = begin(server, token, "new.png", b"\x02" * 2048)

    assert old.chunk_size == 1024
    assert new.chunk_size == 2048
    assert store_chunk(server, token, old, 0, b"\x01" * 1024, total=2048).status_code == 200
    assert store_chunk(server, token, new, 0, b"\x02" * 2048, total=2048).status_code == 200


@positive
@scenario("SCN-UPLOAD-004-A")
def test_a_valid_chunk_is_stored_at_its_offset(server: Sut, root_token: str) -> None:
    """A valid chunk is stored and reported in the bitmap."""
    session = begin(server, root_token, "cover.png", IMAGE)

    response = store_chunk(server, root_token, session, 0, IMAGE)

    assert response.status_code == 200
    assert json.loads(response.content)["bitmap"] == "1"
    assert (server.dir / staged_path(server, session)).read_bytes() == IMAGE


@business_rule
@scenario("SCN-UPLOAD-004-B")
def test_a_resent_chunk_changes_nothing(server: Sut, root_token: str) -> None:
    """Re-sending a recorded chunk keeps the stored content and bitmap unchanged."""
    session = begin(server, root_token, "cover.png", IMAGE)
    store_chunk(server, root_token, session, 0, IMAGE)
    before = server.sha256(staged_path(server, session))

    response = store_chunk(server, root_token, session, 0, IMAGE)

    assert response.status_code == 200
    assert server.sha256(staged_path(server, session)) == before
    assert session_state(server, root_token, session)["bitmap"] == "1"


@negative
@scenario("SCN-UPLOAD-004-C")
def test_a_digest_mismatch_stores_no_bytes(server: Sut, root_token: str) -> None:
    """A chunk whose digest differs from the header stores no bytes and records nothing."""
    session = begin(server, root_token, "cover.png", IMAGE)
    before = server.sha256(staged_path(server, session))

    response = store_chunk(
        server,
        root_token,
        session,
        0,
        IMAGE,
        digest=hashlib.sha256(b"different").digest(),
    )

    assert response.status_code != 200
    assert server.sha256(staged_path(server, session)) == before
    assert session_state(server, root_token, session)["bitmap"] == "0"


@negative
@scenario("SCN-UPLOAD-004-D")
def test_a_range_mismatch_stores_no_bytes(server: Sut, root_token: str) -> None:
    """A range that does not match the chunk stores no bytes and records nothing."""
    session = begin(server, root_token, "cover.png", IMAGE)
    before = server.sha256(staged_path(server, session))

    response = store_chunk(server, root_token, session, 0, IMAGE, start=1)

    assert response.status_code != 200
    assert server.sha256(staged_path(server, session)) == before
    assert session_state(server, root_token, session)["bitmap"] == "0"


@negative
@scenario("SCN-UPLOAD-004-E")
def test_a_malformed_chunk_header_stores_no_bytes(server: Sut, root_token: str) -> None:
    """A present but malformed range or digest header stores no bytes."""
    session = begin(server, root_token, "cover.png", IMAGE)
    before = server.sha256(staged_path(server, session))

    malformed_range = put_upload_chunk.sync_detailed(
        session.id,
        0,
        client=server.client(root_token),
        body=File(payload=IMAGE),
        content_range="bytes nonsense",
        content_digest=f"sha-256=:{hashlib.sha256(IMAGE).digest()!r}:",
    )
    malformed_digest = put_upload_chunk.sync_detailed(
        session.id,
        0,
        client=server.client(root_token),
        body=File(payload=IMAGE),
        content_range=f"bytes 0-{len(IMAGE) - 1}/{len(IMAGE)}",
        content_digest="sha-256=:not-base64:",
    )

    assert malformed_range.status_code != 200
    assert malformed_digest.status_code != 200
    assert server.sha256(staged_path(server, session)) == before
    assert session_state(server, root_token, session)["bitmap"] == "0"


@negative
@scenario("SCN-UPLOAD-004-F")
def test_a_missing_header_stores_no_bytes(server: Sut, root_token: str) -> None:
    """A missing range, a missing digest or an unsupported digest algorithm stores no bytes."""
    session = begin(server, root_token, "cover.png", IMAGE)
    before = server.sha256(staged_path(server, session))

    missing_digest = put_upload_chunk.sync_detailed(
        session.id,
        0,
        client=server.client(root_token),
        body=File(payload=IMAGE),
        content_range=f"bytes 0-{len(IMAGE) - 1}/{len(IMAGE)}",
        content_digest="",
    )
    unsupported_digest = put_upload_chunk.sync_detailed(
        session.id,
        0,
        client=server.client(root_token),
        body=File(payload=IMAGE),
        content_range=f"bytes 0-{len(IMAGE) - 1}/{len(IMAGE)}",
        content_digest="sha-512=:not-relevant:",
    )
    missing_range = put_upload_chunk.sync_detailed(
        session.id,
        0,
        client=server.client(root_token),
        body=File(payload=IMAGE),
        content_range="",
        content_digest=f"sha-256=:{hashlib.sha256(IMAGE).digest()!r}:",
    )

    assert missing_digest.status_code != 200
    assert unsupported_digest.status_code != 200
    assert missing_range.status_code != 200
    assert server.sha256(staged_path(server, session)) == before
    assert session_state(server, root_token, session)["bitmap"] == "0"


@scenario("SCN-UPLOAD-005-B", TestType.NEGATIVE)
@scenario("SCN-API-011-C", TestType.BUSINESS_RULE)
def test_a_chunk_for_a_finished_session_is_refused(server: Sut, root_token: str) -> None:
    """A chunk for a finished session is a conflict and stores no bytes."""
    data = IMAGE + b"\x00" * (next(_ACCOUNTS) + 1)
    session = begin(server, root_token, "cover.png", data)
    store_chunk(server, root_token, session, 0, data)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    file = completed_file(server, root_token, completed)
    before = server.sha256(f"{MEDIA_DIRECTORY}/{file['path']}")

    response = store_chunk(server, root_token, session, 0, IMAGE)

    assert response.status_code == 409
    assert server.sha256(f"{MEDIA_DIRECTORY}/{file['path']}") == before


@positive
@scenario("SCN-UPLOAD-006-A")
def test_a_known_session_is_read(server: Sut, root_token: str) -> None:
    """A known session reports its bitmap, count, expiry, finish flag, file and links."""
    data = IMAGE + b"\x00" * (next(_ACCOUNTS) + 1)
    session = begin(server, root_token, "cover.png", data)
    store_chunk(server, root_token, session, 0, data)

    body = session_state(server, root_token, session)

    assert body["bitmap"] == "1"
    assert body["total_chunks"] == 1
    assert body["is_finished"] is False
    assert body["file_id"] is None
    assert body["expires_at"]
    assert body["_links"]["chunk"]["href"] == session.chunk_template
    assert body["_links"]["chunk"]["templated"] is True
    assert body["_links"]["complete"]["href"] == session.complete_href
    assert body["_links"]["self"]["href"] == session.self_href


@positive
@scenario("SCN-UPLOAD-006-C")
def test_a_finished_session_reports_a_complete_bitmap(server: Sut, root_token: str) -> None:
    """A finished session reports every chunk as received and the caller's file."""
    data = IMAGE + b"\x00" * (next(_ACCOUNTS) + 1)
    session = begin(server, root_token, "cover.png", data)
    store_chunk(server, root_token, session, 0, data)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    file = completed_file(server, root_token, completed)

    body = session_state(server, root_token, session)

    assert body["bitmap"] == "1" * body["total_chunks"]
    assert body["is_finished"] is True
    assert body["file_id"] == file["id"]


@business_rule
@scenario("SCN-UPLOAD-006-D")
def test_no_collection_of_sessions_exists(server: Sut, root_token: str) -> None:
    """The upload entry point is an operation, not a collection that lists sessions."""
    begin(server, root_token, "cover.png", IMAGE)

    response = server.request("GET", server.href("upload"), token=root_token)

    assert response.status_code != 200, "the upload collection must not be readable"
    assert "upload_id" not in response.text


@positive
@scenario("SCN-UPLOAD-007-A")
@scenario("SCN-API-004-C", TestType.POSITIVE)
def test_a_matching_hash_completes(server: Sut, root_token: str) -> None:
    """A session whose staged content matches its hash completes."""
    session = begin(server, root_token, "cover.png", IMAGE)
    store_chunk(server, root_token, session, 0, IMAGE)

    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    file = completed_file(server, root_token, completed)

    assert completed.status_code == 200
    assert file["id"] > 0
    body = json.loads(completed.content)
    assert body["_links"]["self"]["href"] == session.self_href
    assert "file" in body["_links"], "the produced file is not a named link"


@negative
@scenario("SCN-UPLOAD-007-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_hash_mismatch_registers_nothing(sut: Sut) -> None:
    """A recomputed hash that differs from the declared one registers nothing and stays retryable."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    session = begin(
        server,
        token,
        "cover.png",
        IMAGE,
        integrity_hash=hashlib.sha256(b"something else").hexdigest(),
    )
    store_chunk(server, token, session, 0, IMAGE)

    completed = post_upload_complete.sync_detailed(session.id, client=server.client(token))

    assert completed.status_code != 200
    assert json.loads(list_files.sync_detailed(client=server.client(token)).content)["items"] == []
    state = session_state(server, token, session)
    assert state["file_id"] is None
    assert state["is_finished"] is False
    assert server.exists(staged_path(server, session)), "a failed verification must leave the session retryable"


@negative
@scenario("SCN-UPLOAD-007-C")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_incomplete_session_registers_nothing(sut: Sut) -> None:
    """Completing a session missing a chunk registers nothing and leaves it readable."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    session = begin(server, token, "cover.png", IMAGE)

    completed = post_upload_complete.sync_detailed(session.id, client=server.client(token))

    assert completed.status_code != 200
    assert json.loads(list_files.sync_detailed(client=server.client(token)).content)["items"] == []
    state = session_state(server, token, session)
    assert state["bitmap"] == "0"
    assert state["file_id"] is None
    assert state["is_finished"] is False


@positive
@scenario("SCN-UPLOAD-009-A")
def test_an_image_is_completed(server: Sut, root_token: str) -> None:
    """A valid image completes to a file and its image row."""
    session = begin(server, root_token, "cover.png", IMAGE)
    store_chunk(server, root_token, session, 0, IMAGE)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    file = completed_file(server, root_token, completed)

    assert "image" in file["_links"]
    image_id = int(file["_links"]["image"]["href"].rsplit("/", 1)[1])
    response = get_image.sync_detailed(image_id, client=server.client(root_token))

    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)
    assert body["file_id"] == file["id"]
    assert body["width_px"] == 64
    assert body["height_px"] == 64
    assert body["orientation"] == "SQUARE"


@positive
@scenario("SCN-UPLOAD-009-B")
def test_a_video_is_completed(server: Sut, root_token: str) -> None:
    """A valid video completes to a file and its video row with the probed values."""
    session = begin(server, root_token, "clip.mp4", VIDEO, VIDEO_CONTENT_TYPE)
    store_chunk(server, root_token, session, 0, VIDEO)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    file = completed_file(server, root_token, completed)

    assert "video" in file["_links"]
    video_id = int(file["_links"]["video"]["href"].rsplit("/", 1)[1])
    response = get_video.sync_detailed(video_id, client=server.client(root_token))

    assert response.status_code == 200, f"video read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)
    assert body["file_id"] == file["id"]
    assert body["codec"].lower() == VIDEO_CODEC
    assert abs(body["duration_ms"] - VIDEO_DURATION_MS) <= 1.0
    assert (body["width"], body["height"]) == VIDEO_DIMENSIONS
    assert body["scan_type"] == "PROGRESSIVE"


@positive
@scenario("SCN-UPLOAD-009-C")
def test_an_audio_file_is_completed(server: Sut, root_token: str) -> None:
    """A valid audio file completes to a file and its audio row with the probed values."""
    session = begin(server, root_token, "track.flac", AUDIO, AUDIO_CONTENT_TYPE)
    store_chunk(server, root_token, session, 0, AUDIO)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    file = completed_file(server, root_token, completed)

    assert "audio" in file["_links"]
    audio_id = int(file["_links"]["audio"]["href"].rsplit("/", 1)[1])
    response = get_audio.sync_detailed(audio_id, client=server.client(root_token))

    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)
    assert body["file_id"] == file["id"]
    assert body["codec"].lower() == AUDIO_CODEC
    assert body["sample_rate_hz"] == AUDIO_SAMPLE_RATE_HZ
    assert body["duration_ms"] > 0.0


@business_rule
@scenario("SCN-UPLOAD-009-D")
def test_unreported_audio_values_stay_absent(server: Sut, root_token: str) -> None:
    """A completion records an unreported bit depth or block size as absent."""
    session = begin(server, root_token, "wide.wav", UNMEASURED_AUDIO, AUDIO_CONTENT_TYPE)
    store_chunk(server, root_token, session, 0, UNMEASURED_AUDIO)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    file = completed_file(server, root_token, completed)

    audio_id = int(file["_links"]["audio"]["href"].rsplit("/", 1)[1])
    response = get_audio.sync_detailed(audio_id, client=server.client(root_token))

    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)
    assert "bit_depth" not in body
    assert "block_size" not in body
    assert body["codec"].lower() == "pcm_s64le"


@negative
@scenario("SCN-UPLOAD-009-E")
@scenario("SCN-UPLOAD-010-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_content_without_the_declared_family_registers_nothing(sut: Sut) -> None:
    """Content with no stream of the declared family registers neither row."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    for data, name, content_type in (
        (AUDIO, "track.flac", IMAGE_CONTENT_TYPE),
        (IMAGE, "cover.png", AUDIO_MP4_CONTENT_TYPE),
    ):
        session = begin(server, token, name, data, content_type)
        store_chunk(server, token, session, 0, data)
        completed = post_upload_complete.sync_detailed(session.id, client=server.client(token))

        assert completed.status_code != 200, f"{name} was registered as {content_type}"
        assert json.loads(list_files.sync_detailed(client=server.client(token)).content)["items"] == []


@negative
@scenario("SCN-UPLOAD-009-F")
@pytest.mark.own_instance(reason="global_filesystem")
def test_unprobeable_content_registers_nothing(sut: Sut) -> None:
    """Content that is not a supported media file registers neither row."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    data = b"definitely not an image"
    session = begin(server, token, "not-an-image.png", data, IMAGE_CONTENT_TYPE)
    store_chunk(server, token, session, 0, data)

    completed = post_upload_complete.sync_detailed(session.id, client=server.client(token))

    assert completed.status_code != 200
    assert json.loads(list_files.sync_detailed(client=server.client(token)).content)["items"] == []


@business_rule
@scenario("SCN-UPLOAD-010-A")
def test_a_declared_audio_type_over_a_video_track_is_stored_as_declared(server: Sut, root_token: str) -> None:
    """An audio declaration over content with both tracks is stored as audio."""
    session = begin(server, root_token, "audiovisual.mp4", AUDIOVISUAL, AUDIO_MP4_CONTENT_TYPE)
    store_chunk(server, root_token, session, 0, AUDIOVISUAL)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))

    file = completed_file(server, root_token, completed)

    assert file["mime_type"] == AUDIO_MP4_CONTENT_TYPE
    assert "audio" in file["_links"]
    assert "video" not in file["_links"]


@positive
@scenario("SCN-UPLOAD-010-C")
def test_a_declared_image_type_is_stored_unchanged(server: Sut, root_token: str) -> None:
    """An image declaration over image content is stored unchanged."""
    session = begin(server, root_token, "cover.png", IMAGE, IMAGE_CONTENT_TYPE)
    store_chunk(server, root_token, session, 0, IMAGE)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))

    file = completed_file(server, root_token, completed)

    assert file["mime_type"] == IMAGE_CONTENT_TYPE
    assert "image" in file["_links"]


@business_rule
@scenario("SCN-UPLOAD-011-A")
def test_a_caller_reuses_its_own_content(server: Sut, root_token: str) -> None:
    """Completing the same bytes twice returns the first file and leaves the second session unfinished."""
    data = IMAGE + b"\x00" * (next(_ACCOUNTS) + 1)
    first = begin(server, root_token, "same.png", data)
    store_chunk(server, root_token, first, 0, data)
    first_completed = post_upload_complete.sync_detailed(first.id, client=server.client(root_token))
    first_file = completed_file(server, root_token, first_completed)

    second = begin(server, root_token, "same.png", data)
    store_chunk(server, root_token, second, 0, data)
    second_completed = post_upload_complete.sync_detailed(second.id, client=server.client(root_token))
    second_file = completed_file(server, root_token, second_completed)

    assert second_completed.status_code == 200
    assert second_file["id"] == first_file["id"], "a duplicate completion stored a second copy"
    state = session_state(server, root_token, second)
    assert state["is_finished"] is False
    assert state["file_id"] == first_file["id"]


@business_rule
@scenario("SCN-UPLOAD-011-B")
def test_another_caller_receives_its_own_copy(server: Sut, root_token: str) -> None:
    """A different caller owning the same bytes receives its own copy."""
    data = IMAGE + b"\x00" * (next(_ACCOUNTS) + 1)
    first = begin(server, root_token, "same.png", data)
    store_chunk(server, root_token, first, 0, data)
    first_completed = post_upload_complete.sync_detailed(first.id, client=server.client(root_token))
    first_file = completed_file(server, root_token, first_completed)
    _, other = provision(server, root_token, f"uploader{next(_ACCOUNTS)}")

    second = begin(server, other, "same.png", data)
    store_chunk(server, other, second, 0, data)
    second_completed = post_upload_complete.sync_detailed(second.id, client=server.client(other))
    second_file = completed_file(server, other, second_completed)

    assert second_file["id"] != first_file["id"]
    assert second_file["path"] != first_file["path"]


@positive
@scenario("SCN-UPLOAD-013-A")
def test_a_pending_session_is_cancelled(server: Sut, root_token: str) -> None:
    """Cancelling a pending session removes it and its staged content."""
    session = begin(server, root_token, "cover.png", IMAGE)
    assert server.exists(staged_path(server, session))

    response = delete_upload.sync_detailed(session.id, client=server.client(root_token))

    assert response.status_code == 204
    assert not server.exists(staged_path(server, session))
    assert delete_upload.sync_detailed(session.id, client=server.client(root_token)).status_code == 404
    replacement = begin(server, root_token, "replacement.png", IMAGE)
    assert session_state(server, root_token, replacement)["bitmap"] == "0"


@negative
@scenario("SCN-UPLOAD-013-B")
def test_a_finished_session_cannot_be_cancelled(server: Sut, root_token: str) -> None:
    """Cancelling a finished session is refused and leaves its file readable."""
    data = IMAGE + b"\x00" * (next(_ACCOUNTS) + 1)
    session = begin(server, root_token, "cover.png", data)
    store_chunk(server, root_token, session, 0, data)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    file = completed_file(server, root_token, completed)

    response = delete_upload.sync_detailed(session.id, client=server.client(root_token))

    assert response.status_code == 409
    assert server.request("GET", file["_links"]["self"]["href"], token=root_token).status_code == 200, (
        "the completed file was removed by the refused cancellation"
    )


@business_rule
@scenario("SCN-UPLOAD-014-A")
def test_completing_a_finished_session_returns_the_same_file(server: Sut, root_token: str) -> None:
    """A retried completion returns the same file, whose bitmap is complete."""
    data = IMAGE + b"\x00" * (next(_ACCOUNTS) + 1)
    session = begin(server, root_token, "cover.png", data)
    store_chunk(server, root_token, session, 0, data)
    first = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    first_file = completed_file(server, root_token, first)

    again = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))
    again_file = completed_file(server, root_token, again)

    assert again.status_code == 200
    assert again_file["id"] == first_file["id"]
    assert session_state(server, root_token, session)["bitmap"] == "1"


@business_rule
@scenario("SCN-UPLOAD-014-B")
def test_a_finished_identifier_is_not_reused(server: Sut, root_token: str) -> None:
    """A session created after a finished one carries a different identifier."""
    finished = begin(server, root_token, "cover.png", IMAGE)
    store_chunk(server, root_token, finished, 0, IMAGE)
    post_upload_complete.sync_detailed(finished.id, client=server.client(root_token))

    later = begin(server, root_token, "cover.png", IMAGE)

    assert later.id != finished.id


@negative
@scenario("SCN-UPLOAD-005-A")
@scenario("SCN-UPLOAD-012-A")
@scenario("SCN-UPLOAD-012-B")
@scenario("SCN-UPLOAD-012-C")
@scenario("SCN-UPLOAD-012-D")
@scenario("SCN-API-011-C", TestType.BUSINESS_RULE)
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_expired_session_is_gone(sut: Sut) -> None:
    """An expired session refuses a chunk and a completion, is not readable, and activity did not extend it."""
    # Three seconds is the longest deliberate wait TEST-056 allows. It keeps the pre-expiry chunk write clear of the
    # one-second boundary, and the sleep starts after the sessions are begun, so their configured instant has passed.
    server = sut.start(config={"asset": {"upload": {"expiry_seconds": 3}}})
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    chunk_session = begin(server, token, "cover.png", IMAGE)
    complete_session = begin(server, token, "cover.png", IMAGE)
    status_session = begin(server, token, "cover.png", IMAGE)
    active_session = begin(server, token, "cover.png", IMAGE)
    # Activity before the expiry instant must not move it.
    assert store_chunk(server, token, active_session, 0, IMAGE).status_code == 200

    time.sleep(3)

    chunk = store_chunk(server, token, chunk_session, 0, IMAGE)
    completed = post_upload_complete.sync_detailed(complete_session.id, client=server.client(token))
    read = server.request("GET", status_session.self_href, token=token)
    active = server.request("GET", active_session.self_href, token=token)

    assert chunk.status_code == 410
    assert completed.status_code == 410
    assert read.status_code == 404
    assert active.status_code == 404, "a chunk written before expiry must not extend the session"
    for session in (chunk_session, complete_session, status_session, active_session):
        assert not server.exists(staged_path(server, session)), "an expired session's staged content must be removed"


@recovery
@scenario("SCN-UPLOAD-015-A")
@scenario("SCN-UPLOAD-015-C")
@scenario("SCN-UPLOAD-015-D")
@scenario("SCN-UPLOAD-015-E")
@scenario("SCN-LOGGING-017-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_the_reaper_removes_expired_and_orphaned_content(sut: Sut) -> None:
    """A reaping run removes an expired session and an orphaned staging file, reports counts, and delays nothing."""
    server = sut.start(config={"asset": {"upload": {"expiry_seconds": 1, "reap_interval_seconds": 1}}})
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    session = begin(server, token, "cover.png", IMAGE)
    server.write(f"{STAGING_DIRECTORY}/9999", b"orphan")

    # A request served while the reaper is running is answered normally.
    assert server.request("GET", server.href("health")).status_code == 200

    time.sleep(2.5)

    assert not server.exists(staged_path(server, session))
    assert server.listing(STAGING_DIRECTORY) == [], "the orphaned staging file survived the reaper"
    assert server.request("GET", session.self_href, token=token).status_code == 404

    server.stop()
    reports = [line for line in server.log_text().splitlines() if 'event="upload_reap"' in line]
    assert reports, "a reaping run must report its counts"
    for report in reports:
        assert "expired=" in report
        assert "orphaned=" in report
        assert "unsealed=" in report
        assert STAGING_DIRECTORY not in report
        assert "cover.png" not in report


@error_handling
@scenario("SCN-UPLOAD-016-A")
@scenario("SCN-UPLOAD-016-B")
@scenario("SCN-UPLOAD-016-C")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_absent_library_root_refuses_uploads(sut: Sut) -> None:
    """An absent root refuses beginning and completing a session while every other route keeps serving."""
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    session = begin(server, token, "cover.png", IMAGE)
    store_chunk(server, token, session, 0, IMAGE)
    shutil.rmtree(server.dir / MEDIA_DIRECTORY, ignore_errors=True)

    refused = post_upload.sync_detailed(
        client=server.client(token),
        body=PostUploadRequest(
            content_type=IMAGE_CONTENT_TYPE,
            file_name="cover.png",
            file_size=len(IMAGE),
            integrity_hash=hashlib.sha256(IMAGE).hexdigest(),
        ),
    )
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(token))

    assert refused.status_code == 503
    assert completed.status_code == 503
    assert server.listing(STAGING_DIRECTORY) == [str(session.id)]
    assert not server.exists(MEDIA_DIRECTORY)
    assert server.request("GET", server.href("health")).status_code == 200


@business_rule
@scenario("SCN-UPLOAD-017-A")
@scenario("SCN-UPLOAD-017-B")
@scenario("SCN-UPLOAD-017-C")
def test_a_completed_file_is_attributed_to_its_uploader(server: Sut, root_token: str) -> None:
    """A completed file is the caller's, private and uploaded, with the content's size, instant and unique path."""
    _, uploader = provision(server, root_token, f"uploader{next(_ACCOUNTS)}")
    data = IMAGE + b"\x00" * (next(_ACCOUNTS) + 1)
    session = begin(server, uploader, "cover.png", data)
    store_chunk(server, uploader, session, 0, data)
    completed = post_upload_complete.sync_detailed(session.id, client=server.client(uploader))
    file = completed_file(server, uploader, completed)

    current = get_me.sync_detailed(client=server.client(uploader))

    assert current.status_code == 200
    assert file["_links"]["owner"]["href"] == json.loads(current.content)["_links"]["self"]["href"]
    assert file["is_public"] is False
    assert file["origin"] == "UPLOADED"
    assert file["size_bytes"] == len(data)
    assert file["modified_at"]
    assert not file["path"].startswith("/")
    assert not file["path"].startswith(f"{MEDIA_DIRECTORY}/")
    assert ".." not in file["path"]
    assert (server.dir / MEDIA_DIRECTORY / file["path"]).read_bytes() == data
    other_data = data + b"\x00"
    other_session = begin(server, uploader, "other.png", other_data)
    store_chunk(server, uploader, other_session, 0, other_data)
    other_completed = post_upload_complete.sync_detailed(other_session.id, client=server.client(uploader))
    other = completed_file(server, uploader, other_completed)
    assert other["path"] != file["path"]
