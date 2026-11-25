"""Black-box proof of the `audio` capability."""

import json
from itertools import count

from conftest import SEED_MEDIA_DIRECTORY, linked_id, provision, upload
from harness import Sut
from labels import business_rule, negative, positive, scenario, security
from mnemorium_client.api.asset import get_audio, get_file

# The seeded slots this module reads, documented in `seed/sql/0004_audio.sql`.
FULL_FILE = 14101
READ_FILE = 14104
WIDE_FILE = 14105
EMPTY_FILE = 14106

AUDIO_CONTENT_TYPE = "audio/vnd.wave"
SAMPLE_RATE_HZ = 44_100
CODEC_16_BIT = "pcm_s16le"
CODEC_64_BIT = "pcm_s64le"
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

# An identifier no intake can mint within a suite run.
UNKNOWN_AUDIO = 999_999_999

_ACCOUNTS = count()


@positive
@scenario("SCN-AUDIO-001-A")
def test_an_audio_file_creates_one_audio(server: Sut, root_token: str) -> None:
    """An audio upload leaves one audio backed by the uploaded file."""
    data = (SEED_MEDIA_DIRECTORY / "audio-create.wav").read_bytes()
    file = upload(server, root_token, "track.wav", data, AUDIO_CONTENT_TYPE)

    response = server.follow(file, "audio", token=root_token)
    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    audio = json.loads(response.content)

    assert audio["file_id"] == file["id"]
    assert file["_links"]["audio"]["href"].endswith(f"/{audio['id']}")


@business_rule
@scenario("SCN-AUDIO-001-B")
def test_a_file_backs_one_audio(server: Sut, root_token: str) -> None:
    """Uploading the same content twice reuses the stored file and its one audio."""
    data = (SEED_MEDIA_DIRECTORY / "audio-dedupe.wav").read_bytes()
    first = upload(server, root_token, "dedupe.wav", data, AUDIO_CONTENT_TYPE)
    first_response = server.follow(first, "audio", token=root_token)
    assert first_response.status_code == 200, (
        f"audio read failed: {first_response.status_code} {first_response.content!r}"
    )
    first_audio = json.loads(first_response.content)

    second = upload(server, root_token, "dedupe.wav", data, AUDIO_CONTENT_TYPE)
    second_response = server.follow(second, "audio", token=root_token)
    assert second_response.status_code == 200, (
        f"audio read failed: {second_response.status_code} {second_response.content!r}"
    )
    second_audio = json.loads(second_response.content)

    assert second["id"] == first["id"], "the same content must reuse its stored file"
    assert second_audio["id"] == first_audio["id"], "the second registration created a second audio"


@negative
@scenario("SCN-AUDIO-001-C")
def test_image_and_video_content_create_no_audio(server: Sut, root_token: str) -> None:
    """Image and video uploads leave the file without an audio."""
    image = upload(
        server,
        root_token,
        "cover.png",
        (SEED_MEDIA_DIRECTORY / "library-cover.png").read_bytes(),
        "image/png",
    )
    video = upload(
        server,
        root_token,
        "clip.mp4",
        (SEED_MEDIA_DIRECTORY / "episode.mp4").read_bytes(),
        "video/mp4",
    )

    assert "audio" not in image["_links"]
    assert "audio" not in video["_links"]


@positive
@scenario("SCN-AUDIO-002-A")
def test_every_reported_measurement_is_reported(server: Sut, root_token: str) -> None:
    """The one-second 16-bit audio reports its name, codec, duration, sample rate and bit depth."""
    file_response = get_file.sync_detailed(FULL_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)

    response = server.follow(file, "audio", token=root_token)
    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    audio = json.loads(response.content)

    assert audio["name"] == "audio-full.wav"
    assert audio["codec"].lower() == CODEC_16_BIT
    assert abs(audio["duration_ms"] - 1000.0) <= 1.0
    assert audio["sample_rate_hz"] == SAMPLE_RATE_HZ
    assert audio["bit_depth"] == 16


@business_rule
@scenario("SCN-AUDIO-002-B")
def test_a_missing_bit_depth_and_block_size_stay_absent(server: Sut, root_token: str) -> None:
    """The 64-bit audio reports neither a bit depth nor a block size."""
    file_response = get_file.sync_detailed(WIDE_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)

    response = server.follow(file, "audio", token=root_token)
    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    audio = json.loads(response.content)

    assert audio["codec"].lower() == CODEC_64_BIT
    assert "bit_depth" not in audio
    assert "block_size" not in audio
    assert audio["duration_ms"] > 0.0
    assert audio["sample_rate_hz"] == SAMPLE_RATE_HZ


@business_rule
@scenario("SCN-AUDIO-002-C")
def test_a_missing_duration_stays_absent(server: Sut, root_token: str) -> None:
    """The zero-sample audio carries no duration rather than a fabricated one."""
    file_response = get_file.sync_detailed(EMPTY_FILE, client=server.client(root_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)

    response = server.follow(file, "audio", token=root_token)
    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    audio = json.loads(response.content)

    assert "duration_ms" not in audio
    assert audio["sample_rate_hz"] == SAMPLE_RATE_HZ


@positive
@scenario("SCN-AUDIO-003-A")
def test_an_audio_carries_its_representation(server: Sut, root_token: str) -> None:
    """A standard caller the rule admits reads the typed representation with all its fields and links."""
    _, reader = provision(server, root_token, f"audioreader{next(_ACCOUNTS)}")
    file_response = get_file.sync_detailed(READ_FILE, client=server.client(reader))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.content!r}"
    file = json.loads(file_response.content)
    typed_href = file["_links"]["audio"]["href"]

    response = server.follow(file, "audio", token=reader)
    assert response.status_code == 200, f"audio read failed: {response.status_code} {response.content!r}"
    audio = json.loads(response.content)

    assert set(audio) == AUDIO_FIELDS
    assert audio["name"] == "audio-read.wav"
    assert audio["_links"]["self"]["href"] == typed_href
    assert audio["_links"]["file"]["href"] == file["_links"]["self"]["href"]
    assert audio["_links"]["content"]["href"] == f"{typed_href}/content"


@security
@scenario("SCN-AUDIO-003-B")
def test_an_unknown_or_unreadable_audio_is_concealed(server: Sut, root_token: str) -> None:
    """An unknown audio and another caller's private audio answer the same `404`."""
    _, owner = provision(server, root_token, f"audioowner{next(_ACCOUNTS)}")
    _, stranger = provision(server, root_token, f"audiostranger{next(_ACCOUNTS)}")
    file = upload(
        server,
        owner,
        "private.wav",
        (SEED_MEDIA_DIRECTORY / "audio-full.wav").read_bytes(),
        AUDIO_CONTENT_TYPE,
    )
    audio_id = linked_id(file["_links"]["audio"]["href"])

    owned = get_audio.sync_detailed(audio_id, client=server.client(owner))
    concealed = get_audio.sync_detailed(audio_id, client=server.client(stranger))
    unknown = get_audio.sync_detailed(UNKNOWN_AUDIO, client=server.client(stranger))

    assert owned.status_code == 200, f"audio read failed: {owned.status_code} {owned.content!r}"
    assert concealed.status_code == 404, "another caller's private audio must be concealed"
    assert unknown.status_code == 404, "an unknown audio must answer 404"
    assert json.loads(concealed.content)["error"] == json.loads(unknown.content)["error"], (
        "a concealed audio answers differently from a missing one"
    )
