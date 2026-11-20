"""Black-box proof of the `cover-art` capability."""

import json

import allure
from harness import Sut
from mnemorium_client.api.library import (
    get_album,
    get_album_track,
    get_music_group,
    get_playlist,
    get_playlist_track,
    get_series_episode,
    get_series_season,
    list_playlists,
    patch_playlist,
    post_playlist,
)
from mnemorium_client.models import PatchPlaylistRequest, PostPlaylistRequest

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


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-001")
@allure.story("SCN-COVER-ART-001-A")
@allure.feature("REQ-COVER-ART-004")
@allure.story("SCN-COVER-ART-004-A")
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


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-001")
@allure.story("SCN-COVER-ART-001-B")
@allure.feature("REQ-COVER-ART-004")
@allure.story("SCN-COVER-ART-004-B")
def test_a_resource_without_artwork_reports_null(server: Sut, seed_token: str) -> None:
    """A group with no cover reports null and no cover link."""
    response = get_music_group.sync_detailed(BARE_GROUP, client=server.client(seed_token))
    assert response.status_code == 200, f"group read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body.get("cover_image_id") is None
    assert "effective_cover_image_id" not in body
    assert body["_links"].get("cover") is None


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-004")
@allure.story("SCN-COVER-ART-004-A")
def test_a_covered_album_links_its_cover(server: Sut, seed_token: str) -> None:
    """An album with a cover exposes the link to that image."""
    response = get_album.sync_detailed(COVERED_ALBUM, client=server.client(seed_token))
    assert response.status_code == 200, f"album read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body["cover_image_id"] == COVERED_ALBUM_COVER
    cover = server.follow(body, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == COVERED_ALBUM_COVER


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-004")
@allure.story("SCN-COVER-ART-004-B")
def test_a_bare_album_links_no_cover(server: Sut, seed_token: str) -> None:
    """An album without a cover exposes none."""
    response = get_album.sync_detailed(BARE_ALBUM, client=server.client(seed_token))
    assert response.status_code == 200, f"album read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)

    assert body.get("cover_image_id") is None
    assert body["_links"].get("cover") is None


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-006")
@allure.story("SCN-COVER-ART-006-A")
@allure.story("SCN-COVER-ART-006-C")
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
    assert track["_links"].get("cover") is None
    assert album["cover_image_id"] == COVERED_ALBUM_COVER
    cover = server.follow(album, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == COVERED_ALBUM_COVER


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-006")
@allure.story("SCN-COVER-ART-006-D")
def test_a_playlist_track_reports_no_artwork(server: Sut, seed_token: str) -> None:
    """A playlist track carries no cover; the album its recording belongs to still reports one."""
    response = get_playlist_track.sync_detailed(PLAYLIST_TRACK_CASE, 0, client=server.client(seed_token))
    assert response.status_code == 200, f"track read failed: {response.status_code} {response.content!r}"
    track = json.loads(response.content)
    album_response = get_album.sync_detailed(COVERED_ALBUM, client=server.client(seed_token))
    assert album_response.status_code == 200, f"album read failed: {album_response.status_code}"
    album = json.loads(album_response.content)

    assert "cover_image_id" not in track
    assert track["_links"].get("cover") is None
    assert album["cover_image_id"] == COVERED_ALBUM_COVER
    cover = server.follow(album, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == COVERED_ALBUM_COVER


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-006")
@allure.story("SCN-COVER-ART-006-B")
@allure.story("SCN-COVER-ART-006-C")
def test_an_episode_reports_no_artwork(server: Sut, seed_token: str) -> None:
    """An episode carries no cover; the season that sequences it reports its own."""
    response = get_series_episode.sync_detailed(SERIES, SEASON_INDEX, EPISODE_INDEX, client=server.client(seed_token))
    assert response.status_code == 200, f"episode read failed: {response.status_code} {response.content!r}"
    episode = json.loads(response.content)
    season_response = get_series_season.sync_detailed(SERIES, SEASON_INDEX, client=server.client(seed_token))
    assert season_response.status_code == 200, f"season read failed: {season_response.status_code}"
    season = json.loads(season_response.content)

    assert "cover_image_id" not in episode
    assert episode["_links"].get("cover") is None
    assert season["cover_image_id"] == SEASON_COVER
    cover = server.follow(season, "cover", token=seed_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert cover.json()["id"] == SEASON_COVER


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-003")
@allure.story("SCN-COVER-ART-003-B")
@allure.feature("REQ-COVER-ART-005")
@allure.story("SCN-COVER-ART-005-A")
@allure.feature("REQ-COVER-ART-007")
@allure.story("SCN-COVER-ART-007-A")
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


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-005")
@allure.story("SCN-COVER-ART-005-B")
@allure.feature("REQ-COVER-ART-007")
@allure.story("SCN-COVER-ART-007-B")
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


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-005")
@allure.story("SCN-COVER-ART-005-C")
@allure.feature("REQ-COVER-ART-007")
@allure.story("SCN-COVER-ART-007-B")
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
    assert body["_links"].get("cover") is None


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-003")
@allure.story("SCN-COVER-ART-003-C")
@allure.feature("REQ-COVER-ART-007")
@allure.story("SCN-COVER-ART-007-D")
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


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-002")
@allure.story("SCN-COVER-ART-002-B")
@allure.feature("REQ-COVER-ART-007")
@allure.story("SCN-COVER-ART-007-D")
def test_a_playlist_refuses_an_unknown_image(server: Sut, seed_token: str) -> None:
    """A cover that names no image is refused, and the refused creation stores nothing."""
    name = "Unknown Artwork Refused"
    refused = post_playlist.sync_detailed(
        client=server.client(seed_token),
        body=PostPlaylistRequest(name=name, cover_image_id=UNKNOWN_IMAGE),
    )
    listing = list_playlists.sync_detailed(client=server.client(seed_token), name=name, owner_id=10000)

    assert refused.status_code in {409, 422}, f"unknown image accepted: {refused.status_code}"
    assert json.loads(listing.content)["items"] == []


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-003")
@allure.story("SCN-COVER-ART-003-D")
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
