"""Black-box proof of the `file` capability."""

import json
import os
import time
from datetime import UTC, datetime

import pytest
from conftest import SEED_MEDIA_DIRECTORY, login, start_instance, upload
from harness import MEDIA_DIRECTORY, ROOT_ADMIN_USERNAME, Sut
from labels import TestType, business_rule, positive, scenario
from mnemorium_client.api.asset import list_files

LINKED_PATH = "system/file-linked.png"
AUDIO_PATH = "system/file-audio.flac"
VIDEO_PATH = "system/file-video.mp4"
BARE_PATH = "system/file-bare.png"

IMAGE_CONTENT_TYPE = "image/png"

FILE_FIELDS = {
    "id",
    "is_public",
    "mime_type",
    "modified_at",
    "origin",
    "path",
    "size_bytes",
    "uploaded_date",
    "_links",
}

FOUND_PATH = "found.png"
FOUND_INSTANT = 1_700_000_000
WAIT_SECONDS = 20.0


@positive
@scenario("SCN-FILE-001-A")
def test_a_promoted_upload_is_an_uploaded_file(server: Sut, root_token: str) -> None:
    """Content the server received records the `UPLOADED` origin at a root-relative path."""
    data = (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes()

    file = upload(server, root_token, "promoted.png", data, IMAGE_CONTENT_TYPE)

    assert file["origin"] == "UPLOADED"
    assert not file["path"].startswith("/"), "a stored path must be root-relative"
    assert (server.dir / MEDIA_DIRECTORY / file["path"]).read_bytes() == data


@positive
@scenario("SCN-FILE-001-B")
@scenario("SCN-FILE-001-C")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_found_file_records_its_content(sut: Sut) -> None:
    """Content the scan found records the `SCANNED` origin and the content's measurements."""
    data = (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes()
    fixture = sut.dir / MEDIA_DIRECTORY / FOUND_PATH
    fixture.parent.mkdir(parents=True, exist_ok=True)
    fixture.write_bytes(data)
    os.utime(fixture, (FOUND_INSTANT, FOUND_INSTANT))
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    deadline = time.monotonic() + WAIT_SECONDS
    items = []
    while time.monotonic() < deadline:
        response = list_files.sync_detailed(client=server.client(root_token), path=FOUND_PATH)
        assert response.status_code == 200, f"list failed: {response.status_code} {response.content!r}"
        items = json.loads(response.content)["items"]
        if items:
            break
        time.sleep(0.1)
    assert items, f"the scan registered no record for {FOUND_PATH!r}"

    file = items[0]
    assert file["origin"] == "SCANNED"
    assert file["path"] == FOUND_PATH
    assert file["mime_type"] == IMAGE_CONTENT_TYPE
    assert file["size_bytes"] == len(data)
    assert file["modified_at"] == datetime.fromtimestamp(FOUND_INSTANT, UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
    assert file["uploaded_date"] == datetime.now(UTC).date().isoformat()


@positive
@scenario("SCN-FILE-002-A")
def test_a_file_of_an_existing_identifier_is_read(server: Sut, root_token: str) -> None:
    """The representation carries exactly the record's fields and the stored values."""
    listed = list_files.sync_detailed(client=server.client(root_token), path=LINKED_PATH)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
    items = json.loads(listed.content)["items"]
    assert len(items) == 1, f"expected one file at {LINKED_PATH}, got {len(items)}"

    response = server.request("GET", items[0]["_links"]["self"]["href"], token=root_token)

    assert response.status_code == 200, f"file read failed: {response.status_code} {response.content!r}"
    file = json.loads(response.content)
    assert set(file) == FILE_FIELDS
    assert file["id"] == items[0]["id"]
    assert file["path"] == LINKED_PATH
    assert file["mime_type"] == IMAGE_CONTENT_TYPE
    assert file["size_bytes"] == (SEED_MEDIA_DIRECTORY / "file-linked.png").stat().st_size
    assert file["origin"] == "SCANNED"
    assert file["is_public"] is True
    assert file["uploaded_date"] == datetime.now(UTC).date().isoformat()
    assert datetime.fromisoformat(file["modified_at"]).tzinfo is UTC


@scenario("SCN-FILE-003-A", TestType.BUSINESS_RULE)
@scenario("SCN-FILE-004-A", TestType.POSITIVE)
@scenario("SCN-FILE-004-B", TestType.BUSINESS_RULE)
@scenario("SCN-API-005-A", TestType.BUSINESS_RULE)
def test_the_collection_lists_files_and_links_them(server: Sut, root_token: str) -> None:
    """The collection lists a discoverable file, the exact path narrows it, and the item is the read representation."""
    everything = list_files.sync_detailed(client=server.client(root_token), limit=100)
    assert everything.status_code == 200, f"list failed: {everything.status_code} {everything.content!r}"
    all_ids = [item["id"] for item in json.loads(everything.content)["items"]]

    filtered = list_files.sync_detailed(client=server.client(root_token), path=LINKED_PATH)
    assert filtered.status_code == 200, f"filter failed: {filtered.status_code} {filtered.content!r}"
    items = json.loads(filtered.content)["items"]
    assert len(items) == 1, f"expected one file at {LINKED_PATH}, got {len(items)}"
    item = items[0]

    assert item["id"] in all_ids
    owner = server.request("GET", item["_links"]["owner"]["href"], token=root_token)
    content = server.request("GET", item["_links"]["content"]["href"], token=root_token)
    read = server.request("GET", item["_links"]["self"]["href"], token=root_token)

    assert owner.status_code == 200, f"owner read failed: {owner.status_code} {owner.content!r}"
    assert json.loads(owner.content)["id"] == 0, "a seeded library file is the Root Admin's"
    assert item["_links"]["content"]["href"] == f"{item['_links']['self']['href']}/content"
    assert content.status_code == 200, f"content read failed: {content.status_code} {content.content!r}"
    assert content.content == (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes()
    assert read.status_code == 200, f"file read failed: {read.status_code} {read.content!r}"
    assert json.loads(read.content) == item, "a collection item must be the single-file representation"


@scenario("SCN-FILE-003-B", TestType.BUSINESS_RULE)
@scenario("SCN-API-005-A", TestType.BUSINESS_RULE)
def test_an_image_backed_file_links_its_image(server: Sut, root_token: str) -> None:
    """A file with an image row carries `_links.image` naming that image."""
    listed = list_files.sync_detailed(client=server.client(root_token), path=LINKED_PATH)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
    item = json.loads(listed.content)["items"][0]

    response = server.request("GET", item["_links"]["image"]["href"], token=root_token)

    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    image = json.loads(response.content)
    assert image["file_id"] == item["id"]
    assert image["_links"]["file"]["href"] == item["_links"]["self"]["href"]


@business_rule
@scenario("SCN-FILE-003-C")
def test_an_audio_backed_file_links_its_audio(server: Sut, root_token: str) -> None:
    """A file with an audio row carries `_links.audio` naming that audio."""
    listed = list_files.sync_detailed(client=server.client(root_token), path=AUDIO_PATH)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
    item = json.loads(listed.content)["items"][0]

    response = server.request("GET", item["_links"]["audio"]["href"], token=root_token)

    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    audio = json.loads(response.content)
    assert audio["file_id"] == item["id"]
    assert audio["_links"]["file"]["href"] == item["_links"]["self"]["href"]


@business_rule
@scenario("SCN-FILE-003-C")
def test_a_video_backed_file_links_its_video(server: Sut, root_token: str) -> None:
    """A file with a video row carries `_links.video` naming that video."""
    listed = list_files.sync_detailed(client=server.client(root_token), path=VIDEO_PATH)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
    item = json.loads(listed.content)["items"][0]

    response = server.request("GET", item["_links"]["video"]["href"], token=root_token)

    assert response.status_code == 200, f"video read failed: {response.status_code} {response.content!r}"
    video = json.loads(response.content)
    assert video["file_id"] == item["id"]
    assert video["_links"]["file"]["href"] == item["_links"]["self"]["href"]


@business_rule
@scenario("SCN-FILE-003-D")
def test_a_file_with_no_media_row_carries_no_typed_link(server: Sut, root_token: str) -> None:
    """A file that backs no typed asset carries none of `image`, `audio` and `video`."""
    listed = list_files.sync_detailed(client=server.client(root_token), path=BARE_PATH)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
    items = json.loads(listed.content)["items"]
    assert len(items) == 1, f"expected one file at {BARE_PATH}, got {len(items)}"

    links = items[0]["_links"]
    assert set(links).isdisjoint({"image", "audio", "video"}), "a bare file carried a typed link"
    assert set(links) == {"content", "owner", "self"}


@positive
@scenario("SCN-FILE-004-C")
def test_a_path_no_file_carries_lists_nothing(server: Sut, root_token: str) -> None:
    """A path that only starts like a stored path lists no file."""
    response = list_files.sync_detailed(client=server.client(root_token), path="system/file-linked")

    assert response.status_code == 200, f"list failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["items"] == []
