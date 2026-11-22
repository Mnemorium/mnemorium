"""Black-box proof of the `music` capability."""

import datetime
import json
import time
from itertools import count

from conftest import provision
from harness import Sut
from labels import scenario
from mnemorium_client.api.library import (
    delete_playlist,
    delete_playlist_track,
    get_album,
    get_album_medium,
    get_album_track,
    get_album_track_list,
    get_music_group,
    get_playlist,
    get_playlist_track,
    get_playlist_track_list,
    list_albums,
    list_music_groups,
    list_playlists,
    patch_album,
    patch_album_track,
    patch_music_group,
    patch_playlist,
    post_album,
    post_music_group,
    post_playlist,
    put_album_medium,
    put_album_track,
    put_playlist_track,
)
from mnemorium_client.models import (
    AddPlaylistTrackRequest,
    PatchAlbumRequest,
    PatchAlbumTrackRequest,
    PatchMusicGroupRequest,
    PatchPlaylistRequest,
    PostAlbumRequest,
    PostMusicGroupRequest,
    PostPlaylistRequest,
    PutAlbumMediumRequest,
    PutAlbumTrackRequest,
    ReorderPlaylistTracksRequest,
)

# The seeded slots this module reads, documented in `seed/sql/0002_music.sql`.
DETAIL_GROUP = 12003
BARE_GROUP = 12004
NEW_ALBUM_GROUP = 12005
UNNAMED_GENRE_GROUP = 12006
EXISTING_ALBUM_GROUP = 12007
REFUSAL_GROUP = 12008
COLLIDE_FIRST_GROUP = 12011
COLLIDE_SECOND_GROUP = 12012
LISTED_ONE = 12034
LISTED_TWO = 12035
DETAIL_GROUP_ENTITY = 12036
RENAME_KEEP_GROUP = 12037
REPLACED_ARTWORK_GROUP = 12038
CLEARED_ARTWORK_GROUP = 12039
RENAMED_GROUP = 12041
INVALID_NAME_GROUP = 12042
ALBUM_GROUP = 12043
MOVE_TARGET_GROUP = 12045

COLLECTION_COVERED = 12101
COLLECTION_BARE = 12102
FILTERED_ALBUM = 12103
DETAIL_ALBUM = 12104
BARE_ALBUM = 12105
EXISTING_ALBUM = 12106
UPDATED_ALBUM = 12107
CLEARED_ALBUM = 12108
COLLIDE_SECOND_ALBUM = 12110
ABSENT_ALBUM = 12111
MEDIUM_ALBUM = 12112
RETYPE_ALBUM = 12113
IDEMPOTENT_ALBUM = 12114
MEDIUM_READ_ALBUM = 12115
MEDIUM_REFUSAL_ALBUM = 12116
INVALID_MEDIUM_ALBUM = 12117
PLACED_ALBUM = 12118
MOVED_ALBUM = 12119
UNKNOWN_RECORDING_ALBUM = 12120
UNKNOWN_DESTINATION_ALBUM = 12121
OCCUPIED_ALBUM = 12122
RENAME_ALBUM = 12123
CLEAR_FIELD_ALBUM = 12124
ISRC_ALBUM = 12125
SECOND_ISRC_ALBUM = 12126
MALFORMED_ISRC_ALBUM = 12127
UNKNOWN_TRACK_ALBUM = 12128
TRACK_READ_ALBUM = 12129
TRACK_LIST_ALBUM = 12130
POSITION_ALBUM = 12131
NAMING_ALBUM = 12132
MOVING_ALBUM = 12133
SURVIVOR_ALBUM = 12134
DELETED_SURVIVOR_ALBUM = 12135


RETYPE_RECORDING = 12301
PLACED_RECORDING = 12302
MOVED_RECORDING = 12303
DESTINATION_RECORDING = 12304
OCCUPIED_RECORDING = 12305
OCCUPANT_RECORDING = 12306
RENAMED_RECORDING = 12307
READABLE_RECORDING = 12312
LISTED_RECORDINGS = (12313, 12314)
PLAYLIST_RECORDINGS = (12316, 12317)
APPENDED_RECORDINGS = (12318, 12319)
KNOWN_TARGET_RECORDING = 12320
PLAYLIST_TRACK_RECORDING = 12321
REMOVED_RECORDING = 12322
SURVIVING_RECORDING = 12323
ORDERED_RECORDINGS = (12324, 12325, 12326)
PERMUTATION_RECORDINGS = (12327, 12328, 12329)
DELETED_RECORDING = 12330
DELETED_SURVIVOR_RECORDING = 12331

COVERED_ALBUM_IMAGE = 12601
DETAIL_GROUP_IMAGE = 12606
FREE_GROUP_IMAGE = 12604
TRACK_ALBUM_IMAGE = 12603
RECORDING_FILE = 12501

DEFAULT_ALBUM = 0
DEFAULT_GROUP = 0
UNKNOWN_ID = 999_999_999
DIGITAL = "DIGITAL"

_ACCOUNTS = count()


@scenario("SCN-MUSIC-001-A")
def test_the_album_collection_carries_partial_albums(server: Sut, root_token: str) -> None:
    """The collection carries partial albums, the name filter narrows it, and covers follow their links."""
    response = list_albums.sync_detailed(client=server.client(root_token), name="Collected Album")
    assert response.status_code == 200, f"list failed: {response.status_code} {response.content!r}"
    items = json.loads(response.content)["_embedded"]["items"]

    assert [item["id"] for item in items] == [COLLECTION_COVERED, COLLECTION_BARE]
    assert set(items[0]) == {"cover_image_id", "id", "name", "_links"}
    assert items[0]["cover_image_id"] == COVERED_ALBUM_IMAGE
    cover = server.follow(items[0], "cover", token=root_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == COVERED_ALBUM_IMAGE
    assert items[1].get("cover_image_id") is None
    assert items[1]["_links"].get("cover") is None


@scenario("SCN-MUSIC-001-B")
def test_the_album_collection_filters_on_name(server: Sut, root_token: str) -> None:
    """The name filter returns only the album whose name matches."""
    response = list_albums.sync_detailed(client=server.client(root_token), name="Filtered Album")
    assert response.status_code == 200, f"list failed: {response.status_code} {response.content!r}"
    assert [item["id"] for item in json.loads(response.content)["_embedded"]["items"]] == [FILTERED_ALBUM]


@scenario("SCN-MUSIC-002-A")
def test_an_album_detail_read_carries_its_metadata(server: Sut, root_token: str) -> None:
    """An album read carries its identifier, group, metadata, cover and mediums."""
    response = get_album.sync_detailed(DETAIL_ALBUM, client=server.client(root_token))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    album = json.loads(response.content)

    assert album["id"] == DETAIL_ALBUM
    assert album["name"] == "Detailed Album"
    assert album["release_date"] == "1970-01-01"
    assert album["production_type"] == "LIVE"
    assert album["genres"] == ["JAZZ"]
    assert album["cover_image_id"] == 12602
    assert album["track_count"] == 0
    assert [(medium["index"], medium["type"]) for medium in album["mediums"]] == [(1, "CD")]
    assert album["_links"]["group"]["href"].endswith(f"/{DETAIL_GROUP}")


@scenario("SCN-MUSIC-002-B")
def test_an_album_detail_read_omits_unreported_values(server: Sut, root_token: str) -> None:
    """An album that stores no optional value reports null and no cover link."""
    response = get_album.sync_detailed(BARE_ALBUM, client=server.client(root_token))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    album = json.loads(response.content)

    assert album.get("release_date") is None
    assert album.get("production_type") is None
    assert album.get("cover_image_id") is None
    assert album["_links"].get("cover") is None


@scenario("SCN-MUSIC-002-C")
@scenario("SCN-MUSIC-002-D")
def test_an_unknown_or_malformed_album_is_refused(server: Sut, root_token: str) -> None:
    """An unknown album answers 404 and a malformed identifier answers 400."""
    unknown = get_album.sync_detailed(UNKNOWN_ID, client=server.client(root_token))
    malformed = get_album.sync_detailed("not-an-identifier", client=server.client(root_token))

    assert unknown.status_code == 404, f"unknown album answered {unknown.status_code}"
    assert malformed.status_code == 400, f"malformed album answered {malformed.status_code}"


@scenario("SCN-MUSIC-008-A")
def test_a_new_album_is_created(server: Sut, root_token: str) -> None:
    """An album is created from a name and group and holds no medium yet."""
    response = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(music_group_id=NEW_ALBUM_GROUP, name="Brand New Album"),
    )
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    album = json.loads(response.content)

    assert album["name"] == "Brand New Album"
    assert album["genres"] == ["UNKNOWN"]
    assert album["mediums"] == []
    assert album["track_count"] == 0
    assert album.get("cover_image_id") is None
    assert album["_links"]["group"]["href"].endswith(f"/{NEW_ALBUM_GROUP}")


@scenario("SCN-MUSIC-008-C")
def test_an_unnamed_genre_is_stored_as_unknown(server: Sut, root_token: str) -> None:
    """A creation that names no genre stores the UNKNOWN genre."""
    response = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(music_group_id=UNNAMED_GENRE_GROUP, name="Unnamed Genre Album"),
    )
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["genres"] == ["UNKNOWN"]


@scenario("SCN-MUSIC-008-B")
def test_an_existing_album_is_returned_unchanged(server: Sut, root_token: str) -> None:
    """The same name and group answers 200 with the stored album and stores no second one."""
    response = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(music_group_id=EXISTING_ALBUM_GROUP, name="Existing Album"),
    )
    assert response.status_code == 200, f"repeat failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["id"] == EXISTING_ALBUM

    listed = list_albums.sync_detailed(client=server.client(root_token), name="Existing Album")
    assert [item["id"] for item in json.loads(listed.content)["_embedded"]["items"]] == [EXISTING_ALBUM]


@scenario("SCN-MUSIC-008-D")
def test_an_album_with_an_unusable_name_is_refused(server: Sut, root_token: str) -> None:
    """An empty or over-long name is refused on creation and update and stores nothing."""
    empty = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(music_group_id=REFUSAL_GROUP, name=""),
    )
    too_long = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(music_group_id=REFUSAL_GROUP, name="x" * 101),
    )
    padded = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(music_group_id=REFUSAL_GROUP, name=" Padded Album"),
    )
    updated = patch_album.sync_detailed(
        ABSENT_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(name=""),
    )
    padded_update = patch_album.sync_detailed(
        ABSENT_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(name="Kept Album "),
    )

    assert empty.status_code == 422, f"empty name accepted: {empty.status_code}"
    assert too_long.status_code == 422, f"over-long name accepted: {too_long.status_code}"
    assert padded.status_code == 422, f"padded name accepted: {padded.status_code}"
    assert updated.status_code == 422, f"empty update name accepted: {updated.status_code}"
    assert padded_update.status_code == 422, f"padded update name accepted: {padded_update.status_code}"
    stored = get_album.sync_detailed(ABSENT_ALBUM, client=server.client(root_token))
    assert json.loads(stored.content)["name"] == "Kept Album"


@scenario("SCN-MUSIC-008-D")
def test_an_album_with_an_unknown_group_is_refused(server: Sut, root_token: str) -> None:
    """A creation or update naming an unknown group answers 422 and stores nothing."""
    name = "Orphan Album"
    created = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(music_group_id=UNKNOWN_ID, name=name),
    )
    updated = patch_album.sync_detailed(
        ABSENT_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(music_group_id=UNKNOWN_ID),
    )

    assert created.status_code == 422, f"unknown group accepted: {created.status_code}"
    assert updated.status_code == 422, f"unknown group update accepted: {updated.status_code}"
    listed = list_albums.sync_detailed(client=server.client(root_token), name=name)
    assert json.loads(listed.content)["_embedded"]["items"] == []


@scenario("SCN-MUSIC-008-D")
def test_an_album_with_an_empty_genre_is_refused(server: Sut, root_token: str) -> None:
    """A creation or update naming an empty genre answers 422 and stores nothing."""
    name = "Empty Genre Album"
    created = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(music_group_id=REFUSAL_GROUP, name=name, genres=[""]),
    )
    updated = patch_album.sync_detailed(
        ABSENT_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(genres=[""]),
    )

    assert created.status_code == 422, f"empty genre accepted: {created.status_code}"
    assert updated.status_code == 422, f"empty genre update accepted: {updated.status_code}"
    listed = list_albums.sync_detailed(client=server.client(root_token), name=name)
    assert json.loads(listed.content)["_embedded"]["items"] == []


@scenario("SCN-MUSIC-029-A")
def test_an_albums_release_date_and_genres_are_updated(server: Sut, root_token: str) -> None:
    """A new release date and genre list are stored and reported."""
    response = patch_album.sync_detailed(
        UPDATED_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(release_date=datetime.date(1959, 8, 17), genres=["Jazz", "Modal"]),
    )
    assert response.status_code == 200, f"update failed: {response.status_code} {response.content!r}"
    album = json.loads(response.content)

    assert album["release_date"] == "1959-08-17"
    assert set(album["genres"]) == {"JAZZ", "MODAL"}
    stored = json.loads(get_album.sync_detailed(UPDATED_ALBUM, client=server.client(root_token)).content)
    assert stored["release_date"] == "1959-08-17"
    assert set(stored["genres"]) == {"JAZZ", "MODAL"}


@scenario("SCN-MUSIC-029-B")
def test_an_albums_optional_value_is_cleared(server: Sut, root_token: str) -> None:
    """A null release date clears the stored value."""
    response = patch_album.sync_detailed(
        CLEARED_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(release_date=None),
    )
    assert response.status_code == 200, f"clear failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content).get("release_date") is None
    stored = get_album.sync_detailed(CLEARED_ALBUM, client=server.client(root_token))
    assert json.loads(stored.content).get("release_date") is None


@scenario("SCN-MUSIC-029-C")
def test_an_album_group_change_that_collides_is_refused(server: Sut, root_token: str) -> None:
    """Moving an album onto another album's name and group answers 409 and applies nothing."""
    response = patch_album.sync_detailed(
        COLLIDE_SECOND_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(music_group_id=COLLIDE_FIRST_GROUP),
    )
    assert response.status_code == 409, f"collision accepted: {response.status_code} {response.content!r}"
    stored = json.loads(get_album.sync_detailed(COLLIDE_SECOND_ALBUM, client=server.client(root_token)).content)
    assert stored["_links"]["group"]["href"].endswith(f"/{COLLIDE_SECOND_GROUP}")


@scenario("SCN-MUSIC-029-D")
def test_an_absent_album_field_keeps_its_value(server: Sut, root_token: str) -> None:
    """A request that names no production type leaves the stored production type unchanged."""
    response = patch_album.sync_detailed(
        ABSENT_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(name="Renamed Kept Album"),
    )
    assert response.status_code == 200, f"update failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["production_type"] == "LIVE"


@scenario("SCN-MUSIC-003-B")
@scenario("SCN-MUSIC-009-A")
def test_a_medium_is_created(server: Sut, root_token: str) -> None:
    """A medium is created at the named index and read back."""
    created = put_album_medium.sync_detailed(
        MEDIUM_ALBUM,
        1,
        client=server.client(root_token),
        body=PutAlbumMediumRequest(type_="CD"),
    )
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"
    assert json.loads(created.content)["type"] == "CD"

    read = get_album_medium.sync_detailed(MEDIUM_ALBUM, 1, client=server.client(root_token))
    assert read.status_code == 200, f"read failed: {read.status_code}"
    assert json.loads(read.content)["index"] == 1
    assert json.loads(read.content)["type"] == "CD"


@scenario("SCN-MUSIC-009-B")
def test_a_medium_is_retyped(server: Sut, root_token: str) -> None:
    """A medium is retyped at the same index and keeps its tracks."""
    response = put_album_medium.sync_detailed(
        RETYPE_ALBUM,
        1,
        client=server.client(root_token),
        body=PutAlbumMediumRequest(type_="DIGITAL"),
    )
    assert response.status_code == 200, f"retype failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["type"] == "DIGITAL"

    track = get_album_track.sync_detailed(RETYPE_ALBUM, 1, 0, client=server.client(root_token))
    assert track.status_code == 200, f"the retype dropped its track: {track.status_code}"
    assert json.loads(track.content)["recording_id"] == RETYPE_RECORDING


@scenario("SCN-MUSIC-009-C")
def test_a_medium_write_is_idempotent(server: Sut, root_token: str) -> None:
    """Repeating the stored type changes nothing and answers 200."""
    first = put_album_medium.sync_detailed(
        IDEMPOTENT_ALBUM,
        1,
        client=server.client(root_token),
        body=PutAlbumMediumRequest(type_=DIGITAL),
    )
    second = put_album_medium.sync_detailed(
        IDEMPOTENT_ALBUM,
        1,
        client=server.client(root_token),
        body=PutAlbumMediumRequest(type_=DIGITAL),
    )

    assert first.status_code == 200, f"first write failed: {first.status_code}"
    assert second.status_code == 200, f"repeat failed: {second.status_code}"
    assert json.loads(second.content)["type"] == DIGITAL


@scenario("SCN-MUSIC-004-A")
def test_a_medium_read_carries_its_type_and_index(server: Sut, root_token: str) -> None:
    """A medium read carries its album-scoped index, type and track count."""
    response = get_album_medium.sync_detailed(MEDIUM_READ_ALBUM, 1, client=server.client(root_token))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    medium = json.loads(response.content)

    assert medium["index"] == 1
    assert medium["type"] == "CD"
    assert medium["track_count"] == 0


@scenario("SCN-MUSIC-004-B")
@scenario("SCN-MUSIC-004-C")
def test_an_unknown_or_malformed_medium_is_refused(server: Sut, root_token: str) -> None:
    """An unknown album or index answers 404, a malformed index 400 and an out-of-range one 422."""
    unknown_album = get_album_medium.sync_detailed(UNKNOWN_ID, 1, client=server.client(root_token))
    unknown_index = get_album_medium.sync_detailed(MEDIUM_REFUSAL_ALBUM, 2, client=server.client(root_token))
    malformed = get_album_medium.sync_detailed(MEDIUM_REFUSAL_ALBUM, "x", client=server.client(root_token))
    out_of_range = get_album_medium.sync_detailed(MEDIUM_REFUSAL_ALBUM, 256, client=server.client(root_token))

    assert unknown_album.status_code == 404, f"unknown album answered {unknown_album.status_code}"
    assert unknown_index.status_code == 404, f"unknown index answered {unknown_index.status_code}"
    assert malformed.status_code == 400, f"malformed index answered {malformed.status_code}"
    assert out_of_range.status_code == 422, f"out-of-range index answered {out_of_range.status_code}"


@scenario("SCN-MUSIC-009-D")
def test_an_invalid_medium_type_is_refused(server: Sut, root_token: str) -> None:
    """An unsupported type and the default album are refused and store no medium."""
    invalid = put_album_medium.sync_detailed(
        INVALID_MEDIUM_ALBUM,
        1,
        client=server.client(root_token),
        body=PutAlbumMediumRequest(type_="VINYL"),
    )
    protected = put_album_medium.sync_detailed(
        DEFAULT_ALBUM,
        1,
        client=server.client(root_token),
        body=PutAlbumMediumRequest(type_="CD"),
    )

    assert invalid.status_code == 422, f"invalid type accepted: {invalid.status_code}"
    assert protected.status_code == 422, f"default album write accepted: {protected.status_code}"
    assert get_album_medium.sync_detailed(INVALID_MEDIUM_ALBUM, 1, client=server.client(root_token)).status_code == 404


@scenario("SCN-MUSIC-010-A")
def test_a_recording_is_placed(server: Sut, root_token: str) -> None:
    """A recording is placed at an unoccupied destination."""
    response = put_album_track.sync_detailed(
        PLACED_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PutAlbumTrackRequest(recording_id=PLACED_RECORDING),
    )
    assert response.status_code in {200, 201}, f"placement failed: {response.status_code} {response.content!r}"
    track = get_album_track.sync_detailed(PLACED_ALBUM, 1, 0, client=server.client(root_token))
    assert track.status_code == 200, f"placed track unreadable: {track.status_code}"
    assert json.loads(track.content)["recording_id"] == PLACED_RECORDING


@scenario("SCN-MUSIC-010-B")
def test_a_recording_is_moved(server: Sut, root_token: str) -> None:
    """A recording moves to a new destination and leaves its previous placement empty."""
    response = put_album_track.sync_detailed(
        MOVED_ALBUM,
        1,
        1,
        client=server.client(root_token),
        body=PutAlbumTrackRequest(recording_id=MOVED_RECORDING),
    )
    assert response.status_code == 200, f"move failed: {response.status_code} {response.content!r}"
    assert get_album_track.sync_detailed(MOVED_ALBUM, 1, 0, client=server.client(root_token)).status_code == 404
    moved = get_album_track.sync_detailed(MOVED_ALBUM, 1, 1, client=server.client(root_token))
    assert moved.status_code == 200, f"moved track unreadable: {moved.status_code}"
    assert json.loads(moved.content)["recording_id"] == MOVED_RECORDING


@scenario("SCN-MUSIC-010-C")
def test_a_placement_of_an_unknown_recording_is_refused(server: Sut, root_token: str) -> None:
    """A placement naming no recording answers 422 and stores nothing."""
    response = put_album_track.sync_detailed(
        UNKNOWN_RECORDING_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PutAlbumTrackRequest(recording_id=UNKNOWN_ID),
    )
    assert response.status_code == 422, f"unknown recording accepted: {response.status_code}"
    assert (
        get_album_track.sync_detailed(UNKNOWN_RECORDING_ALBUM, 1, 0, client=server.client(root_token)).status_code
        == 404
    )


@scenario("SCN-MUSIC-010-D")
def test_a_placement_into_an_unknown_destination_is_refused(server: Sut, root_token: str) -> None:
    """A placement into an unknown medium or album answers 404 and stores nothing."""
    unknown_medium = put_album_track.sync_detailed(
        UNKNOWN_DESTINATION_ALBUM,
        2,
        0,
        client=server.client(root_token),
        body=PutAlbumTrackRequest(recording_id=DESTINATION_RECORDING),
    )
    unknown_album = put_album_track.sync_detailed(
        UNKNOWN_ID,
        1,
        0,
        client=server.client(root_token),
        body=PutAlbumTrackRequest(recording_id=DESTINATION_RECORDING),
    )

    assert unknown_medium.status_code == 404, f"unknown medium answered {unknown_medium.status_code}"
    assert unknown_album.status_code == 404, f"unknown album answered {unknown_album.status_code}"


@scenario("SCN-MUSIC-010-E")
def test_a_placement_into_an_occupied_destination_is_refused(server: Sut, root_token: str) -> None:
    """An occupied destination answers 409 and the default album refuses the write."""
    occupied = put_album_track.sync_detailed(
        OCCUPIED_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PutAlbumTrackRequest(recording_id=OCCUPANT_RECORDING),
    )
    protected = put_album_track.sync_detailed(
        DEFAULT_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PutAlbumTrackRequest(recording_id=OCCUPANT_RECORDING),
    )

    assert occupied.status_code == 409, f"occupied destination accepted: {occupied.status_code}"
    assert protected.status_code in {409, 422}, f"default album accepted: {protected.status_code}"
    stored = get_album_track.sync_detailed(OCCUPIED_ALBUM, 1, 0, client=server.client(root_token))
    assert json.loads(stored.content)["recording_id"] == OCCUPIED_RECORDING


@scenario("SCN-MUSIC-011-A")
def test_a_recording_rename_reaches_the_playlist(server: Sut, root_token: str) -> None:
    """Renaming a recording through a track reaches every playlist entry that references it."""
    alice_id, alice = provision(
        server,
        root_token,
        f"listener{next(_ACCOUNTS)}",
    )
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name=f"Rename Playlist {alice_id}"),
        ).content
    )
    appended = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=RENAMED_RECORDING)),
    )
    assert appended.status_code == 200, f"append failed: {appended.status_code} {appended.content!r}"

    renamed = patch_album_track.sync_detailed(
        RENAME_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(name="Renamed Song"),
    )
    assert renamed.status_code == 200, f"rename failed: {renamed.status_code} {renamed.content!r}"
    assert json.loads(renamed.content)["name"] == "Renamed Song"

    entry = get_playlist_track.sync_detailed(playlist["id"], 0, client=server.client(alice))
    assert entry.status_code == 200, f"playlist track unreadable: {entry.status_code}"
    assert json.loads(entry.content)["name"] == "Renamed Song"


@scenario("SCN-MUSIC-011-B")
def test_a_recording_field_is_cleared(server: Sut, root_token: str) -> None:
    """A null name and ISRC clear the stored values."""
    response = patch_album_track.sync_detailed(
        CLEAR_FIELD_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(name=None, isrc_code=None),
    )
    assert response.status_code == 200, f"clear failed: {response.status_code} {response.content!r}"
    track = json.loads(response.content)

    assert track["name"] is None
    assert track.get("isrc_code") is None
    stored = get_album_track.sync_detailed(CLEAR_FIELD_ALBUM, 1, 0, client=server.client(root_token))
    assert json.loads(stored.content)["name"] is None


@scenario("SCN-MUSIC-011-C")
def test_a_duplicate_isrc_is_refused(server: Sut, root_token: str) -> None:
    """An ISRC already held by another recording answers 409."""
    first = patch_album_track.sync_detailed(
        ISRC_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(isrc_code="USRC17607899"),
    )
    duplicate = patch_album_track.sync_detailed(
        SECOND_ISRC_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(isrc_code="USRC17607899"),
    )

    assert first.status_code == 200, f"isrc write failed: {first.status_code} {first.content!r}"
    assert duplicate.status_code == 409, f"duplicate isrc accepted: {duplicate.status_code}"
    stored = get_album_track.sync_detailed(SECOND_ISRC_ALBUM, 1, 0, client=server.client(root_token))
    assert json.loads(stored.content).get("isrc_code") is None


@scenario("SCN-MUSIC-011-C")
def test_a_malformed_isrc_is_refused(server: Sut, root_token: str) -> None:
    """An ISRC that is not twelve valid characters answers 422."""
    response = patch_album_track.sync_detailed(
        MALFORMED_ISRC_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(isrc_code="SHORT"),
    )
    assert response.status_code == 422, f"malformed isrc accepted: {response.status_code}"
    stored = get_album_track.sync_detailed(MALFORMED_ISRC_ALBUM, 1, 0, client=server.client(root_token))
    assert json.loads(stored.content).get("isrc_code") is None


@scenario("SCN-MUSIC-011-D")
def test_an_unknown_track_is_refused(server: Sut, root_token: str) -> None:
    """An unknown album, medium or position answers 404."""
    unknown_track = patch_album_track.sync_detailed(
        UNKNOWN_TRACK_ALBUM,
        1,
        9,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(name="x"),
    )
    unknown_album = patch_album_track.sync_detailed(
        UNKNOWN_ID,
        1,
        0,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(name="x"),
    )
    unknown_medium = patch_album_track.sync_detailed(
        UNKNOWN_TRACK_ALBUM,
        9,
        0,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(name="x"),
    )

    assert unknown_track.status_code == 404, f"unknown track answered {unknown_track.status_code}"
    assert unknown_album.status_code == 404, f"unknown album answered {unknown_album.status_code}"
    assert unknown_medium.status_code == 404, f"unknown medium answered {unknown_medium.status_code}"


@scenario("SCN-MUSIC-011-E")
def test_editing_a_default_album_track_is_refused(server: Sut, root_token: str) -> None:
    """A track edit on the default album answers 422 and stores no track."""
    protected = patch_album_track.sync_detailed(
        DEFAULT_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PatchAlbumTrackRequest(name="x"),
    )

    assert protected.status_code == 422, f"default album edit answered {protected.status_code}"
    stored = get_album_track.sync_detailed(DEFAULT_ALBUM, 1, 0, client=server.client(root_token))
    assert stored.status_code == 404, f"protected album gained a track: {stored.status_code}"


@scenario("SCN-MUSIC-007-A")
@scenario("SCN-MUSIC-006-A")
def test_a_track_read_carries_its_recording(server: Sut, root_token: str) -> None:
    """A track read carries its position, recording, file, links and no artwork of its own."""
    response = get_album_track.sync_detailed(TRACK_READ_ALBUM, 1, 0, client=server.client(root_token))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    track = json.loads(response.content)

    assert track["id"] == 0
    assert track["medium_index"] == 1
    assert track["recording_id"] == READABLE_RECORDING
    assert track["file_id"] == RECORDING_FILE
    assert track["name"] == "Readable Track"
    assert track["isrc_code"] == "USRC17607842"
    assert track["_links"]["album"]["href"].endswith(f"/{TRACK_READ_ALBUM}")
    assert track["_links"]["medium"]["href"].endswith("/medium/1")
    assert "cover_image_id" not in track
    assert track["_links"].get("cover") is None

    album = json.loads(get_album.sync_detailed(TRACK_READ_ALBUM, client=server.client(root_token)).content)
    assert album["cover_image_id"] == TRACK_ALBUM_IMAGE
    cover = server.follow(album, "cover", token=root_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == TRACK_ALBUM_IMAGE


@scenario("SCN-MUSIC-005-A")
def test_a_mediums_tracks_are_listed(server: Sut, root_token: str) -> None:
    """The tracks of a medium are listed in ascending index order."""
    response = get_album_track_list.sync_detailed(TRACK_LIST_ALBUM, 1, client=server.client(root_token))
    assert response.status_code == 200, f"list failed: {response.status_code} {response.content!r}"
    items = json.loads(response.content)["items"]

    assert [item["id"] for item in items] == [0, 1]
    assert [item["recording_id"] for item in items] == list(LISTED_RECORDINGS)
    assert all("cover_image_id" not in item for item in items)


@scenario("SCN-MUSIC-005-B")
@scenario("SCN-MUSIC-006-B")
@scenario("SCN-MUSIC-006-C")
def test_an_unknown_or_unsequenced_track_position_is_refused(server: Sut, root_token: str) -> None:
    """An unsequenced position, unknown medium, or malformed index is refused."""
    unsequenced = get_album_track.sync_detailed(POSITION_ALBUM, 1, 1, client=server.client(root_token))
    unknown_medium = get_album_track.sync_detailed(POSITION_ALBUM, 2, 0, client=server.client(root_token))
    malformed_medium = get_album_track.sync_detailed(POSITION_ALBUM, "x", 0, client=server.client(root_token))
    malformed_track = get_album_track.sync_detailed(POSITION_ALBUM, 1, "y", client=server.client(root_token))

    assert unsequenced.status_code == 404, f"unsequenced position answered {unsequenced.status_code}"
    assert unknown_medium.status_code == 404, f"unknown medium answered {unknown_medium.status_code}"
    assert malformed_medium.status_code == 400, f"malformed medium answered {malformed_medium.status_code}"
    assert malformed_track.status_code == 400, f"malformed track answered {malformed_track.status_code}"


@scenario("SCN-MUSIC-014-A")
def test_a_group_is_created(server: Sut, root_token: str) -> None:
    """A group is created with a valid name and no cover."""
    response = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name="Brand New Group"),
    )
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    group = json.loads(response.content)

    assert group["name"] == "Brand New Group"
    assert group.get("cover_image_id") is None
    assert group["_links"].get("cover") is None


@scenario("SCN-MUSIC-014-B")
def test_a_group_stores_its_library_artwork(server: Sut, root_token: str) -> None:
    """A creation naming an existing LIBRARY image stores it as the cover."""
    response = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name="Artwork Group", cover_image_id=FREE_GROUP_IMAGE),
    )
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    group = json.loads(response.content)

    assert group["cover_image_id"] == FREE_GROUP_IMAGE
    cover = server.follow(group, "cover", token=root_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == FREE_GROUP_IMAGE


@scenario("SCN-MUSIC-014-C")
def test_a_duplicate_group_name_is_refused(server: Sut, root_token: str) -> None:
    """A name another group holds answers 409 and creates nothing."""
    response = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name="Duplicate Group"),
    )
    assert response.status_code == 409, f"duplicate group accepted: {response.status_code}"


@scenario("SCN-MUSIC-014-D")
def test_a_group_with_an_invalid_name_is_refused(server: Sut, root_token: str) -> None:
    """An empty or over-long name answers 422."""
    empty = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name="   "),
    )
    too_long = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name="x" * 101),
    )
    padded = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name="Padded Group "),
    )

    assert empty.status_code == 422, f"empty name accepted: {empty.status_code}"
    assert too_long.status_code == 422, f"over-long name accepted: {too_long.status_code}"
    assert padded.status_code == 422, f"padded name accepted: {padded.status_code}"


@scenario("SCN-MUSIC-014-D")
def test_a_group_with_unusable_artwork_is_refused(server: Sut, root_token: str) -> None:
    """A cover naming no image answers 409."""
    response = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name="Unusable Artwork Group", cover_image_id=UNKNOWN_ID),
    )
    assert response.status_code == 409, f"unusable artwork accepted: {response.status_code}"


@scenario("SCN-MUSIC-012-A")
@scenario("SCN-MUSIC-012-B")
def test_the_group_collection_lists_and_filters(server: Sut, root_token: str) -> None:
    """The collection carries partial groups and the name filter narrows it."""
    listed = list_music_groups.sync_detailed(client=server.client(root_token), name="Listed One")
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
    items = json.loads(listed.content)["_embedded"]["items"]

    assert [item["id"] for item in items] == [LISTED_ONE]
    assert set(items[0]) == {"cover_image_id", "id", "name", "_links"}

    filtered = list_music_groups.sync_detailed(client=server.client(root_token), name="Listed Two")
    assert [item["id"] for item in json.loads(filtered.content)["_embedded"]["items"]] == [LISTED_TWO]


@scenario("SCN-MUSIC-013-A")
def test_a_group_detail_read_carries_its_representation(server: Sut, root_token: str) -> None:
    """A group read carries its identifier, name, cover and cover link."""
    response = get_music_group.sync_detailed(DETAIL_GROUP_ENTITY, client=server.client(root_token))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    group = json.loads(response.content)

    assert group["id"] == DETAIL_GROUP_ENTITY
    assert group["name"] == "Detail Group"
    assert group["cover_image_id"] == DETAIL_GROUP_IMAGE
    cover = server.follow(group, "cover", token=root_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == DETAIL_GROUP_IMAGE


@scenario("SCN-MUSIC-013-B")
def test_an_unknown_group_is_refused(server: Sut, root_token: str) -> None:
    """An unknown group answers 404."""
    response = get_music_group.sync_detailed(UNKNOWN_ID, client=server.client(root_token))
    assert response.status_code == 404, f"unknown group answered {response.status_code}"


@scenario("SCN-MUSIC-015-A")
def test_a_group_is_renamed_keeping_its_artwork(server: Sut, root_token: str) -> None:
    """A rename replaces the name and keeps the stored cover."""
    response = patch_music_group.sync_detailed(
        RENAME_KEEP_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(name="Renamed Keep Group"),
    )
    assert response.status_code == 200, f"rename failed: {response.status_code} {response.content!r}"
    group = json.loads(response.content)

    assert group["name"] == "Renamed Keep Group"
    assert group["cover_image_id"] == COVERED_ALBUM_IMAGE


@scenario("SCN-MUSIC-015-B")
def test_group_artwork_is_replaced(server: Sut, root_token: str) -> None:
    """A supplied LIBRARY image replaces the stored cover."""
    response = patch_music_group.sync_detailed(
        REPLACED_ARTWORK_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(cover_image_id=TRACK_ALBUM_IMAGE),
    )
    assert response.status_code == 200, f"replace failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["cover_image_id"] == TRACK_ALBUM_IMAGE


@scenario("SCN-MUSIC-015-C")
def test_group_artwork_is_cleared(server: Sut, root_token: str) -> None:
    """A null cover clears the stored cover and drops its link."""
    response = patch_music_group.sync_detailed(
        CLEARED_ARTWORK_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(cover_image_id=None),
    )
    assert response.status_code == 200, f"clear failed: {response.status_code} {response.content!r}"
    group = json.loads(response.content)

    assert group.get("cover_image_id") is None
    assert group["_links"].get("cover") is None


@scenario("SCN-MUSIC-015-D")
def test_a_group_rename_that_collides_is_refused(server: Sut, root_token: str) -> None:
    """A name another group holds answers 409 and applies nothing."""
    response = patch_music_group.sync_detailed(
        RENAMED_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(name="Taken Group"),
    )
    assert response.status_code == 409, f"collision accepted: {response.status_code}"
    stored = json.loads(get_music_group.sync_detailed(RENAMED_GROUP, client=server.client(root_token)).content)
    assert stored["name"] == "Renamed Group"


@scenario("SCN-MUSIC-015-E")
def test_a_group_rename_with_an_invalid_name_is_refused(server: Sut, root_token: str) -> None:
    """An empty or over-long name answers 422 and applies nothing."""
    empty = patch_music_group.sync_detailed(
        INVALID_NAME_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(name="   "),
    )
    too_long = patch_music_group.sync_detailed(
        INVALID_NAME_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(name="x" * 101),
    )
    padded = patch_music_group.sync_detailed(
        INVALID_NAME_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(name=" Renamed Group"),
    )

    assert empty.status_code == 422, f"empty name accepted: {empty.status_code}"
    assert too_long.status_code == 422, f"over-long name accepted: {too_long.status_code}"
    assert padded.status_code == 422, f"padded name accepted: {padded.status_code}"
    stored = json.loads(get_music_group.sync_detailed(INVALID_NAME_GROUP, client=server.client(root_token)).content)
    assert stored["name"] == "Invalid Name Group"


@scenario("SCN-MUSIC-017-A")
def test_the_album_representation_names_its_group(server: Sut, root_token: str) -> None:
    """An album representation carries a group link naming its group."""
    response = get_album.sync_detailed(NAMING_ALBUM, client=server.client(root_token))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["_links"]["group"]["href"].endswith(f"/{ALBUM_GROUP}")


@scenario("SCN-MUSIC-017-B")
def test_an_album_moves_group(server: Sut, root_token: str) -> None:
    """An update naming another group stores the album under it."""
    response = patch_album.sync_detailed(
        MOVING_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(music_group_id=MOVE_TARGET_GROUP),
    )
    assert response.status_code == 200, f"move failed: {response.status_code} {response.content!r}"
    stored = json.loads(get_album.sync_detailed(MOVING_ALBUM, client=server.client(root_token)).content)
    assert stored["_links"]["group"]["href"].endswith(f"/{MOVE_TARGET_GROUP}")


@scenario("SCN-MUSIC-016-A")
def test_the_default_album_and_group_are_readable(server: Sut, root_token: str) -> None:
    """The default album and group answer 200 with their representations."""
    album = get_album.sync_detailed(DEFAULT_ALBUM, client=server.client(root_token))
    group = get_music_group.sync_detailed(DEFAULT_GROUP, client=server.client(root_token))

    assert album.status_code == 200, f"default album unreadable: {album.status_code}"
    assert group.status_code == 200, f"default group unreadable: {group.status_code}"
    assert json.loads(album.content)["name"] == "Unknown"
    assert json.loads(group.content)["name"] == "Unknown"


@scenario("SCN-MUSIC-016-B")
def test_a_default_group_write_is_refused(server: Sut, root_token: str) -> None:
    """A rename or cover replacement on the default group answers 409 and applies nothing."""
    rename = patch_music_group.sync_detailed(
        DEFAULT_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(name="Changed"),
    )
    cover = patch_music_group.sync_detailed(
        DEFAULT_GROUP,
        client=server.client(root_token),
        body=PatchMusicGroupRequest(cover_image_id=FREE_GROUP_IMAGE),
    )

    assert rename.status_code == 409, f"default group renamed: {rename.status_code}"
    assert cover.status_code == 409, f"default group covered: {cover.status_code}"
    stored = json.loads(get_music_group.sync_detailed(DEFAULT_GROUP, client=server.client(root_token)).content)
    assert stored["name"] == "Unknown"


@scenario("SCN-MUSIC-016-C")
def test_a_default_album_write_is_refused(server: Sut, root_token: str) -> None:
    """An update, medium write or track write on the default album is refused and applies nothing."""
    updated = patch_album.sync_detailed(
        DEFAULT_ALBUM,
        client=server.client(root_token),
        body=PatchAlbumRequest(name="Changed"),
    )
    medium = put_album_medium.sync_detailed(
        DEFAULT_ALBUM,
        1,
        client=server.client(root_token),
        body=PutAlbumMediumRequest(type_="CD"),
    )
    track = put_album_track.sync_detailed(
        DEFAULT_ALBUM,
        1,
        0,
        client=server.client(root_token),
        body=PutAlbumTrackRequest(recording_id=PLACED_RECORDING),
    )

    assert updated.status_code in {409, 422}, f"default album updated: {updated.status_code}"
    assert medium.status_code == 422, f"default medium written: {medium.status_code}"
    assert track.status_code in {409, 422}, f"default track written: {track.status_code}"
    stored = json.loads(get_album.sync_detailed(DEFAULT_ALBUM, client=server.client(root_token)).content)
    assert stored["name"] == "Unknown"


@scenario("SCN-MUSIC-019-A")
def test_a_private_playlist_is_created(server: Sut, root_token: str) -> None:
    """A creation with no visibility is private and owned by the caller."""
    alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    response = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="Mine"),
    )
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    playlist = json.loads(response.content)

    assert playlist["name"] == "Mine"
    assert playlist["is_public"] is False
    assert playlist["owner_id"] == alice_id
    assert playlist["tracks"] == []
    assert playlist["track_count"] == 0
    assert playlist.get("cover_image_id") is None


@scenario("SCN-MUSIC-019-B")
def test_a_public_playlist_is_created(server: Sut, root_token: str) -> None:
    """A creation naming is_public stores a public playlist."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    response = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="Shared Playlist", is_public=True),
    )
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["is_public"] is True


@scenario("SCN-MUSIC-019-C")
def test_a_playlist_with_a_malformed_name_is_refused(server: Sut, root_token: str) -> None:
    """An empty or over-long name answers 422 and creates nothing."""
    alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    empty = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="   "),
    )
    too_long = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="x" * 101),
    )
    padded = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="Malformed Playlist "),
    )

    assert empty.status_code == 422, f"empty name accepted: {empty.status_code}"
    assert too_long.status_code == 422, f"over-long name accepted: {too_long.status_code}"
    assert padded.status_code == 422, f"padded name accepted: {padded.status_code}"
    listed = list_playlists.sync_detailed(client=server.client(alice), owner_id=alice_id)
    assert json.loads(listed.content)["items"] == []


@scenario("SCN-MUSIC-030-A")
def test_a_playlist_name_with_surrounding_whitespace_is_refused(server: Sut, root_token: str) -> None:
    """A name that begins or ends with whitespace answers 422 on creation and rename."""
    alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    created = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Kept Playlist"),
        ).content
    )
    for name in ("  Padded Name  ", " Padded Name", "Padded Name "):
        response = post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name=name),
        )
        assert response.status_code == 422, f"{name!r} accepted: {response.status_code} {response.content!r}"
        renamed = patch_playlist.sync_detailed(
            created["id"],
            client=server.client(alice),
            body=PatchPlaylistRequest(name=name),
        )
        assert renamed.status_code == 422, f"{name!r} rename accepted: {renamed.status_code}"

    listed = list_playlists.sync_detailed(client=server.client(alice), owner_id=alice_id)
    assert [item["name"] for item in json.loads(listed.content)["items"]] == ["Kept Playlist"]
    stored = json.loads(get_playlist.sync_detailed(created["id"], client=server.client(alice)).content)
    assert stored["name"] == "Kept Playlist"


@scenario("SCN-MUSIC-030-B")
def test_a_duplicate_playlist_name_is_refused(server: Sut, root_token: str) -> None:
    """A name the caller already owns answers 409."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    first = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="Taken Name"),
    )
    duplicate = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="Taken Name"),
    )

    assert first.status_code == 201, f"creation failed: {first.status_code}"
    assert duplicate.status_code == 409, f"duplicate name accepted: {duplicate.status_code}"


@scenario("SCN-MUSIC-030-C")
def test_two_owners_may_share_a_playlist_name(server: Sut, root_token: str) -> None:
    """Two callers may each hold a playlist with the same name."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    _bob_id, bob = provision(server, root_token, f"bobby{next(_ACCOUNTS)}")
    first = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="Shared Name"),
    )
    second = post_playlist.sync_detailed(
        client=server.client(bob),
        body=PostPlaylistRequest(name="Shared Name"),
    )

    assert first.status_code == 201, f"first creation failed: {first.status_code}"
    assert second.status_code == 201, f"second creation failed: {second.status_code}"
    assert json.loads(first.content)["id"] != json.loads(second.content)["id"]


@scenario("SCN-MUSIC-018-A")
def test_the_playlist_collection_lists_owned_and_public(server: Sut, root_token: str) -> None:
    """The collection filter returns the caller's own playlists and the public ones."""
    alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    _bob_id, bob = provision(server, root_token, f"bobby{next(_ACCOUNTS)}")
    private = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Owned Private"),
        ).content
    )
    public = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Owned Public", is_public=True),
        ).content
    )
    post_playlist.sync_detailed(
        client=server.client(bob),
        body=PostPlaylistRequest(name="Hidden Private"),
    )

    owned = list_playlists.sync_detailed(client=server.client(alice), owner_id=alice_id)
    assert owned.status_code == 200, f"list failed: {owned.status_code} {owned.content!r}"
    assert [item["id"] for item in json.loads(owned.content)["items"]] == [private["id"], public["id"]]

    visible = list_playlists.sync_detailed(client=server.client(alice), is_public=True, name="Owned Public")
    assert [item["id"] for item in json.loads(visible.content)["items"]] == [public["id"]]


@scenario("SCN-MUSIC-018-C")
def test_another_owners_private_playlist_is_filtered_out(server: Sut, root_token: str) -> None:
    """Another owner's private playlist is absent from the caller's collection."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"bobby{next(_ACCOUNTS)}")
    post_playlist.sync_detailed(
        client=server.client(bob),
        body=PostPlaylistRequest(name="Other Owner Private"),
    )

    listed = list_playlists.sync_detailed(client=server.client(alice), owner_id=bob_id)
    assert listed.status_code == 200, f"list failed: {listed.status_code}"
    assert json.loads(listed.content)["items"] == []


@scenario("SCN-MUSIC-018-B")
def test_the_playlist_collection_filters(server: Sut, root_token: str) -> None:
    """The collection applies every filter together."""
    alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    public = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Filter Public", is_public=True),
        ).content
    )
    private = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Filter Private"),
        ).content
    )

    by_public = list_playlists.sync_detailed(client=server.client(alice), is_public=True, name="Filter Public")
    assert [item["id"] for item in json.loads(by_public.content)["items"]] == [public["id"]]

    by_owner = list_playlists.sync_detailed(
        client=server.client(alice),
        owner_id=alice_id,
        is_public=False,
        name="Filter Private",
    )
    assert [item["id"] for item in json.loads(by_owner.content)["items"]] == [private["id"]]


@scenario("SCN-MUSIC-021-A")
@scenario("SCN-MUSIC-021-B")
@scenario("SCN-MUSIC-023-A")
def test_a_playlist_detail_read_carries_its_tracks(server: Sut, root_token: str) -> None:
    """A playlist read carries its metadata and its tracks in ascending index order."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Tracked Playlist"),
        ).content
    )
    for recording_id in PLAYLIST_RECORDINGS:
        appended = patch_playlist.sync_detailed(
            playlist["id"],
            client=server.client(alice),
            body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=recording_id)),
        )
        assert appended.status_code == 200, f"append failed: {appended.status_code} {appended.content!r}"

    detail = get_playlist.sync_detailed(playlist["id"], client=server.client(alice))
    assert detail.status_code == 200, f"read failed: {detail.status_code} {detail.content!r}"
    tracks = json.loads(detail.content)["tracks"]
    assert [track["id"] for track in tracks] == [0, 1]
    assert [track["recording_id"] for track in tracks] == list(PLAYLIST_RECORDINGS)

    alias = get_playlist_track_list.sync_detailed(playlist["id"], client=server.client(alice))
    assert alias.status_code == 200, f"alias failed: {alias.status_code}"
    assert json.loads(alias.content)["tracks"] == tracks


@scenario("SCN-MUSIC-021-C")
def test_an_unknown_or_malformed_playlist_is_refused(server: Sut, root_token: str) -> None:
    """An unknown, malformed or another owner's private playlist answers 404 or 400."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    _bob_id, bob = provision(server, root_token, f"bobby{next(_ACCOUNTS)}")
    hidden = json.loads(
        post_playlist.sync_detailed(
            client=server.client(bob),
            body=PostPlaylistRequest(name="Hidden From Alice"),
        ).content
    )

    unknown = get_playlist.sync_detailed(UNKNOWN_ID, client=server.client(alice))
    malformed = get_playlist.sync_detailed("abc", client=server.client(alice))
    invisible = get_playlist.sync_detailed(hidden["id"], client=server.client(alice))

    assert unknown.status_code == 404, f"unknown playlist answered {unknown.status_code}"
    assert malformed.status_code == 400, f"malformed playlist answered {malformed.status_code}"
    assert invisible.status_code == 404, f"private playlist answered {invisible.status_code}"


@scenario("SCN-MUSIC-024-A")
def test_playlist_metadata_is_replaced(server: Sut, root_token: str) -> None:
    """A metadata update replaces the name and visibility and advances the last-modified time."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Before Rename"),
        ).content
    )
    time.sleep(1.1)

    response = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(name="After Rename", is_public=True),
    )
    assert response.status_code == 200, f"update failed: {response.status_code} {response.content!r}"
    updated = json.loads(response.content)

    assert updated["name"] == "After Rename"
    assert updated["is_public"] is True
    assert updated["last_modified_at"] > playlist["last_modified_at"]
    assert updated["last_modified_at"].endswith("Z"), (
        f"last_modified_at is not RFC 3339 UTC: {updated['last_modified_at']!r}"
    )


@scenario("SCN-MUSIC-024-B")
def test_an_empty_playlist_update_changes_nothing(server: Sut, root_token: str) -> None:
    """An update carrying no metadata field leaves the playlist unchanged."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Unchanged Playlist"),
        ).content
    )
    response = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(),
    )

    assert response.status_code == 200, f"empty update failed: {response.status_code} {response.content!r}"
    updated = json.loads(response.content)
    assert updated["name"] == "Unchanged Playlist"
    assert updated["is_public"] is False


@scenario("SCN-MUSIC-024-C")
def test_a_playlist_rename_that_collides_is_refused(server: Sut, root_token: str) -> None:
    """A rename onto another playlist's name of the same owner answers 409."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    first = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="First Playlist"),
        ).content
    )
    json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Second Playlist"),
        ).content
    )

    response = patch_playlist.sync_detailed(
        first["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(name="Second Playlist"),
    )
    assert response.status_code == 409, f"collision accepted: {response.status_code}"
    stored = json.loads(get_playlist.sync_detailed(first["id"], client=server.client(alice)).content)
    assert stored["name"] == "First Playlist"


@scenario("SCN-MUSIC-024-C")
def test_a_playlist_rename_with_an_invalid_name_is_refused(server: Sut, root_token: str) -> None:
    """An empty or over-long rename answers 422 and applies nothing."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Valid Playlist"),
        ).content
    )

    empty = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(name="   "),
    )
    too_long = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(name="x" * 101),
    )
    padded = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(name="Valid Playlist "),
    )

    assert empty.status_code == 422, f"empty name accepted: {empty.status_code}"
    assert too_long.status_code == 422, f"over-long name accepted: {too_long.status_code}"
    assert padded.status_code == 422, f"padded name accepted: {padded.status_code}"
    stored = json.loads(get_playlist.sync_detailed(playlist["id"], client=server.client(alice)).content)
    assert stored["name"] == "Valid Playlist"


@scenario("SCN-MUSIC-025-A")
def test_a_recording_is_appended(server: Sut, root_token: str) -> None:
    """Two recordings append at contiguous indices."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Append Playlist"),
        ).content
    )
    for recording_id in APPENDED_RECORDINGS:
        appended = patch_playlist.sync_detailed(
            playlist["id"],
            client=server.client(alice),
            body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=recording_id)),
        )
        assert appended.status_code == 200, f"append failed: {appended.status_code} {appended.content!r}"

    stored = json.loads(get_playlist.sync_detailed(playlist["id"], client=server.client(alice)).content)
    assert [track["id"] for track in stored["tracks"]] == [0, 1]
    assert stored["track_count"] == 2


@scenario("SCN-MUSIC-025-C")
def test_an_unusable_target_recording_is_refused(server: Sut, root_token: str) -> None:
    """An unknown recording answers 422 and a duplicate answers 409."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Unusable Target Playlist"),
        ).content
    )
    unknown = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=UNKNOWN_ID)),
    )
    appended = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=KNOWN_TARGET_RECORDING)),
    )
    duplicate = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=KNOWN_TARGET_RECORDING)),
    )

    assert unknown.status_code == 422, f"unknown recording accepted: {unknown.status_code}"
    assert appended.status_code == 200, f"append failed: {appended.status_code}"
    assert duplicate.status_code == 409, f"duplicate recording accepted: {duplicate.status_code}"
    stored = json.loads(get_playlist.sync_detailed(playlist["id"], client=server.client(alice)).content)
    assert [track["recording_id"] for track in stored["tracks"]] == [KNOWN_TARGET_RECORDING]


@scenario("SCN-MUSIC-022-A")
@scenario("SCN-MUSIC-023-B")
def test_a_playlist_track_read_carries_its_recording(server: Sut, root_token: str) -> None:
    """A playlist track carries its recording, file, name, ISRC and playlist link, and no artwork."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Track Representation Playlist"),
        ).content
    )
    patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=PLAYLIST_TRACK_RECORDING)),
    )

    response = get_playlist_track.sync_detailed(playlist["id"], 0, client=server.client(alice))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    track = json.loads(response.content)

    assert track["id"] == 0
    assert track["recording_id"] == PLAYLIST_TRACK_RECORDING
    assert track["file_id"] == RECORDING_FILE
    assert track["name"] == "Track With Name"
    assert track["isrc_code"] == "USRC17607844"
    assert track["_links"]["playlist"]["href"].endswith(f"/{playlist['id']}")
    assert "cover_image_id" not in track
    assert track["_links"].get("cover") is None


@scenario("SCN-MUSIC-022-B")
def test_an_unsequenced_playlist_position_is_refused(server: Sut, root_token: str) -> None:
    """An index the playlist does not sequence answers 404."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Empty Position Playlist"),
        ).content
    )
    response = get_playlist_track.sync_detailed(playlist["id"], 0, client=server.client(alice))
    assert response.status_code == 404, f"unsequenced index answered {response.status_code}"


@scenario("SCN-MUSIC-026-A")
def test_a_track_is_removed(server: Sut, root_token: str) -> None:
    """Removing an occurrence answers 204 and leaves the playlist empty."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Removal Playlist"),
        ).content
    )
    patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=REMOVED_RECORDING)),
    )

    response = delete_playlist_track.sync_detailed(playlist["id"], 0, client=server.client(alice))
    assert response.status_code == 204, f"removal failed: {response.status_code} {response.content!r}"
    stored = json.loads(get_playlist.sync_detailed(playlist["id"], client=server.client(alice)).content)
    assert stored["tracks"] == []
    assert get_playlist_track.sync_detailed(playlist["id"], 0, client=server.client(alice)).status_code == 404


@scenario("SCN-MUSIC-026-B")
def test_a_removed_tracks_recording_survives(server: Sut, root_token: str) -> None:
    """A removed occurrence leaves the recording readable through its album."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Survivor Removal Playlist"),
        ).content
    )
    patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=SURVIVING_RECORDING)),
    )

    removed = delete_playlist_track.sync_detailed(playlist["id"], 0, client=server.client(alice))
    assert removed.status_code == 204, f"removal failed: {removed.status_code}"
    track = get_album_track.sync_detailed(SURVIVOR_ALBUM, 1, 0, client=server.client(root_token))
    assert track.status_code == 200, f"recording did not survive: {track.status_code}"
    assert json.loads(track.content)["recording_id"] == SURVIVING_RECORDING


@scenario("SCN-MUSIC-026-C")
def test_removing_an_unsequenced_position_is_refused(server: Sut, root_token: str) -> None:
    """Removing an index the playlist does not sequence answers 404."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Empty Removal Playlist"),
        ).content
    )
    response = delete_playlist_track.sync_detailed(playlist["id"], 0, client=server.client(alice))
    assert response.status_code == 404, f"unsequenced removal answered {response.status_code}"


@scenario("SCN-MUSIC-027-A")
def test_the_tracks_of_a_playlist_are_reordered(server: Sut, root_token: str) -> None:
    """A complete permutation reindexes the tracks in the requested order."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Reorder Playlist"),
        ).content
    )
    for recording_id in ORDERED_RECORDINGS:
        patch_playlist.sync_detailed(
            playlist["id"],
            client=server.client(alice),
            body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=recording_id)),
        )

    response = put_playlist_track.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=ReorderPlaylistTracksRequest(order=[2, 0, 1]),
    )
    assert response.status_code == 200, f"reorder failed: {response.status_code} {response.content!r}"
    tracks = json.loads(response.content)["tracks"]

    assert [track["id"] for track in tracks] == [0, 1, 2]
    assert [track["recording_id"] for track in tracks] == [ORDERED_RECORDINGS[2], *ORDERED_RECORDINGS[:2]]


@scenario("SCN-MUSIC-027-B")
def test_an_incomplete_permutation_is_refused(server: Sut, root_token: str) -> None:
    """An incomplete, duplicated or unknown permutation answers 422 and leaves the order unchanged."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Permutation Playlist"),
        ).content
    )
    for recording_id in PERMUTATION_RECORDINGS:
        patch_playlist.sync_detailed(
            playlist["id"],
            client=server.client(alice),
            body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=recording_id)),
        )

    for invalid in ([0, 1], [0, 0, 1], [0, 1, 9]):
        response = put_playlist_track.sync_detailed(
            playlist["id"],
            client=server.client(alice),
            body=ReorderPlaylistTracksRequest(order=invalid),
        )
        assert response.status_code == 422, f"{invalid} accepted: {response.status_code}"

    stored = json.loads(get_playlist.sync_detailed(playlist["id"], client=server.client(alice)).content)
    assert [track["recording_id"] for track in stored["tracks"]] == list(PERMUTATION_RECORDINGS)


@scenario("SCN-MUSIC-028-A")
def test_a_playlist_is_deleted(server: Sut, root_token: str) -> None:
    """Deleting a playlist answers 204 and removes it."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    playlist = json.loads(
        post_playlist.sync_detailed(
            client=server.client(alice),
            body=PostPlaylistRequest(name="Deleted Playlist"),
        ).content
    )
    patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=DELETED_RECORDING)),
    )

    response = delete_playlist.sync_detailed(playlist["id"], client=server.client(alice))
    assert response.status_code == 204, f"delete failed: {response.status_code} {response.content!r}"
    assert get_playlist.sync_detailed(playlist["id"], client=server.client(alice)).status_code == 404


@scenario("SCN-MUSIC-028-B")
def test_a_deleted_playlists_recordings_survive(server: Sut, root_token: str) -> None:
    """A deleted playlist leaves its recordings readable through their album."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    created = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="Survivor Delete Playlist"),
    )
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"
    playlist = json.loads(created.content)
    patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=DELETED_SURVIVOR_RECORDING)),
    )

    response = delete_playlist.sync_detailed(playlist["id"], client=server.client(alice))
    assert response.status_code == 204, f"delete failed: {response.status_code}"
    track = get_album_track.sync_detailed(DELETED_SURVIVOR_ALBUM, 1, 0, client=server.client(root_token))
    assert track.status_code == 200, f"recording did not survive: {track.status_code}"
    assert json.loads(track.content)["recording_id"] == DELETED_SURVIVOR_RECORDING


@scenario("SCN-MUSIC-028-C")
def test_deleting_an_unknown_playlist_is_refused(server: Sut, root_token: str) -> None:
    """Deleting an unknown playlist answers 404."""
    _alice_id, alice = provision(server, root_token, f"alice{next(_ACCOUNTS)}")
    response = delete_playlist.sync_detailed(UNKNOWN_ID, client=server.client(alice))
    assert response.status_code == 404, f"unknown playlist delete answered {response.status_code}"
