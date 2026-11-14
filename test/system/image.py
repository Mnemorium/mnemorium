"""Black-box proof of the `image` capability.

An image is the typed row an uploaded or found image file backs. This module proves what the row reports — the name the
intake assigned, the dimensions, the orientation derived from them, the creation date and the backing file — and how a
caller reads it: the typed representation, its links, its file alias, and the `404` an unknown image answers.

The declarative contract — the media types, the status of each operation, the representation's fields and its
`_links` — is validated by Schemathesis from the served document. These cases prove only what a schema cannot: that the
orientation follows the dimensions rather than the declaration, that a file backs at most one image, that another media
family creates no image, and that following a representation's links reaches the resource it names. Every route comes
from the file's `_links.image` or from the typed representation's own links, so the suite keeps its single `/api/v1`
base literal in `harness.Sut.index`.
"""

import struct
import zlib
from datetime import UTC, datetime
from itertools import count

import allure
from conftest import FIXTURE_DIRECTORY, linked_id, upload
from harness import Sut, link_of

# The three dimensions that classify an image, each distinct so its bytes are distinct too: the upload flow
# deduplicates by integrity hash, so a repeated byte string would reuse the file and its image.
LANDSCAPE = (32, 16)
PORTRAIT = (16, 32)
SQUARE = (16, 16)

# The orientation the system derives from each pair of dimensions.
ORIENTATIONS = {
    LANDSCAPE: "LANDSCAPE",
    PORTRAIT: "PORTRAIT",
    SQUARE: "SQUARE",
}

# Every generated image carries a per-call colour, so a second `SQUARE` upload is not the first one's file.
_IMAGE_NONCE = count(datetime.now(UTC).toordinal())

AUDIO_CONTENT_TYPE = "audio/flac"
VIDEO_CONTENT_TYPE = "video/mp4"


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


def upload_image(server: Sut, token: str, name: str, dimensions: tuple[int, int]) -> dict:
    """Upload one image of the given dimensions and return the file representation."""
    width, height = dimensions
    return upload(server, token, name, png(width, height), "image/png")


def typed_image(server: Sut, token: str, file: dict) -> dict:
    """Follow a file's `image` link and return the typed representation."""
    response = server.follow(file, "image", token=token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.text}"
    return response.json()


def unknown_image(server: Sut, token: str, image: dict):
    """Request the image one identifier past `image`, which the instance has never created."""
    href = link_of(image, "self")
    base, _, _ = href.rpartition("/")
    return server.request("GET", f"{base}/{linked_id(href) + 1}", token=token)


@allure.epic("Image")
@allure.feature("REQ-IMAGE-001")
@allure.story("SCN-IMAGE-001-A")
def test_an_image_file_creates_one_image(server: Sut, root_token: str) -> None:
    """An image upload leaves exactly one image backing the uploaded file."""
    file = upload_image(server, root_token, "screen.png", SQUARE)

    image = typed_image(server, root_token, file)

    assert image["file_id"] == file["id"]
    assert link_of(file, "image").endswith(f"/{image['id']}")
    assert unknown_image(server, root_token, image).status_code == 404


@allure.epic("Image")
@allure.feature("REQ-IMAGE-001")
@allure.story("SCN-IMAGE-001-B")
def test_a_file_backs_one_image(server: Sut, root_token: str) -> None:
    """Registering the same stored file again creates no second image for it."""
    data = png(*SQUARE)
    first = upload(server, root_token, "screen.png", data, "image/png")
    before = typed_image(server, root_token, first)

    again = upload(server, root_token, "screen.png", data, "image/png")
    after = typed_image(server, root_token, again)

    assert again["id"] == first["id"], "the same content must reuse its stored file"
    assert after["id"] == before["id"], "the second registration created a second image"
    assert unknown_image(server, root_token, after).status_code == 404


@allure.epic("Image")
@allure.feature("REQ-IMAGE-001")
@allure.story("SCN-IMAGE-001-C")
def test_audio_and_video_content_create_no_image(server: Sut, root_token: str) -> None:
    """Audio and video uploads leave the file without an image."""
    audio = upload(
        server,
        root_token,
        "track.flac",
        (FIXTURE_DIRECTORY / "track.flac").read_bytes(),
        AUDIO_CONTENT_TYPE,
    )
    video = upload(
        server,
        root_token,
        "clip.mp4",
        (FIXTURE_DIRECTORY / "clip.mp4").read_bytes(),
        VIDEO_CONTENT_TYPE,
    )

    assert "image" not in audio["_links"]
    assert "image" not in video["_links"]


@allure.epic("Image")
@allure.feature("REQ-IMAGE-002")
@allure.story("SCN-IMAGE-002-A")
def test_a_wider_image_is_landscape(server: Sut, root_token: str) -> None:
    """Width exceeding height reports `LANDSCAPE`."""
    image = typed_image(server, root_token, upload_image(server, root_token, "wide.png", LANDSCAPE))

    assert (image["width_px"], image["height_px"]) == LANDSCAPE
    assert image["orientation"] == ORIENTATIONS[LANDSCAPE]


@allure.epic("Image")
@allure.feature("REQ-IMAGE-002")
@allure.story("SCN-IMAGE-002-B")
def test_a_taller_image_is_portrait(server: Sut, root_token: str) -> None:
    """Height exceeding width reports `PORTRAIT`."""
    image = typed_image(server, root_token, upload_image(server, root_token, "tall.png", PORTRAIT))

    assert (image["width_px"], image["height_px"]) == PORTRAIT
    assert image["orientation"] == ORIENTATIONS[PORTRAIT]


@allure.epic("Image")
@allure.feature("REQ-IMAGE-002")
@allure.story("SCN-IMAGE-002-C")
def test_a_square_image_is_square(server: Sut, root_token: str) -> None:
    """Equal width and height report `SQUARE`."""
    image = typed_image(server, root_token, upload_image(server, root_token, "square.png", SQUARE))

    assert (image["width_px"], image["height_px"]) == SQUARE
    assert image["orientation"] == ORIENTATIONS[SQUARE]


@allure.epic("Image")
@allure.feature("REQ-IMAGE-002")
@allure.story("SCN-IMAGE-002-D")
def test_the_name_and_creation_date_are_reported(server: Sut, root_token: str) -> None:
    """The name is the one the intake assigned and the creation date is the registration date."""
    registered_on = datetime.now(UTC).date().isoformat()

    image = typed_image(server, root_token, upload_image(server, root_token, "holiday.png", SQUARE))

    assert image["name"] == "holiday.png"
    assert image["created_at"] == registered_on


@allure.epic("Image")
@allure.feature("REQ-IMAGE-003")
@allure.story("SCN-IMAGE-003-A")
def test_an_image_carries_its_representation(server: Sut, root_token: str) -> None:
    """The typed representation carries the image's fields and links its own resource, file and content."""
    file = upload_image(server, root_token, "poster.png", LANDSCAPE)
    typed_href = link_of(file, "image")

    image = typed_image(server, root_token, file)

    assert set(image) == {"created_at", "file_id", "height_px", "id", "name", "orientation", "width_px", "_links"}
    assert image["name"] == "poster.png"
    assert link_of(image, "self") == typed_href
    assert link_of(image, "file") == link_of(file, "self")
    assert link_of(image, "content") == f"{typed_href}/content"


@allure.epic("Image")
@allure.feature("REQ-IMAGE-003")
@allure.story("SCN-IMAGE-003-B")
def test_an_unknown_image_is_not_found(server: Sut, root_token: str) -> None:
    """An image the instance never created answers `404`, like one the caller may not read."""
    image = typed_image(server, root_token, upload_image(server, root_token, "gone.png", SQUARE))

    response = unknown_image(server, root_token, image)

    assert response.status_code == 404, f"unknown image answered {response.status_code}: {response.text}"


@allure.epic("Image")
@allure.feature("REQ-IMAGE-004")
@allure.story("SCN-IMAGE-004-A")
def test_the_file_alias_redirects_to_the_file(server: Sut, root_token: str) -> None:
    """The typed file alias answers `303` with the backing file resource in `Location`."""
    file = upload_image(server, root_token, "alias.png", SQUARE)
    typed_href = link_of(file, "image")

    response = server.request("GET", f"{typed_href}/file", token=root_token, allow_redirects=False)

    assert response.status_code == 303, f"alias answered {response.status_code}: {response.text}"
    assert response.headers["Location"] == link_of(file, "self")
