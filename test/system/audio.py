"""Black-box proof of the `audio` capability.

An audio is the typed row an uploaded or found audio file backs. This module proves what the row reports — the name the
intake assigned, the codec, and the measurements the content reports or leaves out — and how a caller reads it: the
typed representation, its links, its file alias, and the `404` an unknown audio answers.

The declarative contract — the media types, the status of each operation, the representation's fields and its
`_links` — is validated by Schemathesis from the served document. These cases prove only what a schema cannot: that a
measurement the content does not report stays absent instead of being fabricated, that a file backs at most one audio,
that another media family creates none, and that following a representation's links reaches the resource it names.
Every route comes from the file's `_links.audio` or from the typed representation's own links, so the suite keeps its
single `/api/v1` base literal in `harness.Sut.index`.

The fixtures are hand-built PCM WAVs: a sample width chooses the reported bit depth, and a sample count chooses whether
the content reports a duration. The probe derives numbers only from what the container declares, so a `0`-sample WAV
reports no duration and a 64-bit WAV reports no bit depth — the two absences the capability promises not to fabricate.

A codec is compared case-insensitively: the specification fixes that the row reports one, not how it cases it.
"""

import struct
from datetime import UTC, datetime
from itertools import count

import allure
from conftest import FIXTURE_DIRECTORY, linked_id, upload
from harness import Sut, link_of

# A WAV's declared content type: the intake takes the declared family as authoritative and the bytes confirm it.
AUDIO_CONTENT_TYPE = "audio/vnd.wave"
VIDEO_CONTENT_TYPE = "video/mp4"

# One second of 16-bit mono samples at 44.1 kHz: the measurement set a PCM WAV reports in full.
SAMPLE_RATE_HZ = 44_100
ONE_SECOND = SAMPLE_RATE_HZ

# Every generated WAV carries a per-call sample byte, so two uploads never share a file through deduplication.
_AUDIO_NONCE = count(datetime.now(UTC).toordinal())

AUDIO_FIELDS = {
    "bit_depth",
    "codec",
    "duration_ms",
    "file_id",
    "id",
    "name",
    "sample_rate_hz",
    "_links",
}


def wav(sample_bits: int, samples: int) -> bytes:
    """Build a mono PCM WAV of the given sample width and count, unique per call."""
    nonce = next(_AUDIO_NONCE)
    sample_bytes = sample_bits // 8
    pcm = bytes((nonce + index) % 7 for index in range(sample_bytes * samples))
    format_chunk = struct.pack(
        "<HHIIHH",
        1,
        1,
        SAMPLE_RATE_HZ,
        SAMPLE_RATE_HZ * sample_bytes,
        sample_bytes,
        sample_bits,
    )
    body = b"WAVE" + b"fmt " + struct.pack("<I", len(format_chunk)) + format_chunk
    body += b"data" + struct.pack("<I", len(pcm)) + pcm
    return struct.pack("<4sI", b"RIFF", len(body)) + body


def upload_audio(server: Sut, token: str, name: str, data: bytes) -> dict:
    """Upload one audio file and return the file representation."""
    return upload(server, token, name, data, AUDIO_CONTENT_TYPE)


def typed_audio(server: Sut, token: str, file: dict) -> dict:
    """Follow a file's `audio` link and return the typed representation."""
    response = server.follow(file, "audio", token=token)
    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.text}"
    return response.json()


def unknown_audio(server: Sut, token: str, audio: dict):
    """Request the audio one identifier past `audio`, which the instance has never created."""
    href = link_of(audio, "self")
    base, _, _ = href.rpartition("/")
    return server.request("GET", f"{base}/{linked_id(href) + 1}", token=token)


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-001")
@allure.story("SCN-AUDIO-001-A")
def test_an_audio_file_creates_one_audio(server: Sut, root_token: str) -> None:
    """An audio upload leaves exactly one audio backing the uploaded file."""
    file = upload_audio(server, root_token, "track.wav", wav(16, ONE_SECOND))

    audio = typed_audio(server, root_token, file)

    assert audio["file_id"] == file["id"]
    assert link_of(file, "audio").endswith(f"/{audio['id']}")
    assert unknown_audio(server, root_token, audio).status_code == 404


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-001")
@allure.story("SCN-AUDIO-001-B")
def test_a_file_backs_one_audio(server: Sut, root_token: str) -> None:
    """Registering the same stored file again creates no second audio for it."""
    data = wav(16, ONE_SECOND)
    first = upload_audio(server, root_token, "track.wav", data)
    before = typed_audio(server, root_token, first)

    again = upload_audio(server, root_token, "track.wav", data)
    after = typed_audio(server, root_token, again)

    assert again["id"] == first["id"], "the same content must reuse its stored file"
    assert after["id"] == before["id"], "the second registration created a second audio"
    assert unknown_audio(server, root_token, after).status_code == 404


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-001")
@allure.story("SCN-AUDIO-001-C")
def test_image_and_video_content_create_no_audio(server: Sut, root_token: str) -> None:
    """Image and video uploads leave the file without an audio."""
    video = upload(
        server,
        root_token,
        "clip.mp4",
        (FIXTURE_DIRECTORY / "clip.mp4").read_bytes(),
        VIDEO_CONTENT_TYPE,
    )
    image = upload(
        server,
        root_token,
        "cover.png",
        (FIXTURE_DIRECTORY / "cover.png").read_bytes(),
        "image/png",
    )

    assert "audio" not in video["_links"]
    assert "audio" not in image["_links"]


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-002")
@allure.story("SCN-AUDIO-002-A")
def test_every_reported_measurement_is_reported(server: Sut, root_token: str) -> None:
    """A one-second 16-bit WAV reports its name, codec, duration, sample rate and bit depth."""
    audio = typed_audio(
        server,
        root_token,
        upload_audio(server, root_token, "song.wav", wav(16, ONE_SECOND)),
    )

    assert audio["name"] == "song.wav"
    assert audio["codec"].lower() == "pcm_s16le"
    assert abs(audio["duration_ms"] - 1000.0) <= 1.0
    assert audio["sample_rate_hz"] == SAMPLE_RATE_HZ
    assert audio["bit_depth"] == 16


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-002")
@allure.story("SCN-AUDIO-002-B")
def test_a_missing_bit_depth_and_block_size_stay_absent(server: Sut, root_token: str) -> None:
    """64-bit PCM reports no bit depth and no block size, and neither field is fabricated."""
    audio = typed_audio(
        server,
        root_token,
        upload_audio(server, root_token, "wide.wav", wav(64, ONE_SECOND)),
    )

    assert audio["codec"].lower() == "pcm_s64le"
    assert "bit_depth" not in audio
    assert "block_size" not in audio
    assert audio["duration_ms"] > 0.0
    assert audio["sample_rate_hz"] == SAMPLE_RATE_HZ


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-002")
@allure.story("SCN-AUDIO-002-C")
def test_a_missing_duration_stays_absent(server: Sut, root_token: str) -> None:
    """A WAV whose content reports no duration carries no duration rather than a fabricated one."""
    audio = typed_audio(
        server,
        root_token,
        upload_audio(server, root_token, "empty.wav", wav(16, 0)),
    )

    assert "duration_ms" not in audio
    assert audio["sample_rate_hz"] == SAMPLE_RATE_HZ


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-003")
@allure.story("SCN-AUDIO-003-A")
def test_an_audio_carries_its_representation(server: Sut, root_token: str) -> None:
    """The typed representation carries the audio's fields and links its own resource, file and content."""
    file = upload_audio(server, root_token, "voice.wav", wav(16, ONE_SECOND))
    typed_href = link_of(file, "audio")

    audio = typed_audio(server, root_token, file)

    assert set(audio) == AUDIO_FIELDS
    assert audio["name"] == "voice.wav"
    assert link_of(audio, "self") == typed_href
    assert link_of(audio, "file") == link_of(file, "self")
    assert link_of(audio, "content") == f"{typed_href}/content"


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-003")
@allure.story("SCN-AUDIO-003-B")
def test_an_unknown_audio_is_not_found(server: Sut, root_token: str) -> None:
    """An audio the instance never created answers `404`, like one the caller may not read."""
    audio = typed_audio(
        server,
        root_token,
        upload_audio(server, root_token, "gone.wav", wav(16, ONE_SECOND)),
    )

    response = unknown_audio(server, root_token, audio)

    assert response.status_code == 404, f"unknown audio answered {response.status_code}: {response.text}"


@allure.epic("Audio")
@allure.feature("REQ-AUDIO-004")
@allure.story("SCN-AUDIO-004-A")
def test_the_file_alias_redirects_to_the_file(server: Sut, root_token: str) -> None:
    """The typed file alias answers `303` with the backing file resource in `Location`."""
    file = upload_audio(server, root_token, "alias.wav", wav(16, ONE_SECOND))
    typed_href = link_of(file, "audio")

    response = server.request("GET", f"{typed_href}/file", token=root_token, allow_redirects=False)

    assert response.status_code == 303, f"alias answered {response.status_code}: {response.text}"
    assert response.headers["Location"] == link_of(file, "self")
