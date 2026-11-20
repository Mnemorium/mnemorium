"""Black-box proof of the `image` capability."""

import json
import struct
import zlib
from datetime import UTC, datetime
from itertools import count

import allure
from conftest import SEED_MEDIA_DIRECTORY, upload
from harness import Sut

IMAGE_CONTENT_TYPE = "image/png"
AUDIO_CONTENT_TYPE = "audio/flac"
VIDEO_CONTENT_TYPE = "video/mp4"

LANDSCAPE = (32, 16)
PORTRAIT = (16, 32)
SQUARE = (16, 16)

# Every generated image carries a per-call colour, so a repeated byte string is not the first one's file.
_IMAGE_NONCE = count(datetime.now(UTC).toordinal())


def _png_chunk(chunk_type: bytes, data: bytes) -> bytes:
    """Build one PNG chunk with its CRC."""
    crc = zlib.crc32(chunk_type + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + chunk_type + data + struct.pack(">I", crc)


def png(width: int, height: int) -> bytes:
    """Build a minimal valid, unique RGB PNG of the given dimensions."""
    nonce = next(_IMAGE_NONCE)
    colour = bytes([nonce & 0xFF, (nonce >> 8) & 0xFF, (nonce >> 16) & 0xFF])
    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    scanline = b"\x00" + colour * width
    pixels = zlib.compress(scanline * height)
    return b"\x89PNG\r\n\x1a\n" + _png_chunk(b"IHDR", header) + _png_chunk(b"IDAT", pixels) + _png_chunk(b"IEND", b"")


@allure.epic("Image")
@allure.feature("REQ-IMAGE-001")
@allure.story("SCN-IMAGE-001-A")
def test_an_image_file_creates_one_image(server: Sut, root_token: str) -> None:
    """An image upload leaves one image backed by the uploaded file."""
    file = upload(server, root_token, "screen.png", png(*SQUARE), IMAGE_CONTENT_TYPE)

    response = server.follow(file, "image", token=root_token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    image = json.loads(response.content)

    assert image["file_id"] == file["id"]
    assert file["_links"]["image"]["href"].endswith(f"/{image['id']}")


@allure.epic("Image")
@allure.feature("REQ-IMAGE-001")
@allure.story("SCN-IMAGE-001-B")
def test_a_file_backs_one_image(server: Sut, root_token: str) -> None:
    """Uploading the same content twice reuses the stored file and its one image."""
    data = png(*SQUARE)
    first = upload(server, root_token, "screen.png", data, IMAGE_CONTENT_TYPE)
    first_response = server.follow(first, "image", token=root_token)
    assert first_response.status_code == 200, (
        f"image read failed: {first_response.status_code} {first_response.content!r}"
    )
    first_image = json.loads(first_response.content)

    second = upload(server, root_token, "screen.png", data, IMAGE_CONTENT_TYPE)
    second_response = server.follow(second, "image", token=root_token)
    assert second_response.status_code == 200, (
        f"image read failed: {second_response.status_code} {second_response.content!r}"
    )
    second_image = json.loads(second_response.content)

    assert second["id"] == first["id"], "the same content must reuse its stored file"
    assert second_image["id"] == first_image["id"], "the second registration created a second image"


@allure.epic("Image")
@allure.feature("REQ-IMAGE-001")
@allure.story("SCN-IMAGE-001-C")
def test_audio_and_video_content_create_no_image(server: Sut, root_token: str) -> None:
    """Audio and video uploads leave the file without an image."""
    audio = upload(
        server,
        root_token,
        "track.flac",
        (SEED_MEDIA_DIRECTORY / "track.flac").read_bytes(),
        AUDIO_CONTENT_TYPE,
    )
    video = upload(
        server,
        root_token,
        "clip.mp4",
        (SEED_MEDIA_DIRECTORY / "episode.mp4").read_bytes(),
        VIDEO_CONTENT_TYPE,
    )

    assert "image" not in audio["_links"]
    assert "image" not in video["_links"]


@allure.epic("Image")
@allure.feature("REQ-IMAGE-002")
@allure.story("SCN-IMAGE-002-A")
def test_a_wider_image_is_landscape(server: Sut, root_token: str) -> None:
    """Width exceeding height reports `LANDSCAPE`."""
    file = upload(server, root_token, "wide.png", png(*LANDSCAPE), IMAGE_CONTENT_TYPE)

    response = server.follow(file, "image", token=root_token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    image = json.loads(response.content)

    assert (image["width_px"], image["height_px"]) == LANDSCAPE
    assert image["orientation"] == "LANDSCAPE"


@allure.epic("Image")
@allure.feature("REQ-IMAGE-002")
@allure.story("SCN-IMAGE-002-B")
def test_a_taller_image_is_portrait(server: Sut, root_token: str) -> None:
    """Height exceeding width reports `PORTRAIT`."""
    file = upload(server, root_token, "tall.png", png(*PORTRAIT), IMAGE_CONTENT_TYPE)

    response = server.follow(file, "image", token=root_token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    image = json.loads(response.content)

    assert (image["width_px"], image["height_px"]) == PORTRAIT
    assert image["orientation"] == "PORTRAIT"


@allure.epic("Image")
@allure.feature("REQ-IMAGE-002")
@allure.story("SCN-IMAGE-002-C")
def test_a_square_image_is_square(server: Sut, root_token: str) -> None:
    """Equal width and height report `SQUARE`."""
    file = upload(server, root_token, "square.png", png(*SQUARE), IMAGE_CONTENT_TYPE)

    response = server.follow(file, "image", token=root_token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    image = json.loads(response.content)

    assert (image["width_px"], image["height_px"]) == SQUARE
    assert image["orientation"] == "SQUARE"


@allure.epic("Image")
@allure.feature("REQ-IMAGE-002")
@allure.story("SCN-IMAGE-002-D")
def test_the_name_and_creation_date_are_reported(server: Sut, root_token: str) -> None:
    """The name is the one the intake assigned and the creation date is the registration date."""
    registered_on = datetime.now(UTC).date().isoformat()
    file = upload(server, root_token, "holiday.png", png(*SQUARE), IMAGE_CONTENT_TYPE)

    response = server.follow(file, "image", token=root_token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.content!r}"
    image = json.loads(response.content)

    assert image["name"] == "holiday.png"
    assert image["created_date"] == registered_on
