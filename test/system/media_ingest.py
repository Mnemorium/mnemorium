"""Black-box proof of the `media-ingest` capability."""

import json
import os
import shutil
import time
from datetime import UTC, datetime
from itertools import count

import pytest
from conftest import (
    SEED_MEDIA_DIRECTORY,
    add_to_gallery,
    create_gallery,
    file_at,
    login,
    upload,
    wait_for_ingest,
)
from harness import (
    API_PREFIX,
    MEDIA_DIRECTORY,
    MSG_MEDIA_ROOT_ABSENT,
    ROOT_ADMIN_USERNAME,
    Sut,
)
from labels import alternate_flow, business_rule, error_handling, positive, scenario
from mnemorium_client.api.asset import get_audio, get_file, get_image, get_video
from mnemorium_client.api.library import delete_gallery, delete_gallery_item
from mnemorium_client.api.system import get_api_index
from mnemorium_client.api.user import get_me

IMAGE_PATH = "file-linked.png"
VIDEO_PATH = "file-video.mp4"
FLAC_PATH = "file-audio.flac"
IMAGE_CONTENT_TYPE = "image/png"

# A file that is not media content: a PDF the sniffer recognises as one and the ingest refuses.
PDF_BYTES = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n"

INSTANT = 1_700_000_000

_ACCOUNTS = count()


def mp3_frame_bytes() -> bytes:
    """Build a short, decodable MPEG-1 Layer III stream: twenty silent frame headers with their payloads."""
    return (bytes([0xFF, 0xFB, 0x90, 0x00]) + bytes(413)) * 20


@positive
@scenario("SCN-MEDIA-INGEST-001-A")
def test_a_promoted_upload_is_addressed_under_the_root(server: Sut, root_token: str) -> None:
    """A file the server stored through upload records a path relative to the root that resolves under it."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    name = f"promoted{next(_ACCOUNTS)}.png"

    file = upload(server, root_token, name, data)

    assert not file["path"].startswith("/"), "a stored path must be root-relative"
    assert not file["path"].startswith(f"{MEDIA_DIRECTORY}/"), "a stored path is relative to the root itself"
    assert (server.dir / MEDIA_DIRECTORY / file["path"]).read_bytes() == data


@positive
@scenario("SCN-MEDIA-INGEST-001-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_found_file_is_addressed_under_the_same_root(sut: Sut) -> None:
    """A file the scan found records a path relative to the same root that resolves to that file."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    sut.write(f"{MEDIA_DIRECTORY}/found.png", data)
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.wait_until(
        lambda: file_at(server, token, "found.png") is not None,
        "the scan registered the found file",
    )

    record = file_at(server, token, "found.png")

    assert record is not None
    assert not record["path"].startswith("/"), "a found path must be root-relative"
    assert (server.dir / MEDIA_DIRECTORY / record["path"]).read_bytes() == data


@positive
@scenario("SCN-MEDIA-INGEST-002-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_present_root_is_examined_at_startup(sut: Sut) -> None:
    """The root exists at startup, so the scan registers the file under it without waiting."""
    sut.write(f"{MEDIA_DIRECTORY}/ready.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    report = wait_for_ingest(server, lambda reported: reported["registered"] == 1, "the scan registered the file")

    assert report["registered"] == 1, f"the scan registered nothing: {report}"
    assert file_at(server, token, "ready.png") is not None


@alternate_flow
@scenario("SCN-MEDIA-INGEST-002-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_root_that_appears_within_the_window_is_examined(sut: Sut) -> None:
    """A root that appears within the window is scanned, and no record is reconciled as removed for its absence."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    sut.write(f"{MEDIA_DIRECTORY}/late.png", data)
    os.utime(sut.dir / MEDIA_DIRECTORY / "late.png", (INSTANT, INSTANT))
    server = sut.start()
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    server.wait_until(
        lambda: file_at(server, token, "late.png") is not None,
        "the scan registered the file",
    )
    server.stop()
    shutil.rmtree(server.dir / MEDIA_DIRECTORY)
    server.start(config={"library": {"startup": {"media_root_wait_seconds": 10}}}, create_media=False)
    token = login(server, ROOT_ADMIN_USERNAME, password)
    sut.write(f"{MEDIA_DIRECTORY}/late.png", data)
    os.utime(sut.dir / MEDIA_DIRECTORY / "late.png", (INSTANT, INSTANT))

    report = wait_for_ingest(server, lambda reported: reported["removed"] == 0, "the waiting scan reported")

    assert report["untouched"] == 1, f"the scan did not examine the restored file: {report}"
    assert file_at(server, token, "late.png") is not None


@error_handling
@scenario("SCN-MEDIA-INGEST-002-C")
@scenario("SCN-MEDIA-INGEST-003-C")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_absent_required_root_refuses_startup(sut: Sut) -> None:
    """A root that never appears while it is required refuses startup and keeps the record it held."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    sut.write(f"{MEDIA_DIRECTORY}/kept.png", data)
    os.utime(sut.dir / MEDIA_DIRECTORY / "kept.png", (INSTANT, INSTANT))
    server = sut.start()
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    server.wait_until(
        lambda: file_at(server, token, "kept.png") is not None,
        "the scan registered the file",
    )
    server.stop()
    shutil.rmtree(server.dir / MEDIA_DIRECTORY)

    failure = server.start_expecting_failure(
        config={"library": {"startup": {"media_root_wait_seconds": 1, "fail_without_media_root": True}}},
        create_media=False,
    )

    assert failure.exit_code != 0, f"startup was not refused: {failure.exit_code}"
    assert MSG_MEDIA_ROOT_ABSENT in failure.output, f"the refusal does not name the root: {failure.output}"

    server.stop()
    sut.write(f"{MEDIA_DIRECTORY}/kept.png", data)
    os.utime(sut.dir / MEDIA_DIRECTORY / "kept.png", (INSTANT, INSTANT))
    server.start()
    token = login(server, ROOT_ADMIN_USERNAME, password)

    report = wait_for_ingest(server, lambda reported: reported["untouched"] == 1, "the restored scan reported")

    assert report["registered"] == 0, f"a refused startup stored a second record: {report}"
    assert file_at(server, token, "kept.png") is not None, "a refused startup reconciled the record as removed"


@alternate_flow
@scenario("SCN-MEDIA-INGEST-002-D")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_absent_optional_root_starts_and_reconciles_empty(sut: Sut) -> None:
    """A root that never appears while it is not required starts the server and reconciles every record as removed."""
    sut.write(f"{MEDIA_DIRECTORY}/gone.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    server.wait_until(
        lambda: file_at(server, token, "gone.png") is not None,
        "the scan registered the file",
    )
    server.stop()
    shutil.rmtree(server.dir / MEDIA_DIRECTORY)

    server.start(config={"library": {"startup": {"media_root_wait_seconds": 0}}}, create_media=False)
    token = login(server, ROOT_ADMIN_USERNAME, password)

    report = wait_for_ingest(server)

    assert report["removed"] == 1, f"the scan did not reconcile the record as removed: {report}"
    assert file_at(server, token, "gone.png") is None


@business_rule
@scenario("SCN-MEDIA-INGEST-003-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_request_during_the_scan_is_answered(sut: Sut) -> None:
    """A request reaching a server that scans a populated root is answered, and the scan still registers every file."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    for index in range(12):
        sut.write(f"{MEDIA_DIRECTORY}/photo-{index}.png", data)
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    index = get_api_index.sync_detailed(client=server.client(token))
    server.wait_until(
        lambda: all(file_at(server, token, f"photo-{position}.png") is not None for position in range(12)),
        "the scan registered every file",
    )

    assert index.status_code == 200
    assert json.loads(index.content)["_links"]["self"]["href"] == API_PREFIX


@business_rule
@scenario("SCN-MEDIA-INGEST-003-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_absent_optional_root_does_not_delay_readiness(sut: Sut) -> None:
    """The root is absent and not required, so the server is healthy while the reconciliation runs without blocking."""
    server = sut.start(config={"library": {"startup": {"media_root_wait_seconds": 3}}}, create_media=False)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    responded = get_api_index.sync_detailed(client=server.client(token))

    assert not server.exists(MEDIA_DIRECTORY), "readiness waited for a root it was not waiting for"
    assert responded.status_code == 200
    assert json.loads(responded.content)["_links"]["self"]["href"] == API_PREFIX
    assert wait_for_ingest(server)["removed"] == 0


@business_rule
@scenario("SCN-MEDIA-INGEST-004-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_the_content_decides_the_media_type(sut: Sut) -> None:
    """A file whose name suggests text is registered as the image its content is."""
    sut.write(f"{MEDIA_DIRECTORY}/photo.txt", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.wait_until(
        lambda: file_at(server, token, "photo.txt") is not None,
        "the scan registered the file",
    )

    record = file_at(server, token, "photo.txt")

    assert record is not None
    assert record["mime_type"] == IMAGE_CONTENT_TYPE
    assert "image" in record["_links"]


@business_rule
@scenario("SCN-MEDIA-INGEST-004-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_content_without_a_media_type_is_skipped(sut: Sut) -> None:
    """A file whose content yields no media type is skipped."""
    sut.write(f"{MEDIA_DIRECTORY}/opaque.bin", bytes(range(64)))
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    report = wait_for_ingest(server)

    assert report["skipped"] == 1, f"the file was not skipped: {report}"
    assert file_at(server, token, "opaque.bin") is None


@business_rule
@scenario("SCN-MEDIA-INGEST-005-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_file_that_is_not_media_is_skipped(sut: Sut) -> None:
    """A file whose content is not an image, a video or audio is skipped and the scan continues."""
    sut.write(f"{MEDIA_DIRECTORY}/document.pdf", PDF_BYTES)
    sut.write(f"{MEDIA_DIRECTORY}/photo.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    report = wait_for_ingest(server)

    assert file_at(server, token, "document.pdf") is None
    assert file_at(server, token, "photo.png") is not None, "the scan stopped at the unsuitable file"
    assert report["skipped"] == 1, f"the unsuitable file was not skipped: {report}"


@business_rule
@scenario("SCN-MEDIA-INGEST-005-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_unsupported_media_type_is_skipped(sut: Sut) -> None:
    """An audio file whose media type the server does not support is skipped while a supported one is registered."""
    sut.write(f"{MEDIA_DIRECTORY}/track.flac", (SEED_MEDIA_DIRECTORY / FLAC_PATH).read_bytes())
    sut.write(f"{MEDIA_DIRECTORY}/song.mp3", mp3_frame_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    report = wait_for_ingest(server)

    assert file_at(server, token, "track.flac") is None, "an unsupported media type was registered"
    assert file_at(server, token, "song.mp3") is not None, "the scan stopped at the unsupported type"
    assert report["skipped"] == 1, f"the unsupported type was not skipped: {report}"


@business_rule
@scenario("SCN-MEDIA-INGEST-005-C")
@pytest.mark.own_instance(reason="global_filesystem")
def test_unsuitable_files_do_not_abort_the_scan(sut: Sut) -> None:
    """Several unsuitable files among suitable ones leave every suitable file registered."""
    sut.write(f"{MEDIA_DIRECTORY}/notes.txt", b"plain notes\n")
    sut.write(f"{MEDIA_DIRECTORY}/document.pdf", PDF_BYTES)
    sut.write(f"{MEDIA_DIRECTORY}/photo.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    sut.write(f"{MEDIA_DIRECTORY}/clip.mp4", (SEED_MEDIA_DIRECTORY / VIDEO_PATH).read_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    report = wait_for_ingest(server)

    assert report["registered"] == 2, f"the scan did not register both suitable files: {report}"
    assert report["skipped"] == 2, f"the scan did not skip both unsuitable files: {report}"
    assert file_at(server, token, "photo.png") is not None
    assert file_at(server, token, "clip.mp4") is not None


@positive
@scenario("SCN-MEDIA-INGEST-006-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_image_is_registered_with_its_media_row(sut: Sut) -> None:
    """An accepted image records its file row and an image row carrying the probed dimensions and orientation."""
    sut.write(f"{MEDIA_DIRECTORY}/photo.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.wait_until(
        lambda: file_at(server, token, "photo.png") is not None,
        "the scan registered the file",
    )
    record = file_at(server, token, "photo.png")

    image_id = int(record["_links"]["image"]["href"].rsplit("/", 1)[1])
    response = get_image.sync_detailed(image_id, client=server.client(token))

    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    image = json.loads(response.content)
    assert (image["width_px"], image["height_px"]) == (64, 64)
    assert image["orientation"] == "SQUARE"
    assert image["name"] == "photo.png"


@positive
@scenario("SCN-MEDIA-INGEST-006-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_video_is_registered_with_its_media_row(sut: Sut) -> None:
    """An accepted video records its file row and a video row carrying the probed characteristics."""
    sut.write(f"{MEDIA_DIRECTORY}/clip.mp4", (SEED_MEDIA_DIRECTORY / VIDEO_PATH).read_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.wait_until(
        lambda: file_at(server, token, "clip.mp4") is not None,
        "the scan registered the file",
    )
    record = file_at(server, token, "clip.mp4")

    video_id = int(record["_links"]["video"]["href"].rsplit("/", 1)[1])
    response = get_video.sync_detailed(video_id, client=server.client(token))

    assert response.status_code == 200, f"video read failed: {response.status_code} {response.content!r}"
    video = json.loads(response.content)
    assert video["duration_ms"] > 0
    assert video["codec"]
    assert video["width"] > 0 and video["height"] > 0
    assert video["scan_type"] in {"PROGRESSIVE", "INTERLACED", "MBAFF", "PAFF"}


@positive
@scenario("SCN-MEDIA-INGEST-006-C")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_audio_file_is_registered_with_absent_measurements(sut: Sut) -> None:
    """An accepted audio file records the reported codec, sample rate and duration, and omits the block size."""
    sut.write(f"{MEDIA_DIRECTORY}/song.mp3", mp3_frame_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.wait_until(
        lambda: file_at(server, token, "song.mp3") is not None,
        "the scan registered the file",
    )
    record = file_at(server, token, "song.mp3")

    audio_id = int(record["_links"]["audio"]["href"].rsplit("/", 1)[1])
    response = get_audio.sync_detailed(audio_id, client=server.client(token))

    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    audio = json.loads(response.content)
    assert audio["codec"]
    assert audio["sample_rate_hz"] == 44_100
    assert audio["duration_ms"] > 0
    assert "block_size" not in audio, "a block size the content does not report was fabricated"


@positive
@scenario("SCN-MEDIA-INGEST-007-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_registered_file_carries_its_size_and_instant(sut: Sut) -> None:
    """A registered file records the size its content has and the instant it was last modified."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    sut.write(f"{MEDIA_DIRECTORY}/sized.png", data)
    os.utime(sut.dir / MEDIA_DIRECTORY / "sized.png", (INSTANT, INSTANT))
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.wait_until(
        lambda: file_at(server, token, "sized.png") is not None,
        "the scan registered the file",
    )

    record = file_at(server, token, "sized.png")

    assert record is not None
    assert record["size_bytes"] == len(data)
    assert record["modified_at"] == datetime.fromtimestamp(INSTANT, UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


@business_rule
@scenario("SCN-MEDIA-INGEST-008-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_found_file_belongs_to_the_root_admin(sut: Sut) -> None:
    """A file the scan found is attributed to the Root Admin and to no other account."""
    sut.write(f"{MEDIA_DIRECTORY}/owned.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.wait_until(
        lambda: file_at(server, token, "owned.png") is not None,
        "the scan registered the file",
    )
    record = file_at(server, token, "owned.png")

    current = get_me.sync_detailed(client=server.client(token))

    assert current.status_code == 200
    profile = json.loads(current.content)
    assert profile["id"] == 0
    assert record["_links"]["owner"]["href"] == profile["_links"]["self"]["href"]


@business_rule
@scenario("SCN-MEDIA-INGEST-009-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_unchanged_file_is_reported_untouched(sut: Sut) -> None:
    """A registered file whose size and instant match its record is reported untouched and nothing is written."""
    sut.write(f"{MEDIA_DIRECTORY}/stable.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    server.wait_until(
        lambda: file_at(server, token, "stable.png") is not None,
        "the scan registered the file",
    )
    before = file_at(server, token, "stable.png")
    server.stop()

    server.start()
    token = login(server, ROOT_ADMIN_USERNAME, password)

    report = wait_for_ingest(server, lambda reported: reported["untouched"] == 1, "the scan reported the file")

    assert report["untouched"] == 1, f"the unchanged file was not reported untouched: {report}"
    assert report["registered"] == 0
    assert file_at(server, token, "stable.png") == before


@business_rule
@scenario("SCN-MEDIA-INGEST-009-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_changed_file_is_reported_and_left_alone(sut: Sut) -> None:
    """A registered file whose size differs from its record is reported changed and its record is left as it was."""
    sut.write(f"{MEDIA_DIRECTORY}/drifted.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    server.wait_until(
        lambda: file_at(server, token, "drifted.png") is not None,
        "the scan registered the file",
    )
    before = file_at(server, token, "drifted.png")
    server.stop()
    with (server.dir / MEDIA_DIRECTORY / "drifted.png").open("ab") as content:
        content.write(b"more bytes")

    server.start()
    token = login(server, ROOT_ADMIN_USERNAME, password)

    report = wait_for_ingest(server, lambda reported: reported["changed"] == 1, "the scan reported the change")

    assert report["changed"] == 1, f"the changed file was not reported changed: {report}"
    assert report["registered"] == 0
    assert file_at(server, token, "drifted.png") == before


@business_rule
@scenario("SCN-MEDIA-INGEST-009-C")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_vanished_file_is_reconciled_as_removed(sut: Sut) -> None:
    """A registered file the scan no longer finds is reconciled as removed, not as untouched or changed."""
    sut.write(f"{MEDIA_DIRECTORY}/vanished.png", (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes())
    server = sut.start()
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    server.wait_until(
        lambda: file_at(server, token, "vanished.png") is not None,
        "the scan registered the file",
    )
    server.stop()
    (server.dir / MEDIA_DIRECTORY / "vanished.png").unlink()

    server.start()
    token = login(server, ROOT_ADMIN_USERNAME, password)

    report = wait_for_ingest(server, lambda reported: reported["removed"] == 1, "the scan reported the removal")

    assert report["removed"] == 1, f"the vanished file was not reconciled as removed: {report}"
    assert report["untouched"] == 0 and report["changed"] == 0
    assert file_at(server, token, "vanished.png") is None


@business_rule
@scenario("SCN-MEDIA-INGEST-011-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_the_ingest_leaves_the_content_byte_identical(sut: Sut) -> None:
    """Every file under the root is byte-identical after the scan, and the scan added, moved and removed none."""
    files = {
        "photo.png": (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes(),
        "clip.mp4": (SEED_MEDIA_DIRECTORY / VIDEO_PATH).read_bytes(),
        "unsorted.bin": bytes(range(32)),
    }
    for name, content in files.items():
        sut.write(f"{MEDIA_DIRECTORY}/{name}", content)
    server = sut.start()

    report = wait_for_ingest(server, lambda reported: reported["registered"] == 2, "the scan registered both files")

    assert report["registered"] == 2
    for name, content in files.items():
        assert (server.dir / MEDIA_DIRECTORY / name).read_bytes() == content, f"{name} was not left byte-identical"
    assert sorted(os.listdir(server.dir / MEDIA_DIRECTORY)) == sorted(files)


@error_handling
@scenario("SCN-MEDIA-INGEST-011-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_unreadable_file_does_not_damage_the_ingest(sut: Sut) -> None:
    """A file the ingest cannot read is skipped without being altered, and the remaining files are still registered."""
    image = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    sut.write(f"{MEDIA_DIRECTORY}/readable.png", image)
    sut.write(f"{MEDIA_DIRECTORY}/unreadable.png", image)
    unreadable = sut.dir / MEDIA_DIRECTORY / "unreadable.png"
    unreadable.chmod(0o000)
    try:
        server = sut.start()
        token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

        report = wait_for_ingest(server)

        assert file_at(server, token, "readable.png") is not None, "the unreadable file stopped the scan"
        assert file_at(server, token, "unreadable.png") is None
        assert unreadable.stat().st_size == len(image), "the unreadable file was altered"
        assert report["skipped"] == 1, f"the unreadable file was not skipped: {report}"
    finally:
        unreadable.chmod(0o600)


@business_rule
@scenario("SCN-MEDIA-INGEST-012-A")
@scenario("SCN-MEDIA-INGEST-012-B")
def test_removing_a_gallery_leaves_the_content_alone(server: Sut, root_token: str) -> None:
    """Removing a gallery item and removing a whole gallery each leave the stored bytes and the file record."""
    kept_bytes = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    dropped_bytes = (SEED_MEDIA_DIRECTORY / "gallery-media.png").read_bytes()
    removed = upload(server, root_token, f"kept{next(_ACCOUNTS)}.png", kept_bytes)
    discarded = upload(server, root_token, f"discarded{next(_ACCOUNTS)}.png", dropped_bytes)
    gallery = create_gallery(server, root_token, f"kept{next(_ACCOUNTS)}")
    item = add_to_gallery(server, root_token, gallery, removed["id"])
    second = create_gallery(server, root_token, f"dropped{next(_ACCOUNTS)}")
    add_to_gallery(server, root_token, second, discarded["id"])

    client = server.client(root_token)
    item_id = int(item["_links"]["self"]["href"].rsplit("/", 1)[1])
    removed_item = delete_gallery_item.sync_detailed(gallery["id"], item_id, client=client)
    removed_gallery = delete_gallery.sync_detailed(second["id"], client=client)

    assert removed_item.status_code == 204, f"item removal failed: {removed_item.status_code}"
    assert removed_gallery.status_code == 204, f"gallery removal failed: {removed_gallery.status_code}"
    assert (server.dir / MEDIA_DIRECTORY / removed["path"]).read_bytes() == kept_bytes, (
        "an item removal deleted the stored content"
    )
    assert (server.dir / MEDIA_DIRECTORY / discarded["path"]).read_bytes() == dropped_bytes, (
        "a gallery removal deleted the stored content"
    )


@business_rule
@scenario("SCN-MEDIA-INGEST-012-A")
def test_content_removed_with_its_gallery_reference_can_be_uploaded_again(server: Sut, root_token: str) -> None:
    """A removed reference leaves no orphaned file record that a later upload would dedup to."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    first = upload(server, root_token, f"again{next(_ACCOUNTS)}.png", data)
    gallery = create_gallery(server, root_token, f"again{next(_ACCOUNTS)}")
    item = add_to_gallery(server, root_token, gallery, first["id"])
    item_id = int(item["_links"]["self"]["href"].rsplit("/", 1)[1])

    removed = delete_gallery_item.sync_detailed(gallery["id"], item_id, client=server.client(root_token))
    assert removed.status_code == 204, f"item removal failed: {removed.status_code}"

    # The upload helper reads the completed file, so a dedup that returned the
    # removed row without its resource would fail here instead of completing.
    second = upload(server, root_token, f"again{next(_ACCOUNTS)}.png", data)

    response = get_file.sync_detailed(second["id"], client=server.client(root_token))
    assert response.status_code == 200, f"the re-uploaded file is not readable: {response.status_code}"


@business_rule
@scenario("SCN-MEDIA-INGEST-014-A")
def test_a_renamed_file_keeps_its_record(server: Sut, root_token: str) -> None:
    """A file renamed while the server runs follows its content, keeping the record and the media row it carried."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    before_path = f"before{next(_ACCOUNTS)}.png"
    after_path = before_path.replace("before", "after")
    (server.dir / MEDIA_DIRECTORY / before_path).write_bytes(data)
    server.wait_until(
        lambda: file_at(server, root_token, before_path) is not None,
        "the observer registered the file",
    )
    before = file_at(server, root_token, before_path)
    image_id = int(before["_links"]["image"]["href"].rsplit("/", 1)[1])
    response = get_image.sync_detailed(image_id, client=server.client(root_token))
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    image = json.loads(response.content)

    (server.dir / MEDIA_DIRECTORY / before_path).rename(server.dir / MEDIA_DIRECTORY / after_path)
    server.wait_until(
        lambda: file_at(server, root_token, after_path) is not None,
        "the observer registered the renamed file",
    )

    after = file_at(server, root_token, after_path)

    assert after["id"] == before["id"], "the rename stored a second record"
    assert after["path"] == after_path, "the recorded location did not follow the file"
    assert after["size_bytes"] == len(data)
    assert file_at(server, root_token, before_path) is None, "the previous location kept its record"
    preserved = get_image.sync_detailed(image_id, client=server.client(root_token))
    assert preserved.status_code == 200, f"image read failed: {preserved.status_code} {preserved.content!r}"
    assert json.loads(preserved.content)["id"] == image["id"], "the rename replaced the media row"


@business_rule
@scenario("SCN-MEDIA-INGEST-013-A")
def test_a_file_added_while_the_server_runs_is_registered(server: Sut, root_token: str) -> None:
    """A file added under the root while the server runs is registered without a restart."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    name = f"added{next(_ACCOUNTS)}.png"
    (server.dir / MEDIA_DIRECTORY / name).write_bytes(data)

    server.wait_until(
        lambda: file_at(server, root_token, name) is not None,
        "the observer registered the added file",
    )

    record = file_at(server, root_token, name)

    assert record["mime_type"] == IMAGE_CONTENT_TYPE
    assert record["origin"] == "SCANNED"


@business_rule
@scenario("SCN-MEDIA-INGEST-013-B")
def test_a_file_still_being_written_is_registered_once_complete(server: Sut, root_token: str) -> None:
    """A file whose content is still changing is not registered until it stops, then with the size it finally has."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    name = f"growing{next(_ACCOUNTS)}.png"
    with (server.dir / MEDIA_DIRECTORY / name).open("wb") as content:
        for offset in range(0, len(data), 10):
            content.write(data[offset : offset + 10])
            content.flush()
            time.sleep(0.05)
            if offset == 20:
                assert file_at(server, root_token, name) is None, "an unfinished file was registered"

    server.wait_until(
        lambda: file_at(server, root_token, name) is not None,
        "the observer registered the finished file",
    )

    record = file_at(server, root_token, name)

    assert record["size_bytes"] == len(data), "the record does not carry the size the file finally has"


@business_rule
@scenario("SCN-MEDIA-INGEST-015-A")
def test_a_file_removed_while_the_server_runs_is_reconciled(server: Sut, root_token: str) -> None:
    """A registered file removed while the server runs has its record and its media row removed."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    name = f"leaving{next(_ACCOUNTS)}.png"
    (server.dir / MEDIA_DIRECTORY / name).write_bytes(data)
    server.wait_until(
        lambda: file_at(server, root_token, name) is not None,
        "the observer registered the file",
    )
    record = file_at(server, root_token, name)
    image_id = int(record["_links"]["image"]["href"].rsplit("/", 1)[1])

    (server.dir / MEDIA_DIRECTORY / name).unlink()
    server.wait_until(
        lambda: file_at(server, root_token, name) is None,
        "the observer reconciled the removed file",
    )

    gone = get_image.sync_detailed(image_id, client=server.client(root_token))
    assert gone.status_code == 404, "the media row survived its file"


@business_rule
@scenario("SCN-MEDIA-INGEST-015-C")
def test_a_reappearing_file_is_a_new_record(server: Sut, root_token: str) -> None:
    """A file removed and then added again is a new record, not the restored one."""
    data = (SEED_MEDIA_DIRECTORY / IMAGE_PATH).read_bytes()
    cycled_path = f"cycled{next(_ACCOUNTS)}.png"
    survivor_path = f"survivor{next(_ACCOUNTS)}.png"
    (server.dir / MEDIA_DIRECTORY / cycled_path).write_bytes(data)
    (server.dir / MEDIA_DIRECTORY / survivor_path).write_bytes(data)
    server.wait_until(
        lambda: (
            file_at(server, root_token, cycled_path) is not None
            and file_at(server, root_token, survivor_path) is not None
        ),
        "the observer registered both files",
    )
    removed = file_at(server, root_token, cycled_path)
    survivor = file_at(server, root_token, survivor_path)

    (server.dir / MEDIA_DIRECTORY / cycled_path).unlink()
    server.wait_until(
        lambda: file_at(server, root_token, cycled_path) is None,
        "the observer reconciled the removal",
    )
    (server.dir / MEDIA_DIRECTORY / cycled_path).write_bytes(data)
    server.wait_until(
        lambda: file_at(server, root_token, cycled_path) is not None,
        "the observer registered the reappearing file",
    )

    returned = file_at(server, root_token, cycled_path)

    assert returned["id"] != removed["id"], "the removed record was restored"
    assert file_at(server, root_token, survivor_path)["id"] == survivor["id"]
