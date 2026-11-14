"""Black-box proof of the `music` capability.

The catalogue holds groups, albums with their mediums and placed recordings, and playlists whose ordered tracks
reference the same recordings. This module proves what a schema cannot: that the album write is an upsert by name and
group, that an unnamed album genre becomes `UNKNOWN`, that a medium is created, retyped and repeated without losing its
tracks, that a recording moves between destinations and its edit reaches every playlist, that the seeded default rows
refuse every write while staying readable, that a playlist name is unique per owner and trimmed, that a refused write
stores nothing,
and that only a caller holding the playlist's write action may change its tracks.

The declarative contract — the media types, the status of each operation, the representation's fields, its `_links` and
the request schema — is validated by Schemathesis from the served document. Every route comes from the API index
(`Sut.href`), from a representation's named link or from the response's own `_links.self`, so the suite keeps its single
`/api/v1` base literal in `harness.Sut.index`.

No route creates a music recording: a recording is placed into an album medium by identifier. The fixture therefore
stages the recording and its resource in the working directory's datastore, exactly as a seeded instance holds them,
and every assertion afterwards reads only the HTTP surface.

The image, audio, video and show helpers come from the feature modules that own them, so this module uploads real media
and never fabricates a typed identifier.
"""

import sqlite3
import time

import allure
from audio import ONE_SECOND, typed_audio, upload_audio, wav
from conftest import add_to_gallery, create_gallery, provision
from gallery import image_file
from harness import Sut

# The music resource kinds the resource-policy route addresses.
ALBUM = "MUSIC_ALBUM"
PLAYLIST = "MUSIC_PLAYLIST"
RECORDING = "MUSIC_RECORDING"

# The seeded default rows: both are readable and both refuse every write.
DEFAULT_ALBUM_ID = 0
DEFAULT_GROUP_ID = 0

# An identifier no album, group, playlist or recording ever carries.
UNKNOWN_ID = 999_999_999

# The medium type a release that is not a multi-disc compact disc is stored as.
DIGITAL = "DIGITAL"


def library_image(server: Sut, token: str) -> int:
    """Upload one image no resource has attached yet and return its identifier."""
    return image_id(server, token, image_file(server, token, "cover.png"))


def gallery_image(server: Sut, token: str) -> int:
    """Upload one image, use it as gallery media and return its identifier."""
    file = image_file(server, token, "gallery-cover.png")
    gallery = create_gallery(server, token, f"Media {time.time_ns()}")
    add_to_gallery(server, token, gallery, file["id"])
    return image_id(server, token, file)


def image_id(server: Sut, token: str, file: dict) -> int:
    """Follow a file's `image` link and return the image identifier."""
    response = server.follow(file, "image", token=token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.text}"
    return int(response.json()["id"])


def create_recording(server: Sut, token: str, name: str = "Song") -> int:
    """Stage one recording on a freshly uploaded audio and return its identifier.

    No route creates a recording, and a placed recording needs a resource so the
    playlist append can decide `ViewMetadata` on it. The fixture writes both rows
    into the datastore the container reads, beside the catalogue root the seeded
    grants cover.
    """
    audio = typed_audio(server, token, upload_audio(server, token, "track.wav", wav(16, ONE_SECOND)))
    with sqlite3.connect(server.dir / "mnemorium.db") as connection:
        catalogue_id = connection.execute(
            "SELECT resource_id FROM resource WHERE kind = 'CATALOGUE' AND entity_id = 0"
        ).fetchone()[0]
        recording_id = connection.execute(
            "INSERT INTO music_recording (name, audio_id) VALUES (?1, ?2) RETURNING music_recording_id",
            (name, audio["id"]),
        ).fetchone()[0]
        connection.execute(
            "INSERT INTO resource (kind, entity_id, owner_id, auth_parent_id, visibility)"
            " VALUES (?1, ?2, NULL, ?3, 'PUBLIC')",
            (RECORDING, recording_id, catalogue_id),
        )
    return int(recording_id)


# --- groups -------------------------------------------------------------------


def group_body(**overrides: object) -> dict[str, object]:
    """A group-creation payload, with optional overrides."""
    body: dict[str, object] = {"name": "The Band"}
    body.update(overrides)
    return body


def post_group(server: Sut, token: str, body: dict[str, object]):
    """POST a group creation and return the raw response."""
    return server.request("POST", server.href("music_group"), token=token, json=body)


def create_group(server: Sut, token: str, **overrides: object) -> dict:
    """Create a group and return its representation."""
    response = post_group(server, token, group_body(**overrides))
    assert response.status_code == 201, f"group creation failed: {response.status_code} {response.text}"
    return response.json()


def read_group(server: Sut, token: str, group_id: object):
    """GET one group by identifier."""
    return server.request("GET", f"{server.href('music_group')}/{group_id}", token=token)


def list_groups(server: Sut, token: str, query: str = ""):
    """GET the group collection with an optional query string."""
    return server.request("GET", f"{server.href('music_group')}{query}", token=token)


def patch_group(server: Sut, token: str, group_id: object, body: dict[str, object]):
    """PATCH one group and return the raw response."""
    return server.request("PATCH", f"{server.href('music_group')}/{group_id}", token=token, json=body)


# --- albums and their mediums -------------------------------------------------


def album_body(music_group_id: object, **overrides: object) -> dict[str, object]:
    """An album-creation payload naming `music_group_id`, with optional overrides."""
    body: dict[str, object] = {"name": "An Album", "music_group_id": music_group_id}
    body.update(overrides)
    return body


def post_album(server: Sut, token: str, body: dict[str, object]):
    """POST an album creation and return the raw response."""
    return server.request("POST", server.href("music_album"), token=token, json=body)


def create_album(server: Sut, token: str, music_group_id: object, **overrides: object) -> dict:
    """Create an album and return its representation."""
    response = post_album(server, token, album_body(music_group_id, **overrides))
    assert response.status_code == 201, f"album creation failed: {response.status_code} {response.text}"
    return response.json()


def read_album(server: Sut, token: str, album_id: object):
    """GET one album by identifier."""
    return server.request("GET", f"{server.href('music_album')}/{album_id}", token=token)


def list_albums(server: Sut, token: str, query: str = ""):
    """GET the album collection with an optional query string."""
    return server.request("GET", f"{server.href('music_album')}{query}", token=token)


def patch_album(server: Sut, token: str, album_id: object, body: dict[str, object]):
    """PATCH one album and return the raw response."""
    return server.request("PATCH", f"{server.href('music_album')}/{album_id}", token=token, json=body)


def medium_href(server: Sut, album_id: object, medium_index: object) -> str:
    """The route of one medium of `album_id`."""
    return f"{server.href('music_album')}/{album_id}/medium/{medium_index}"


def track_href(server: Sut, album_id: object, medium_index: object, track_index: object) -> str:
    """The route of one track of a medium of `album_id`."""
    return f"{medium_href(server, album_id, medium_index)}/track/{track_index}"


def put_medium(server: Sut, token: str, album_id: object, medium_index: object = 1, medium_type: str = DIGITAL):
    """PUT a medium write and return the raw response."""
    return server.request(
        "PUT",
        medium_href(server, album_id, medium_index),
        token=token,
        json={"type": medium_type},
    )


def read_medium(server: Sut, token: str, album_id: object, medium_index: object = 1):
    """GET one medium of an album."""
    return server.request("GET", medium_href(server, album_id, medium_index), token=token)


def list_tracks(server: Sut, token: str, album_id: object, medium_index: object = 1):
    """GET the tracks of one medium."""
    return server.request("GET", f"{medium_href(server, album_id, medium_index)}/track", token=token)


def read_track(server: Sut, token: str, album_id: object, medium_index: object = 1, track_index: object = 0):
    """GET one track of one medium."""
    return server.request("GET", track_href(server, album_id, medium_index, track_index), token=token)


def patch_track(
    server: Sut,
    token: str,
    album_id: object,
    medium_index: object,
    track_index: object,
    body: dict[str, object],
):
    """PATCH the recording behind one track address and return the raw response."""
    return server.request(
        "PATCH",
        track_href(server, album_id, medium_index, track_index),
        token=token,
        json=body,
    )


def place_recording(
    server: Sut,
    token: str,
    album_id: object,
    recording_id: int,
    medium_index: object = 1,
    track_index: object = 0,
    medium_type: str = DIGITAL,
):
    """PUT a recording placement and return the raw response."""
    return server.request(
        "PUT",
        track_href(server, album_id, medium_index, track_index),
        token=token,
        json={"recording_id": recording_id},
    )


def placed_catalogue(server: Sut, token: str, *, album_cover: int | None = None) -> dict[str, object]:
    """Build one group, album, digital medium and placed recording, and return their identifiers."""
    group = create_group(server, token, name=f"Band {time.time_ns()}")
    album = create_album(
        server,
        token,
        group["id"],
        name=f"Album {time.time_ns()}",
        cover_image_id=album_cover,
    )
    medium = put_medium(server, token, album["id"], 1, DIGITAL)
    assert medium.status_code in {200, 201}, f"medium write failed: {medium.status_code} {medium.text}"
    recording_id = create_recording(server, token, name="Song")
    placed = place_recording(server, token, album["id"], recording_id, 1, 0)
    assert placed.status_code in {200, 201}, f"placement failed: {placed.status_code} {placed.text}"
    return {
        "group": group,
        "album": album,
        "recording_id": recording_id,
        "track": placed.json(),
    }


# --- playlists ----------------------------------------------------------------


def playlist_body(name: str, **overrides: object) -> dict[str, object]:
    """A playlist-creation payload, with optional overrides."""
    body: dict[str, object] = {"name": name}
    body.update(overrides)
    return body


def post_playlist(server: Sut, token: str, body: dict[str, object]):
    """POST a playlist creation and return the raw response."""
    return server.request("POST", server.href("playlist"), token=token, json=body)


def create_playlist(server: Sut, token: str, name: str | None = None, **overrides: object) -> dict:
    """Create a playlist and return its representation."""
    body = playlist_body(name if name is not None else f"Playlist {time.time_ns()}", **overrides)
    response = post_playlist(server, token, body)
    assert response.status_code == 201, f"playlist creation failed: {response.status_code} {response.text}"
    return response.json()


def playlist_href(server: Sut, playlist_id: object) -> str:
    """The route of one playlist."""
    return f"{server.href('playlist')}/{playlist_id}"


def read_playlist(server: Sut, token: str, playlist_id: object):
    """GET one playlist by identifier."""
    return server.request("GET", playlist_href(server, playlist_id), token=token)


def read_playlist_tracks(server: Sut, token: str, playlist_id: object):
    """GET the track alias of one playlist."""
    return server.request("GET", f"{playlist_href(server, playlist_id)}/track", token=token)


def read_playlist_track(server: Sut, token: str, playlist_id: object, track_index: object):
    """GET one track of a playlist."""
    return server.request("GET", f"{playlist_href(server, playlist_id)}/track/{track_index}", token=token)


def list_playlists(server: Sut, token: str, query: str = ""):
    """GET the playlist collection with an optional query string."""
    return server.request("GET", f"{server.href('playlist')}{query}", token=token)


def patch_playlist(server: Sut, token: str, playlist_id: object, body: dict[str, object]):
    """PATCH one playlist and return the raw response."""
    return server.request("PATCH", playlist_href(server, playlist_id), token=token, json=body)


def reorder_playlist(server: Sut, token: str, playlist_id: object, order: list[int]):
    """PUT a complete track permutation and return the raw response."""
    return server.request(
        "PUT",
        f"{playlist_href(server, playlist_id)}/track",
        token=token,
        json={"order": order},
    )


def remove_playlist_track(server: Sut, token: str, playlist_id: object, track_index: object):
    """DELETE one track of a playlist and return the raw response."""
    return server.request(
        "DELETE",
        f"{playlist_href(server, playlist_id)}/track/{track_index}",
        token=token,
    )


def delete_playlist(server: Sut, token: str, playlist_id: object):
    """DELETE one playlist and return the raw response."""
    return server.request("DELETE", playlist_href(server, playlist_id), token=token)


def grant_contributor(server: Sut, token: str, resource_kind: str, resource_id: int, user_id: int) -> None:
    """Grant one caller the Contributor bundle on a resource."""
    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{resource_kind}/{resource_id}",
        token=token,
        json={
            "visibility": "PUBLIC",
            "grants": [
                {
                    "principal": {"kind": "USER", "id": user_id},
                    "role": "CONTRIBUTOR",
                    "effect": "ALLOW",
                }
            ],
        },
    )
    assert response.status_code == 200, f"grant failed: {response.status_code} {response.text}"


def deny_role(
    server: Sut,
    token: str,
    resource_kind: str,
    resource_id: int,
    user_id: int,
    role: str = "VIEWER",
) -> None:
    """Deny one caller a role bundle on a resource."""
    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{resource_kind}/{resource_id}",
        token=token,
        json={
            "visibility": "PUBLIC",
            "grants": [
                {
                    "principal": {"kind": "USER", "id": user_id},
                    "role": role,
                    "effect": "DENY",
                }
            ],
        },
    )
    assert response.status_code == 200, f"deny failed: {response.status_code} {response.text}"


def album_ids(response) -> list[int]:
    """The album identifiers a collection lists, without the seeded default album."""
    return [item["id"] for item in response.json()["_embedded"]["items"] if item["id"] != DEFAULT_ALBUM_ID]


def group_ids(response) -> list[int]:
    """The group identifiers a collection lists, without the seeded default group."""
    return [item["id"] for item in response.json()["_embedded"]["items"] if item["id"] != DEFAULT_GROUP_ID]


# --- albums -------------------------------------------------------------------


@allure.epic("Music")
@allure.feature("REQ-MUSIC-008")
@allure.story("SCN-MUSIC-008-A")
@allure.story("SCN-MUSIC-008-C")
def test_a_new_album_is_created(server: Sut, root_token: str) -> None:
    """An album is created from a name and group, stores the UNKNOWN genre and carries no medium yet."""
    group = create_group(server, root_token)

    response = post_album(server, root_token, album_body(group["id"], name="Kind of Blue"))

    assert response.status_code == 201, f"album creation failed: {response.status_code} {response.text}"
    assert "Location" in response.headers
    body = response.json()
    assert body["name"] == "Kind of Blue"
    assert body["id"] > 0
    assert body["genres"] == ["UNKNOWN"], "an unnamed genre must be stored as UNKNOWN"
    assert body["mediums"] == [], "a new album holds no medium"
    assert body["track_count"] == 0
    assert body["cover_image_id"] is None
    assert body["_links"]["group"]["href"].endswith(f"/{group['id']}")


@allure.epic("Music")
@allure.feature("REQ-MUSIC-008")
@allure.story("SCN-MUSIC-008-B")
def test_an_existing_album_is_returned_unchanged(server: Sut, root_token: str) -> None:
    """The same name and group answers 200 with the stored album, not a second album."""
    group = create_group(server, root_token)
    first = create_album(server, root_token, group["id"], name="Blue Train")

    response = post_album(server, root_token, album_body(group["id"], name="Blue Train"))

    assert response.status_code == 200, f"repeat failed: {response.status_code} {response.text}"
    assert response.json()["id"] == first["id"]
    listed = list_albums(server, root_token)
    assert album_ids(listed) == [first["id"]], "an upsert stored a second album"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-008")
@allure.story("SCN-MUSIC-008-D")
def test_unusable_album_creation_is_refused(server: Sut, root_token: str) -> None:
    """An empty name, an unknown group and an empty genre are refused and store nothing."""
    group = create_group(server, root_token)

    empty_name = post_album(server, root_token, album_body(group["id"], name=""))
    unknown_group = post_album(server, root_token, album_body(UNKNOWN_ID))
    empty_genre = post_album(server, root_token, album_body(group["id"], genres=[""]))

    assert empty_name.status_code == 422, f"empty name accepted: {empty_name.status_code}"
    assert unknown_group.status_code == 422, f"unknown group accepted: {unknown_group.status_code}"
    assert empty_genre.status_code == 422, f"empty genre accepted: {empty_genre.status_code}"
    assert album_ids(list_albums(server, root_token)) == [], "a refused creation stored an album"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-029")
@allure.story("SCN-MUSIC-029-A")
def test_album_metadata_is_updated_partially(server: Sut, root_token: str) -> None:
    """A new release date and genre list are stored and the album representation is returned."""
    group = create_group(server, root_token)
    album = create_album(server, root_token, group["id"], name="Kind of Blue")

    response = patch_album(server, root_token, album["id"], {"release_date": "1959-08-17", "genres": ["Jazz", "Modal"]})

    assert response.status_code == 200, f"update failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["release_date"] == "1959-08-17"
    assert set(body["genres"]) == {"JAZZ", "MODAL"}
    assert body["_links"]["self"]["href"].endswith(f"/{album['id']}")
    stored = read_album(server, root_token, album["id"]).json()
    assert stored["release_date"] == "1959-08-17"
    assert set(stored["genres"]) == {"JAZZ", "MODAL"}


@allure.epic("Music")
@allure.feature("REQ-MUSIC-029")
@allure.story("SCN-MUSIC-029-B")
def test_an_optional_album_value_is_cleared(server: Sut, root_token: str) -> None:
    """A `null` release date stores none and the representation reports `null`."""
    group = create_group(server, root_token)
    album = create_album(server, root_token, group["id"], release_date="1970-01-01")

    response = patch_album(server, root_token, album["id"], {"release_date": None})

    assert response.status_code == 200, f"clear failed: {response.status_code} {response.text}"
    assert response.json()["release_date"] is None
    assert read_album(server, root_token, album["id"]).json()["release_date"] is None


@allure.epic("Music")
@allure.feature("REQ-MUSIC-029")
@allure.story("SCN-MUSIC-029-C")
def test_a_group_change_that_collides_is_refused(server: Sut, root_token: str) -> None:
    """Moving an album onto another album's name and group answers `409` and applies nothing."""
    first = create_group(server, root_token, name=f"First {time.time_ns()}")
    second = create_group(server, root_token, name=f"Second {time.time_ns()}")
    title = f"Shared {time.time_ns()}"
    create_album(server, root_token, first["id"], name=title)
    album = create_album(server, root_token, second["id"], name=title)

    response = patch_album(server, root_token, album["id"], {"music_group_id": first["id"]})

    assert response.status_code == 409, f"collision accepted: {response.status_code} {response.text}"
    stored = read_album(server, root_token, album["id"]).json()
    assert stored["_links"]["group"]["href"].endswith(f"/{second['id']}"), "a refused update applied the group"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-029")
@allure.story("SCN-MUSIC-029-D")
def test_an_absent_album_field_keeps_its_value(server: Sut, root_token: str) -> None:
    """A request that names no production type leaves the stored production type unchanged."""
    group = create_group(server, root_token)
    album = create_album(server, root_token, group["id"], production_type="LIVE")

    response = patch_album(server, root_token, album["id"], {"name": "Renamed"})

    assert response.status_code == 200, f"update failed: {response.status_code} {response.text}"
    assert response.json()["name"] == "Renamed"
    assert response.json()["production_type"] == "LIVE", "an absent field changed its stored value"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-029")
@allure.story("SCN-MUSIC-029-E")
def test_an_invalid_album_value_applies_nothing(server: Sut, root_token: str) -> None:
    """An empty or over-long name, an empty genre and an unknown group all answer `422` and store nothing."""
    group = create_group(server, root_token)
    album = create_album(server, root_token, group["id"], name="Original")

    responses = [
        patch_album(server, root_token, album["id"], {"name": ""}),
        patch_album(server, root_token, album["id"], {"name": "x" * 101}),
        patch_album(server, root_token, album["id"], {"genres": [""]}),
        patch_album(server, root_token, album["id"], {"music_group_id": UNKNOWN_ID}),
        patch_album(server, root_token, album["id"], {"release_date": "not-a-date"}),
    ]

    for response in responses:
        assert response.status_code == 422, f"invalid update accepted: {response.status_code} {response.text}"
    stored = read_album(server, root_token, album["id"]).json()
    assert stored["name"] == "Original"
    assert stored["release_date"] is None


@allure.epic("Music")
@allure.feature("REQ-MUSIC-001")
@allure.story("SCN-MUSIC-001-A")
@allure.story("SCN-MUSIC-001-B")
@allure.story("SCN-MUSIC-001-C")
@allure.feature("REQ-MUSIC-002")
@allure.story("SCN-MUSIC-002-A")
@allure.story("SCN-MUSIC-002-B")
@allure.story("SCN-MUSIC-002-C")
@allure.story("SCN-MUSIC-002-D")
def test_the_album_collection_is_partial_and_filtered(server: Sut, root_token: str) -> None:
    """The collection carries partial albums, the name filter narrows it, and a detail read carries the medium."""
    group = create_group(server, root_token)
    cover = library_image(server, root_token)
    first = create_album(server, root_token, group["id"], name="First", cover_image_id=cover)
    second = create_album(server, root_token, group["id"], name="Second")

    response = list_albums(server, root_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    items = [item for item in response.json()["_embedded"]["items"] if item["id"] != DEFAULT_ALBUM_ID]
    assert album_ids(response) == [first["id"], second["id"]]
    assert set(items[0]) == {"cover_image_id", "id", "name", "_links"}
    assert items[0]["cover_image_id"] == cover
    assert "cover" in items[0]["_links"]
    assert items[1]["cover_image_id"] is None
    assert "cover" not in items[1]["_links"]

    filtered = list_albums(server, root_token, "?name=First")
    assert album_ids(filtered) == [first["id"]], "the name filter let another album through"

    read = read_album(server, root_token, second["id"])
    assert read.status_code == 200, f"read failed: {read.status_code} {read.text}"
    detail = read.json()
    assert detail["release_date"] is None
    assert detail["cover_image_id"] is None
    assert detail["_links"]["group"]["href"].endswith(f"/{group['id']}")

    assert read_album(server, root_token, UNKNOWN_ID).status_code == 404
    assert read_album(server, root_token, "not-an-identifier").status_code == 400
    unknown_filter = list_albums(server, root_token, "?name=Nobody")
    assert album_ids(unknown_filter) == [], "a caller discovers no matching album"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-003")
@allure.story("SCN-MUSIC-003-B")
@allure.feature("REQ-MUSIC-009")
@allure.story("SCN-MUSIC-009-A")
@allure.story("SCN-MUSIC-009-B")
@allure.story("SCN-MUSIC-009-C")
@allure.feature("REQ-MUSIC-004")
@allure.story("SCN-MUSIC-004-A")
@allure.story("SCN-MUSIC-004-B")
def test_a_medium_is_created_retyped_and_repeated(server: Sut, root_token: str) -> None:
    """A medium is created, retyped the same index, and a repeat of the stored type changes nothing."""
    group = create_group(server, root_token)
    album = create_album(server, root_token, group["id"])

    created = put_medium(server, root_token, album["id"], 1, DIGITAL)
    assert created.status_code == 201, f"medium creation failed: {created.status_code} {created.text}"
    assert created.json()["type"] == DIGITAL

    retyped = put_medium(server, root_token, album["id"], 1, "CD")
    assert retyped.status_code == 200, f"medium retype failed: {retyped.status_code} {retyped.text}"
    assert retyped.json()["type"] == "CD"

    repeated = put_medium(server, root_token, album["id"], 1, "CD")
    assert repeated.status_code == 200, f"repeat failed: {repeated.status_code} {repeated.text}"
    assert repeated.json()["type"] == "CD"

    read = read_medium(server, root_token, album["id"], 1)
    assert read.status_code == 200, f"medium read failed: {read.status_code} {read.text}"
    assert read.json()["type"] == "CD"
    assert read.json()["index"] == 1
    assert read_medium(server, root_token, album["id"], 2).status_code == 404
    assert read_medium(server, root_token, UNKNOWN_ID, 1).status_code == 404


@allure.epic("Music")
@allure.feature("REQ-MUSIC-009")
@allure.story("SCN-MUSIC-009-D")
@allure.feature("REQ-MUSIC-016")
@allure.story("SCN-MUSIC-016-A")
@allure.story("SCN-MUSIC-016-C")
def test_the_default_album_stays_readable_and_refuses_writes(server: Sut, root_token: str) -> None:
    """Album 0 is readable and refuses a medium write; an invalid medium type is refused on any album."""
    group = create_group(server, root_token)
    album = create_album(server, root_token, group["id"])

    readable = read_album(server, root_token, DEFAULT_ALBUM_ID)
    assert readable.status_code == 200, f"default album unreadable: {readable.status_code}"
    assert readable.json()["name"] == "Unknown"

    protected = put_medium(server, root_token, DEFAULT_ALBUM_ID, 1, "CD")
    assert protected.status_code == 422, f"default album write accepted: {protected.status_code}"

    invalid = put_medium(server, root_token, album["id"], 1, "VINYL")
    assert invalid.status_code == 422, f"invalid medium type accepted: {invalid.status_code}"

    assert read_medium(server, root_token, album["id"], 1).status_code == 404, "a refused write created a medium"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-010")
@allure.story("SCN-MUSIC-010-A")
@allure.story("SCN-MUSIC-010-B")
@allure.story("SCN-MUSIC-010-C")
@allure.story("SCN-MUSIC-010-E")
@allure.feature("REQ-MUSIC-011")
@allure.story("SCN-MUSIC-011-A")
@allure.story("SCN-MUSIC-011-B")
@allure.feature("REQ-MUSIC-007")
@allure.story("SCN-MUSIC-007-A")
@allure.story("SCN-MUSIC-007-B")
def test_a_recording_is_placed_moved_and_edited(server: Sut, root_token: str) -> None:
    """A recording is placed, moved, read without artwork beside its album, and renamed where a playlist sees it."""
    cover = library_image(server, root_token)
    catalogue = placed_catalogue(server, root_token, album_cover=cover)
    album_id = catalogue["album"]["id"]
    recording_id = catalogue["recording_id"]
    put_medium(server, root_token, album_id, 2, DIGITAL)

    read = read_track(server, root_token, album_id, 1, 0)
    assert read.status_code == 200, f"track read failed: {read.status_code} {read.text}"
    track = read.json()
    assert track["id"] == 0
    assert track["medium_index"] == 1
    assert track["recording_id"] == recording_id
    assert "cover_image_id" not in track, "a track must carry no artwork of its own"
    assert "cover" not in track["_links"], "a track must carry no cover link"
    assert track["_links"]["album"]["href"].endswith(f"/{album_id}")
    assert read_album(server, root_token, album_id).json()["cover_image_id"] == cover

    listed = list_tracks(server, root_token, album_id, 1)
    assert listed.status_code == 200, f"track list failed: {listed.status_code} {listed.text}"
    assert [item["id"] for item in listed.json()["items"]] == [0]
    assert "cover_image_id" not in listed.json()["items"][0]

    moved = place_recording(server, root_token, album_id, recording_id, 2, 5)
    assert moved.status_code == 200, f"move failed: {moved.status_code} {moved.text}"
    assert read_track(server, root_token, album_id, 1, 0).status_code == 404, "the previous placement survived"
    assert read_track(server, root_token, album_id, 2, 5).status_code == 200

    playlist = create_playlist(server, root_token, is_public=True)
    appended = patch_playlist(server, root_token, playlist["id"], {"add_track": {"recording_id": recording_id}})
    assert appended.status_code == 200, f"append failed: {appended.status_code} {appended.text}"

    renamed = patch_track(server, root_token, album_id, 2, 5, {"name": "Renamed"})
    assert renamed.status_code == 200, f"rename failed: {renamed.status_code} {renamed.text}"
    assert renamed.json()["name"] == "Renamed"
    entry = read_playlist_track(server, root_token, playlist["id"], 0)
    assert entry.json()["name"] == "Renamed"

    cleared = patch_track(server, root_token, album_id, 2, 5, {"name": None})
    assert cleared.status_code == 200, f"clear failed: {cleared.status_code} {cleared.text}"
    assert cleared.json()["name"] is None

    occupied_recording = create_recording(server, root_token, name="Other")
    assert place_recording(server, root_token, album_id, occupied_recording, 2, 5).status_code == 409

    unknown = place_recording(server, root_token, album_id, UNKNOWN_ID, 2, 6)
    assert unknown.status_code == 422, f"unknown recording accepted: {unknown.status_code}"
    assert place_recording(server, root_token, DEFAULT_ALBUM_ID, recording_id, 1, 0).status_code in {
        409,
        422,
    }, "a default-album write was accepted"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-011")
@allure.story("SCN-MUSIC-011-C")
@allure.story("SCN-MUSIC-011-D")
def test_the_recording_edit_refusals(server: Sut, root_token: str) -> None:
    """A duplicate ISRC, a malformed ISRC, an unknown track and the default album are refused."""
    catalogue = placed_catalogue(server, root_token)
    album_id = catalogue["album"]["id"]
    other = placed_catalogue(server, root_token)
    other_album_id = other["album"]["id"]

    first = patch_track(server, root_token, album_id, 1, 0, {"isrc_code": "USRC17607839"})
    assert first.status_code == 200, f"isrc write failed: {first.status_code} {first.text}"
    duplicate = patch_track(server, root_token, other_album_id, 1, 0, {"isrc_code": "USRC17607839"})
    assert duplicate.status_code == 409, f"duplicate ISRC accepted: {duplicate.status_code}"

    malformed = patch_track(server, root_token, other_album_id, 1, 0, {"isrc_code": "SHORT"})
    assert malformed.status_code == 422, f"malformed ISRC accepted: {malformed.status_code}"

    assert patch_track(server, root_token, album_id, 1, 9, {"name": "x"}).status_code == 404
    assert patch_track(server, root_token, UNKNOWN_ID, 1, 0, {"name": "x"}).status_code == 404

    recording_id = create_recording(server, root_token)
    assert place_recording(server, root_token, DEFAULT_ALBUM_ID, recording_id, 1, 0).status_code in {
        409,
        422,
    }


# --- groups -------------------------------------------------------------------


@allure.epic("Music")
@allure.feature("REQ-MUSIC-012")
@allure.story("SCN-MUSIC-012-A")
@allure.story("SCN-MUSIC-012-B")
@allure.feature("REQ-MUSIC-013")
@allure.story("SCN-MUSIC-013-A")
@allure.story("SCN-MUSIC-013-B")
@allure.feature("REQ-MUSIC-014")
@allure.story("SCN-MUSIC-014-A")
@allure.story("SCN-MUSIC-014-B")
@allure.story("SCN-MUSIC-014-C")
@allure.story("SCN-MUSIC-014-D")
@allure.feature("REQ-MUSIC-017")
@allure.story("SCN-MUSIC-017-A")
@allure.story("SCN-MUSIC-017-B")
def test_a_group_is_created_listed_and_read(server: Sut, root_token: str) -> None:
    """A group is created with artwork, listed by name and names the group of its album."""
    cover = library_image(server, root_token)
    first = create_group(server, root_token, name="First Band", cover_image_id=cover)
    assert first["cover_image_id"] == cover
    assert first["_links"]["cover"]["href"].endswith(f"/{cover}")

    second = create_group(server, root_token, name="Second Band")
    response = list_groups(server, root_token)
    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert group_ids(response) == [first["id"], second["id"]]
    filtered = list_groups(server, root_token, "?name=Second+Band")
    assert group_ids(filtered) == [second["id"]]

    read = read_group(server, root_token, second["id"])
    assert read.status_code == 200, f"read failed: {read.status_code} {read.text}"
    assert read.json()["name"] == "Second Band"
    assert read.json()["cover_image_id"] is None
    assert "cover" not in read.json()["_links"]
    assert read_group(server, root_token, UNKNOWN_ID).status_code == 404

    album = create_album(server, root_token, first["id"], name="Group Album")
    assert album["_links"]["group"]["href"].endswith(f"/{first['id']}")
    other = create_group(server, root_token, name="Third Band")
    moved = patch_album(server, root_token, album["id"], {"music_group_id": other["id"]})
    assert moved.status_code == 200, f"album move failed: {moved.status_code} {moved.text}"
    assert read_album(server, root_token, album["id"]).json()["_links"]["group"]["href"].endswith(f"/{other['id']}")

    duplicate = post_group(server, root_token, group_body(name="First Band"))
    assert duplicate.status_code == 409, f"duplicate group accepted: {duplicate.status_code}"
    unusable = post_group(server, root_token, group_body(name="  "))
    assert unusable.status_code == 422, f"empty group name accepted: {unusable.status_code}"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-015")
@allure.story("SCN-MUSIC-015-A")
@allure.story("SCN-MUSIC-015-B")
@allure.story("SCN-MUSIC-015-C")
@allure.story("SCN-MUSIC-015-D")
@allure.story("SCN-MUSIC-015-E")
def test_a_group_is_renamed_and_its_cover_cleared(server: Sut, root_token: str) -> None:
    """A group is renamed keeping its artwork, its artwork is replaced and cleared, and a refusal stores nothing."""
    first_cover = library_image(server, root_token)
    second_cover = library_image(server, root_token)
    group = create_group(server, root_token, name="Original", cover_image_id=first_cover)
    other = create_group(server, root_token, name="Taken")

    renamed = patch_group(server, root_token, group["id"], {"name": "Renamed"})
    assert renamed.status_code == 200, f"rename failed: {renamed.status_code} {renamed.text}"
    assert renamed.json()["name"] == "Renamed"
    assert renamed.json()["cover_image_id"] == first_cover, "a rename must keep the stored cover"

    replaced = patch_group(server, root_token, group["id"], {"cover_image_id": second_cover})
    assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"
    assert replaced.json()["cover_image_id"] == second_cover

    kept = patch_group(server, root_token, group["id"], {"name": "Renamed Again"})
    assert kept.status_code == 200, f"absent cover failed: {kept.status_code} {kept.text}"
    assert kept.json()["cover_image_id"] == second_cover, "an absent field must keep the cover"

    cleared = patch_group(server, root_token, group["id"], {"cover_image_id": None})
    assert cleared.status_code == 200, f"clear failed: {cleared.status_code} {cleared.text}"
    assert cleared.json()["cover_image_id"] is None
    assert "cover" not in cleared.json()["_links"], "a cleared cover keeps its link"

    duplicate = patch_group(server, root_token, group["id"], {"name": "Taken"})
    assert duplicate.status_code == 409, f"duplicate name accepted: {duplicate.status_code}"
    assert read_group(server, root_token, group["id"]).json()["name"] == "Renamed Again"
    assert other["name"] == "Taken"

    gallery_cover = gallery_image(server, root_token)
    unusable = patch_group(server, root_token, group["id"], {"cover_image_id": gallery_cover})
    assert unusable.status_code == 409, f"gallery media accepted as cover: {unusable.status_code}"

    invalid = patch_group(server, root_token, group["id"], {"name": "   "})
    assert invalid.status_code == 422, f"empty name accepted: {invalid.status_code}"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-016")
@allure.story("SCN-MUSIC-016-A")
@allure.story("SCN-MUSIC-016-B")
def test_the_default_group_is_protected(server: Sut, root_token: str) -> None:
    """Group 0 is readable and every write to it answers 409."""
    readable = read_group(server, root_token, DEFAULT_GROUP_ID)
    assert readable.status_code == 200, f"default group unreadable: {readable.status_code}"

    rename = patch_group(server, root_token, DEFAULT_GROUP_ID, {"name": "Changed"})
    cover = patch_group(server, root_token, DEFAULT_GROUP_ID, {"cover_image_id": library_image(server, root_token)})

    assert rename.status_code == 409, f"default group renamed: {rename.status_code}"
    assert cover.status_code == 409, f"default group covered: {cover.status_code}"
    assert read_group(server, root_token, DEFAULT_GROUP_ID).json()["name"] == "Unknown"


# --- playlists ----------------------------------------------------------------


@allure.epic("Music")
@allure.feature("REQ-MUSIC-019")
@allure.story("SCN-MUSIC-019-A")
@allure.story("SCN-MUSIC-019-B")
@allure.story("SCN-MUSIC-019-C")
@allure.feature("REQ-MUSIC-020")
@allure.story("SCN-MUSIC-020-A")
@allure.story("SCN-MUSIC-020-B")
@allure.story("SCN-MUSIC-020-C")
@allure.feature("REQ-MUSIC-021")
@allure.story("SCN-MUSIC-021-A")
def test_a_playlist_is_created_private_or_public(server: Sut, root_token: str) -> None:
    """A playlist is created private by default, public on request, and its name is trimmed and unique per owner."""
    _alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")

    private = create_playlist(server, alice, "  Mine  ")
    assert private["name"] == "Mine", "the stored name must be trimmed"
    assert private["is_public"] is False
    assert private["owner_id"] == _alice_id
    assert private["tracks"] == []
    assert private["track_count"] == 0
    assert private["cover_image_id"] is None

    public = create_playlist(server, alice, "Shared", is_public=True)
    assert public["is_public"] is True

    duplicate = post_playlist(server, alice, playlist_body("Mine"))
    assert duplicate.status_code == 409, f"duplicate name accepted: {duplicate.status_code}"
    same_name = create_playlist(server, bob, "Mine")
    assert same_name["id"] != private["id"], "two owners may hold the same name"

    empty = post_playlist(server, alice, playlist_body("   "))
    too_long = post_playlist(server, alice, playlist_body("x" * 101))
    assert empty.status_code == 422, f"empty name accepted: {empty.status_code}"
    assert too_long.status_code == 422, f"over-long name accepted: {too_long.status_code}"

    read = read_playlist(server, bob, public["id"])
    assert read.status_code == 200, f"public read failed: {read.status_code} {read.text}"
    assert read.json()["name"] == "Shared"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-019")
@allure.story("SCN-MUSIC-019-A")
def test_a_denied_callers_playlist_creation_is_refused(server: Sut, root_token: str) -> None:
    """A caller whose CreateChild on the playlist collection is denied answers 403 and creates nothing."""
    alice_id, alice = provision(server, root_token, "alice")
    deny_role(server, root_token, "PLAYLIST_COLLECTION", 0, alice_id, "CONTRIBUTOR")

    response = post_playlist(server, alice, playlist_body("Denied"))

    assert response.status_code == 403, f"denied creation accepted: {response.status_code} {response.text}"
    assert list_playlists(server, alice).json()["items"] == [], "a refused creation stored a playlist"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-018")
@allure.story("SCN-MUSIC-018-A")
@allure.story("SCN-MUSIC-018-B")
@allure.story("SCN-MUSIC-018-C")
def test_the_playlist_collection_lists_public_and_owned(server: Sut, root_token: str) -> None:
    """The collection carries the caller's own and the public playlists, and filters on every field."""
    alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")
    mine = create_playlist(server, alice, "Mine")
    shared = create_playlist(server, alice, "Shared", is_public=True)
    create_playlist(server, bob, "Hidden")

    response = list_playlists(server, alice)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    listed = [item["id"] for item in response.json()["items"]]
    assert listed == [mine["id"], shared["id"]], "the collection must hide another owner's private playlist"

    by_owner = list_playlists(server, alice, f"?owner_id={alice_id}&is_public=true")
    assert [item["id"] for item in by_owner.json()["items"]] == [shared["id"]]
    hidden_from_alice = list_playlists(server, alice, f"?owner_id={_bob_id}")
    assert hidden_from_alice.json()["items"] == [], "another owner's private playlist leaked through a filter"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-024")
@allure.story("SCN-MUSIC-024-A")
@allure.story("SCN-MUSIC-024-B")
@allure.story("SCN-MUSIC-024-C")
@allure.feature("REQ-MUSIC-021")
@allure.story("SCN-MUSIC-021-C")
@allure.feature("REQ-MUSIC-022")
@allure.story("SCN-MUSIC-022-B")
def test_playlist_metadata_is_updated(server: Sut, root_token: str) -> None:
    """A metadata update replaces the name and visibility, an empty one changes nothing, and a collision is refused."""
    _alice_id, alice = provision(server, root_token, "alice")
    playlist = create_playlist(server, alice, "Mine")
    create_playlist(server, alice, "Taken")

    updated = patch_playlist(server, alice, playlist["id"], {"name": "Renamed", "is_public": True})
    assert updated.status_code == 200, f"update failed: {updated.status_code} {updated.text}"
    assert updated.json()["name"] == "Renamed"
    assert updated.json()["is_public"] is True
    assert updated.json()["last_modified_at"] >= playlist["last_modified_at"]

    empty = patch_playlist(server, alice, playlist["id"], {})
    assert empty.status_code == 200, f"empty update failed: {empty.status_code} {empty.text}"
    assert empty.json()["name"] == "Renamed"

    duplicate = patch_playlist(server, alice, playlist["id"], {"name": "Taken"})
    assert duplicate.status_code == 409, f"duplicate name accepted: {duplicate.status_code}"
    invalid = patch_playlist(server, alice, playlist["id"], {"name": "  "})
    assert invalid.status_code == 422, f"empty name accepted: {invalid.status_code}"

    assert read_playlist(server, alice, UNKNOWN_ID).status_code == 404
    assert read_playlist(server, alice, "abc").status_code == 400
    assert read_playlist_track(server, alice, playlist["id"], 0).status_code == 404


@allure.epic("Music")
@allure.feature("REQ-MUSIC-022")
@allure.story("SCN-MUSIC-022-A")
@allure.feature("REQ-MUSIC-023")
@allure.story("SCN-MUSIC-023-A")
@allure.story("SCN-MUSIC-023-B")
@allure.feature("REQ-MUSIC-025")
@allure.story("SCN-MUSIC-025-A")
def test_a_recording_is_appended_and_read(server: Sut, root_token: str) -> None:
    """Two recordings append at contiguous indices and the track representations carry their shared recordings."""
    _alice_id, alice = provision(server, root_token, "alice")
    cover = library_image(server, root_token)
    catalogue = placed_catalogue(server, root_token, album_cover=cover)
    recording_id = catalogue["recording_id"]
    other = placed_catalogue(server, root_token)
    playlist = create_playlist(server, alice, is_public=True)

    first = patch_playlist(server, alice, playlist["id"], {"add_track": {"recording_id": recording_id}})
    assert first.status_code == 200, f"first append failed: {first.status_code} {first.text}"
    appended = patch_playlist(server, alice, playlist["id"], {"add_track": {"recording_id": other["recording_id"]}})
    assert appended.status_code == 200, f"second append failed: {appended.status_code} {appended.text}"

    tracks = appended.json()["tracks"]
    assert [track["id"] for track in tracks] == [0, 1], "playlist indices must be contiguous from zero"
    assert appended.json()["track_count"] == 2

    entry = read_playlist_track(server, root_token, playlist["id"], 0)
    assert entry.status_code == 200, f"track read failed: {entry.status_code} {entry.text}"
    assert entry.json()["recording_id"] == recording_id
    assert "cover_image_id" not in entry.json(), "a playlist track must carry no artwork of its own"
    assert "cover" not in entry.json()["_links"]
    assert entry.json()["_links"]["playlist"]["href"].endswith(f"/{playlist['id']}")

    alias = read_playlist_tracks(server, root_token, playlist["id"])
    assert alias.status_code == 200, f"alias failed: {alias.status_code} {alias.text}"
    assert [track["id"] for track in alias.json()["tracks"]] == [0, 1]

    duplicate = patch_playlist(server, root_token, playlist["id"], {"add_track": {"recording_id": recording_id}})
    assert duplicate.status_code == 409, f"duplicate recording accepted: {duplicate.status_code}"
    unknown = patch_playlist(server, root_token, playlist["id"], {"add_track": {"recording_id": UNKNOWN_ID}})
    assert unknown.status_code == 422, f"unknown recording accepted: {unknown.status_code}"


@allure.epic("Music")
@allure.feature("REQ-MUSIC-025")
@allure.story("SCN-MUSIC-025-B")
def test_an_unreadable_recording_is_refused(server: Sut, root_token: str) -> None:
    """A caller who may add to the playlist but may not read the recording answers 403 and appends nothing."""
    bob_id, bob = provision(server, root_token, "bobby")
    catalogue = placed_catalogue(server, root_token)
    playlist = create_playlist(server, root_token, is_public=True)
    grant_contributor(server, root_token, PLAYLIST, playlist["id"], bob_id)
    deny_role(server, root_token, RECORDING, catalogue["recording_id"], bob_id)

    response = patch_playlist(server, bob, playlist["id"], {"add_track": {"recording_id": catalogue["recording_id"]}})

    assert response.status_code == 403, f"unreadable recording appended: {response.status_code} {response.text}"
    assert read_playlist(server, root_token, playlist["id"]).json()["tracks"] == []


@allure.epic("Music")
@allure.feature("REQ-MUSIC-026")
@allure.story("SCN-MUSIC-026-A")
@allure.story("SCN-MUSIC-026-B")
@allure.story("SCN-MUSIC-026-C")
def test_a_track_is_removed_and_the_recording_survives(server: Sut, root_token: str) -> None:
    """A removed occurrence is gone while its recording stays readable through the album."""
    catalogue = placed_catalogue(server, root_token)
    recording_id = catalogue["recording_id"]
    album_id = catalogue["album"]["id"]
    playlist = create_playlist(server, root_token)
    patch_playlist(server, root_token, playlist["id"], {"add_track": {"recording_id": recording_id}})

    removed = remove_playlist_track(server, root_token, playlist["id"], 0)

    assert removed.status_code == 204, f"removal failed: {removed.status_code} {removed.text}"
    playlist_tracks = read_playlist(server, root_token, playlist["id"]).json()["tracks"]
    assert playlist_tracks == [], "the removed occurrence survived"
    assert read_track(server, root_token, album_id, 1, 0).json()["recording_id"] == recording_id
    assert read_playlist_track(server, root_token, playlist["id"], 0).status_code == 404


@allure.epic("Music")
@allure.feature("REQ-MUSIC-027")
@allure.story("SCN-MUSIC-027-A")
@allure.story("SCN-MUSIC-027-B")
def test_the_tracks_of_a_playlist_are_reordered(server: Sut, root_token: str) -> None:
    """A complete permutation reindexes the tracks and an incomplete one is refused."""
    playlist = create_playlist(server, root_token)
    recordings = [create_recording(server, root_token, name=f"Song {index}") for index in range(3)]
    for recording_id in recordings:
        appended = patch_playlist(server, root_token, playlist["id"], {"add_track": {"recording_id": recording_id}})
        assert appended.status_code == 200, f"append failed: {appended.status_code} {appended.text}"

    reordered = reorder_playlist(server, root_token, playlist["id"], [2, 0, 1])

    assert reordered.status_code == 200, f"reorder failed: {reordered.status_code} {reordered.text}"
    tracks = reordered.json()["tracks"]
    assert [track["id"] for track in tracks] == [0, 1, 2]
    assert [track["recording_id"] for track in tracks] == [recordings[2], recordings[0], recordings[1]]

    for invalid in ([0, 1], [0, 0, 1], [0, 1, 9]):
        response = reorder_playlist(server, root_token, playlist["id"], invalid)
        assert response.status_code == 422, f"{invalid} accepted: {response.status_code}"

    unchanged = read_playlist(server, root_token, playlist["id"]).json()["tracks"]
    assert [track["recording_id"] for track in unchanged] == [recordings[2], recordings[0], recordings[1]]


@allure.epic("Music")
@allure.feature("REQ-MUSIC-028")
@allure.story("SCN-MUSIC-028-A")
@allure.story("SCN-MUSIC-028-B")
@allure.story("SCN-MUSIC-028-C")
def test_a_playlist_is_deleted_and_recordings_survive(server: Sut, root_token: str) -> None:
    """A deleted playlist takes its placements and keeps the recordings, audio and files it referenced."""
    catalogue = placed_catalogue(server, root_token)
    playlist = create_playlist(server, root_token)
    patch_playlist(server, root_token, playlist["id"], {"add_track": {"recording_id": catalogue["recording_id"]}})

    response = delete_playlist(server, root_token, playlist["id"])

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    assert read_playlist(server, root_token, playlist["id"]).status_code == 404
    surviving = read_track(server, root_token, catalogue["album"]["id"], 1, 0)
    assert surviving.status_code == 200, "the recording's placement did not survive the playlist deletion"
    assert surviving.json()["recording_id"] == catalogue["recording_id"]
    assert delete_playlist(server, root_token, playlist["id"]).status_code == 404


@allure.epic("Music")
@allure.feature("REQ-MUSIC-008")
@allure.story("SCN-MUSIC-008-D")
@allure.feature("REQ-MUSIC-014")
@allure.story("SCN-MUSIC-014-D")
@allure.feature("REQ-MUSIC-015")
@allure.story("SCN-MUSIC-015-D")
def test_a_standard_user_cannot_curate_the_catalogue(server: Sut, root_token: str) -> None:
    """A Standard user reads a group and an album but every catalogue write answers 403."""
    _alice_id, alice = provision(server, root_token, "alice")
    group = create_group(server, root_token)

    assert read_group(server, alice, group["id"]).status_code == 200
    assert post_group(server, alice, group_body(name="Denied")).status_code == 403
    assert patch_group(server, alice, group["id"], {"name": "Denied"}).status_code == 403
    assert post_album(server, alice, album_body(group["id"], name="Denied")).status_code == 403
