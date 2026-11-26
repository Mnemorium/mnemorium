"""Black-box proof of the `cover-art` capability.

A resource reports the artwork it stores and links the image, a resource that owns no artwork reports none and inherits
nothing, and a cover write attaches, replaces or clears exactly the image it names. Every scope claim is observed
through the refusals the scope forces: a fresh upload is accepted, the catalogue refuses it once a playlist fixed it
`USER`, and a playlist refuses it once a catalogue row fixed it `LIBRARY`.
"""

import json
import time

import pytest
from conftest import login, provision, start_instance
from gallery import image_file, video_file
from harness import ROOT_ADMIN_USERNAME, Sut
from labels import TestType, alternate_flow, business_rule, negative, positive, scenario, security
from mnemorium_client.api.asset import get_file
from mnemorium_client.api.library import (
    get_album,
    get_album_track,
    get_music_group,
    get_playlist,
    get_playlist_track,
    get_series_episode,
    get_series_season,
    list_music_groups,
    list_playlists,
    patch_album,
    patch_playlist,
    post_album,
    post_movie,
    post_music_group,
    post_playlist,
    post_series,
    put_series_season,
)
from mnemorium_client.models import (
    PatchAlbumRequest,
    PatchPlaylistRequest,
    PostAlbumRequest,
    PostMovieRequest,
    PostMusicGroupRequest,
    PostPlaylistRequest,
    PostSeriesRequest,
    PutSeriesSeasonRequest,
)

# The seeded slots this module reads, documented in `seed/sql/0001_cover_art.sql`.
COVERED_GROUP = 11001
BARE_GROUP = 11002
COVERED_ALBUM = 11003
BARE_ALBUM = 11004
ALBUM_MEDIUM_INDEX = 1
ALBUM_TRACK_INDEX = 0
PLAYLIST_TRACK_CASE = 11008
REPLACE_PLAYLIST = 11010
CLEAR_PLAYLIST = 11011
SERIES = 11014
SEASON_INDEX = 1
EPISODE_INDEX = 0
COVERED_GROUP_COVER = 11201
COVERED_ALBUM_COVER = 11202
SEASON_COVER = 11204
NEW_PLAYLIST_COVER = 11205
REPLACEMENT_COVER = 11207
GALLERY_IMAGE = 11209
LIBRARY_IMAGE = 11210
UNKNOWN_IMAGE = 999_999_999
# A stored file, not an image: the profile's file block (11101-11113) and image block (11201-11213) never overlap, so
# this identifier names a real file that is not an image row.
SEEDED_FILE = 11101


def image_id(server: Sut, token: str, file: dict) -> int:
    """The identifier of the image the file representation links."""
    response = server.follow(file, "image", token=token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.text}"
    return int(response.json()["id"])


def video_id(server: Sut, token: str, file: dict) -> int:
    """The identifier of the video the file representation links."""
    response = server.follow(file, "video", token=token)
    assert response.status_code == 200, f"video read failed: {response.status_code} {response.text}"
    return int(response.json()["id"])


def unique(prefix: str) -> str:
    """A name no other case on the instance stores."""
    return f"{prefix} {time.time_ns()}"


def refusal(response) -> None:
    """Assert a refusal answers the error envelope."""
    body = json.loads(response.content)
    assert set(body) == {"error", "_links"}, f"unexpected refusal body: {body}"
    assert isinstance(body["error"], str) and body["error"]


@scenario("SCN-COVER-ART-001-A", TestType.POSITIVE)
@scenario("SCN-COVER-ART-004-A", TestType.BUSINESS_RULE)
def test_a_resource_with_its_own_artwork_reports_it(server: Sut, seed_token: str) -> None:
    """A group that stores a cover reports it as its only cover field."""
    response = get_music_group.sync_detailed(COVERED_GROUP, client=server.client(seed_token))
    assert response.status_code == 200, f"group read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body["cover_image_id"] == COVERED_GROUP_COVER
    assert "effective_cover_image_id" not in body
    cover = server.follow(body, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == COVERED_GROUP_COVER


@business_rule
@scenario("SCN-COVER-ART-001-B")
@scenario("SCN-COVER-ART-004-B")
def test_a_resource_without_artwork_reports_null(server: Sut, seed_token: str) -> None:
    """A group with no cover reports null and no cover link."""
    response = get_music_group.sync_detailed(BARE_GROUP, client=server.client(seed_token))
    assert response.status_code == 200, f"group read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body.get("cover_image_id") is None
    assert "effective_cover_image_id" not in body
    assert "cover" not in body["_links"]


@business_rule
@scenario("SCN-COVER-ART-004-A")
def test_a_covered_album_links_its_cover(server: Sut, seed_token: str) -> None:
    """An album with a cover exposes the link to that image."""
    response = get_album.sync_detailed(COVERED_ALBUM, client=server.client(seed_token))
    assert response.status_code == 200, f"album read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body["cover_image_id"] == COVERED_ALBUM_COVER
    cover = server.follow(body, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == COVERED_ALBUM_COVER


@business_rule
@scenario("SCN-COVER-ART-004-B")
def test_a_bare_album_links_no_cover(server: Sut, seed_token: str) -> None:
    """An album without a cover exposes none."""
    response = get_album.sync_detailed(BARE_ALBUM, client=server.client(seed_token))
    assert response.status_code == 200, f"album read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body.get("cover_image_id") is None
    assert "cover" not in body["_links"]


@business_rule
@scenario("SCN-COVER-ART-006-A")
@scenario("SCN-COVER-ART-006-C")
def test_an_album_track_reports_no_artwork(server: Sut, seed_token: str) -> None:
    """An album track carries no cover; the album it belongs to reports one."""
    response = get_album_track.sync_detailed(
        COVERED_ALBUM, ALBUM_MEDIUM_INDEX, ALBUM_TRACK_INDEX, client=server.client(seed_token)
    )
    assert response.status_code == 200, f"track read failed: {response.status_code} {response.content!r}"
    track = json.loads(response.content)
    album_response = get_album.sync_detailed(COVERED_ALBUM, client=server.client(seed_token))
    assert album_response.status_code == 200, f"album read failed: {album_response.status_code}"
    album = json.loads(album_response.content)

    assert "cover_image_id" not in track
    assert "cover" not in track["_links"]
    assert album["cover_image_id"] == COVERED_ALBUM_COVER
    cover = server.follow(album, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == COVERED_ALBUM_COVER


@business_rule
@scenario("SCN-COVER-ART-006-D")
def test_a_playlist_track_reports_no_artwork(server: Sut, seed_token: str) -> None:
    """A playlist track carries no cover; the album its recording belongs to still reports one."""
    response = get_playlist_track.sync_detailed(PLAYLIST_TRACK_CASE, 0, client=server.client(seed_token))
    assert response.status_code == 200, f"track read failed: {response.status_code} {response.content!r}"
    track = json.loads(response.content)
    album_response = get_album.sync_detailed(COVERED_ALBUM, client=server.client(seed_token))
    assert album_response.status_code == 200, f"album read failed: {album_response.status_code}"
    album = json.loads(album_response.content)

    assert "cover_image_id" not in track
    assert "cover" not in track["_links"]
    assert album["cover_image_id"] == COVERED_ALBUM_COVER
    cover = server.follow(album, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == COVERED_ALBUM_COVER


@business_rule
@scenario("SCN-COVER-ART-006-B")
@scenario("SCN-COVER-ART-006-C")
def test_an_episode_reports_no_artwork(server: Sut, seed_token: str) -> None:
    """An episode carries no cover; the season that sequences it reports its own."""
    response = get_series_episode.sync_detailed(SERIES, SEASON_INDEX, EPISODE_INDEX, client=server.client(seed_token))
    assert response.status_code == 200, f"episode read failed: {response.status_code} {response.content!r}"
    episode = json.loads(response.content)
    season_response = get_series_season.sync_detailed(SERIES, SEASON_INDEX, client=server.client(seed_token))
    assert season_response.status_code == 200, f"season read failed: {season_response.status_code}"
    season = json.loads(season_response.content)

    assert "cover_image_id" not in episode
    assert "cover" not in episode["_links"]
    assert season["cover_image_id"] == SEASON_COVER
    cover = server.follow(season, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == SEASON_COVER


@positive
@scenario("SCN-COVER-ART-003-B")
@scenario("SCN-COVER-ART-005-A")
@scenario("SCN-COVER-ART-007-A")
def test_a_new_playlist_stores_its_cover(server: Sut, seed_token: str) -> None:
    """A playlist stores the cover it was created with, reports it and exposes its link."""
    response = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name="Created Cover", cover_image_id=NEW_PLAYLIST_COVER),
    )
    assert response.status_code == 201, f"playlist creation failed: {response.status_code} {response.content!r}"
    created = json.loads(response.content)

    assert created["cover_image_id"] == NEW_PLAYLIST_COVER
    cover = server.follow(created, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == NEW_PLAYLIST_COVER
    stored_response = get_playlist.sync_detailed(created["id"], client=server.client(seed_token))
    assert stored_response.status_code == 200, f"playlist read failed: {stored_response.status_code}"
    assert json.loads(stored_response.content)["cover_image_id"] == NEW_PLAYLIST_COVER


@positive
@scenario("SCN-COVER-ART-005-B")
@scenario("SCN-COVER-ART-007-B")
def test_a_playlist_replaces_its_cover(server: Sut, seed_token: str) -> None:
    """A playlist metadata update replaces the stored cover."""
    response = patch_playlist.sync_detailed(
        REPLACE_PLAYLIST,
        client=server.client(seed_token),
        body=PatchPlaylistRequest(cover_image_id=REPLACEMENT_COVER),
    )
    assert response.status_code == 200, f"playlist update failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body["cover_image_id"] == REPLACEMENT_COVER
    cover = server.follow(body, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == REPLACEMENT_COVER


@alternate_flow
@scenario("SCN-COVER-ART-005-C")
@scenario("SCN-COVER-ART-007-B")
def test_a_playlist_clears_its_cover_with_null(server: Sut, seed_token: str) -> None:
    """A playlist metadata update clears the stored cover with null and drops its link."""
    response = patch_playlist.sync_detailed(
        CLEAR_PLAYLIST,
        client=server.client(seed_token),
        body=PatchPlaylistRequest(cover_image_id=None),
    )
    assert response.status_code == 200, f"playlist update failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body.get("cover_image_id") is None
    assert "cover" not in body["_links"]


@negative
@scenario("SCN-COVER-ART-003-C")
@scenario("SCN-COVER-ART-007-D")
def test_a_playlist_refuses_library_artwork(server: Sut, seed_token: str) -> None:
    """A library image cannot back a playlist, and the refused creation stores nothing."""
    name = "Library Artwork Refused"
    refused = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=name, cover_image_id=LIBRARY_IMAGE),
    )
    listing = list_playlists.sync_detailed(client=server.client(seed_token), name=name, owner_id=10000)

    assert refused.status_code == 409, f"library image accepted: {refused.status_code} {refused.content!r}"
    assert json.loads(listing.content)["items"] == []


@negative
@scenario("SCN-COVER-ART-002-B")
@scenario("SCN-COVER-ART-007-D")
def test_a_playlist_refuses_an_unknown_image(server: Sut, seed_token: str) -> None:
    """A cover that names no image is refused, and the refused creation stores nothing."""
    name = "Unknown Artwork Refused"
    refused = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=name, cover_image_id=UNKNOWN_IMAGE),
    )
    listing = list_playlists.sync_detailed(client=server.client(seed_token), name=name, owner_id=10000)

    assert refused.status_code == 409, f"unknown image accepted: {refused.status_code}"
    assert json.loads(listing.content)["items"] == []


@negative
@scenario("SCN-COVER-ART-003-D")
def test_gallery_media_is_refused_as_artwork(server: Sut, seed_token: str) -> None:
    """An image already used as gallery media cannot back a playlist, and nothing is stored."""
    name = "Gallery Artwork Refused"
    refused = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=name, cover_image_id=GALLERY_IMAGE),
    )
    listing = list_playlists.sync_detailed(client=server.client(seed_token), name=name, owner_id=10000)

    assert refused.status_code == 409, f"gallery image accepted: {refused.status_code} {refused.content!r}"
    assert json.loads(listing.content)["items"] == []


@business_rule
@scenario("SCN-COVER-ART-005-D")
def test_an_update_that_omits_the_cover_keeps_it(server: Sut, seed_token: str, root_token: str) -> None:
    """A patch that names no cover leaves the stored artwork and its link in place."""
    playlist_cover = image_id(server, seed_token, image_file(server, seed_token, "kept-playlist.png"))
    created = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=unique("Kept Playlist"), cover_image_id=playlist_cover),
    )
    assert created.status_code == 201, f"playlist creation failed: {created.status_code} {created.content!r}"
    playlist = json.loads(created.content)

    renamed = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(seed_token),
        body=PatchPlaylistRequest(name=unique("Kept Playlist Renamed")),
    )
    assert renamed.status_code == 200, f"playlist rename failed: {renamed.status_code} {renamed.content!r}"
    body = json.loads(renamed.content)
    assert body["cover_image_id"] == playlist_cover
    cover = server.follow(body, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == playlist_cover
    stored = get_playlist.sync_detailed(playlist["id"], client=server.client(seed_token))
    assert stored.status_code == 200, f"playlist read failed: {stored.status_code}"
    assert json.loads(stored.content)["cover_image_id"] == playlist_cover

    group = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name=unique("Kept Artwork Group")),
    )
    assert group.status_code == 201, f"group creation failed: {group.status_code} {group.content!r}"
    album_cover = image_id(server, root_token, image_file(server, root_token, "kept-album.png"))
    album = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(
            music_group_id=json.loads(group.content)["id"],
            name=unique("Kept Artwork Album"),
            cover_image_id=album_cover,
        ),
    )
    assert album.status_code == 201, f"album creation failed: {album.status_code} {album.content!r}"
    album_body = json.loads(album.content)

    patched = patch_album.sync_detailed(
        album_body["id"],
        client=server.client(root_token),
        body=PatchAlbumRequest(name=unique("Kept Artwork Album Renamed")),
    )
    assert patched.status_code == 200, f"album patch failed: {patched.status_code} {patched.content!r}"
    patched_body = json.loads(patched.content)
    assert patched_body["cover_image_id"] == album_cover
    assert server.follow(patched_body, "cover", token=root_token).status_code == 200


@negative
@scenario("SCN-COVER-ART-002-C")
def test_a_file_identifier_is_not_a_cover(server: Sut, seed_token: str) -> None:
    """A file identifier is refused as a cover and no stored artwork changes."""
    file_response = get_file.sync_detailed(SEEDED_FILE, client=server.client(seed_token))
    assert file_response.status_code == 200, f"file read failed: {file_response.status_code} {file_response.text}"
    assert json.loads(file_response.content)["id"] == SEEDED_FILE
    name = unique("File Cover Refused")

    refused = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=name, cover_image_id=SEEDED_FILE),
    )
    assert refused.status_code == 409, f"file accepted as a cover: {refused.status_code} {refused.content!r}"
    refusal(refused)
    listing = list_playlists.sync_detailed(client=server.client(seed_token), name=name)
    assert json.loads(listing.content)["items"] == []

    cover = image_id(server, seed_token, image_file(server, seed_token, "file-refusal-kept.png"))
    created = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=unique("File Cover Kept"), cover_image_id=cover),
    )
    assert created.status_code == 201, f"playlist creation failed: {created.status_code} {created.content!r}"
    playlist = json.loads(created.content)
    patched = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(seed_token),
        body=PatchPlaylistRequest(cover_image_id=SEEDED_FILE),
    )
    assert patched.status_code == 409, f"file accepted on update: {patched.status_code} {patched.content!r}"
    stored = get_playlist.sync_detailed(playlist["id"], client=server.client(seed_token))
    assert json.loads(stored.content)["cover_image_id"] == cover


@scenario("SCN-COVER-ART-002-A", TestType.POSITIVE)
@scenario("SCN-COVER-ART-003-B", TestType.BUSINESS_RULE)
def test_a_new_image_is_accepted_and_marked_user(server: Sut, seed_token: str, root_token: str) -> None:
    """A fresh image backs a playlist, and the catalogue refuses it as artwork afterwards."""
    cover = image_id(server, seed_token, image_file(server, seed_token, "fresh-user-cover.png"))

    created = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=unique("Fresh Cover"), cover_image_id=cover),
    )
    assert created.status_code == 201, f"fresh image refused: {created.status_code} {created.content!r}"
    playlist = json.loads(created.content)
    assert playlist["cover_image_id"] == cover
    stored = server.follow(playlist, "cover", token=seed_token)
    assert stored.status_code == 200, f"cover read failed: {stored.status_code} {stored.text}"
    assert stored.json()["id"] == cover

    name = unique("User Artwork Group")
    refused = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name=name, cover_image_id=cover),
    )
    assert refused.status_code == 409, f"user image became library artwork: {refused.status_code}"
    listing = list_music_groups.sync_detailed(client=server.client(root_token), name=name)
    assert json.loads(listing.content)["_embedded"]["items"] == []


@negative
@scenario("SCN-COVER-ART-003-E")
def test_a_user_image_backs_one_cover(server: Sut, seed_token: str) -> None:
    """A second playlist cannot name the image the first one stores."""
    cover = image_id(server, seed_token, image_file(server, seed_token, "single-user-cover.png"))
    first = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=unique("First Cover"), cover_image_id=cover),
    )
    assert first.status_code == 201, f"playlist creation failed: {first.status_code} {first.content!r}"
    name = unique("Second Cover")

    second = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=name, cover_image_id=cover),
    )
    assert second.status_code == 409, f"shared image accepted: {second.status_code} {second.content!r}"
    refusal(second)
    listing = list_playlists.sync_detailed(client=server.client(seed_token), name=name)
    assert json.loads(listing.content)["items"] == []

    stored = get_playlist.sync_detailed(json.loads(first.content)["id"], client=server.client(seed_token))
    assert json.loads(stored.content)["cover_image_id"] == cover


@positive
@scenario("SCN-COVER-ART-003-A")
@pytest.mark.own_instance(reason="exact_collection")
def test_a_catalogue_cover_marks_the_image_library(sut: Sut) -> None:
    """Every catalogue resource fixes the fresh image it names to `LIBRARY`."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    attached: list[tuple[str, dict, int]] = []

    group_cover = image_id(server, root_token, image_file(server, root_token, "group-cover.png"))
    group = post_music_group.sync_detailed(
        client=server.client(root_token),
        body=PostMusicGroupRequest(name=unique("Catalogue Group"), cover_image_id=group_cover),
    )
    assert group.status_code == 201, f"group creation failed: {group.status_code} {group.content!r}"
    group_body = json.loads(group.content)
    attached.append(("group", group_body, group_cover))

    album_cover = image_id(server, root_token, image_file(server, root_token, "album-cover.png"))
    album = post_album.sync_detailed(
        client=server.client(root_token),
        body=PostAlbumRequest(
            music_group_id=group_body["id"],
            name=unique("Catalogue Album"),
            cover_image_id=album_cover,
        ),
    )
    assert album.status_code in {200, 201}, f"album creation failed: {album.status_code} {album.content!r}"
    attached.append(("album", json.loads(album.content), album_cover))

    film_cover = image_id(server, root_token, image_file(server, root_token, "film-cover.png"))
    film = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(
            name=unique("Catalogue Film"),
            video_id=video_id(server, root_token, video_file(server, root_token)),
            cover_image_id=film_cover,
        ),
    )
    assert film.status_code in {200, 201}, f"film creation failed: {film.status_code} {film.content!r}"
    attached.append(("film", json.loads(film.content), film_cover))

    show_cover = image_id(server, root_token, image_file(server, root_token, "show-cover.png"))
    show = post_series.sync_detailed(
        client=server.client(root_token),
        body=PostSeriesRequest(name=unique("Catalogue Show"), cover_image_id=show_cover),
    )
    assert show.status_code == 201, f"show creation failed: {show.status_code} {show.content!r}"
    show_body = json.loads(show.content)
    attached.append(("show", show_body, show_cover))

    season_cover = image_id(server, root_token, image_file(server, root_token, "season-cover.png"))
    season = put_series_season.sync_detailed(
        show_body["id"],
        1,
        client=server.client(root_token),
        body=PutSeriesSeasonRequest(name="Season One", cover_image_id=season_cover),
    )
    assert season.status_code == 201, f"season creation failed: {season.status_code} {season.content!r}"
    attached.append(("season", json.loads(season.content), season_cover))

    for label, representation, cover in attached:
        assert representation["cover_image_id"] == cover, f"the {label} does not report its cover"
        response = server.follow(representation, "cover", token=root_token)
        assert response.status_code == 200, f"the {label} cover read failed: {response.status_code}"
        assert response.json()["id"] == cover

    for label, _representation, cover in attached:
        name = unique(f"{label} User Cover")
        refused = post_playlist.sync_detailed(
            client=server.client(root_token),
            body=PostPlaylistRequest(name=name, cover_image_id=cover),
        )
        assert refused.status_code == 409, f"the {label} cover was not fixed to LIBRARY"
        refusal(refused)
        listing = list_playlists.sync_detailed(client=server.client(root_token), name=name)
        assert json.loads(listing.content)["items"] == []


@business_rule
@scenario("SCN-COVER-ART-007-C")
def test_a_playlist_without_a_cover_reports_none(server: Sut, seed_token: str) -> None:
    """A playlist created without artwork reports a null cover and no cover link."""
    created = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=unique("Bare Playlist")),
    )
    assert created.status_code == 201, f"playlist creation failed: {created.status_code} {created.content!r}"
    body = json.loads(created.content)
    assert body.get("cover_image_id") is None
    assert "cover" not in body["_links"]

    stored = get_playlist.sync_detailed(body["id"], client=server.client(seed_token))
    assert stored.status_code == 200, f"playlist read failed: {stored.status_code}"
    stored_body = json.loads(stored.content)
    assert stored_body.get("cover_image_id") is None
    assert "cover" not in stored_body["_links"]


@security
@scenario("SCN-COVER-ART-008-A")
@scenario("SCN-COVER-ART-008-B", TestType.POSITIVE)
@pytest.mark.own_instance(reason="exact_collection")
def test_a_cover_needs_a_viewable_image(sut: Sut) -> None:
    """A caller who may create a playlist cannot attach artwork it may not view, while its own image is stored."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    _alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")
    cover = image_id(server, alice, image_file(server, alice, "alice-cover.png"))
    name = unique("Borrowed Cover")

    refused = post_playlist.sync_detailed(
        client=server.client(bob),
        body=PostPlaylistRequest(name=name, cover_image_id=cover),
    )
    assert refused.status_code == 409, f"another caller's image accepted: {refused.status_code}"
    refusal(refused)
    listing = list_playlists.sync_detailed(client=server.client(bob), name=name)
    assert json.loads(listing.content)["items"] == []

    allowed = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name=unique("Own Cover"), cover_image_id=cover),
    )
    assert allowed.status_code == 201, f"the owner's own image refused: {allowed.status_code}"
    assert json.loads(allowed.content)["cover_image_id"] == cover
