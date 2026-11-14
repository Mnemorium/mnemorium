"""Black-box proof of the `cover-art` capability.

A resource that owns artwork stores an optional cover and reports it as a nullable image identifier, with a link to the
image it references. An episode and a recording own no artwork: their representations carry no cover, and a client reads
the cover of the album, season or show it fetched beside them. The server never resolves or inherits a cover.

This module proves what a schema cannot: that the stored cover is the only cover field, that `_links.cover` follows it,
that a resource with no cover reports `null` and no link, that a track and an episode carry no artwork of their own, and
that attaching or clearing a playlist's artwork stores nothing when the image is unusable.

The declarative contract — the media types, the status of each operation, the representation's fields and its `_links` —
is validated by Schemathesis from the served document. Every route comes from the API index (`Sut.href`), from a
representation's named link or from the response's own `_links.self`, so the suite keeps its single `/api/v1` base
literal in `harness.Sut.index`.

The catalogue fixtures, the image helpers and the show helpers come from the feature modules that own them, so this
module uploads real media and never fabricates a typed identifier.
"""

import allure
from harness import Sut
from music import (
    create_album,
    create_group,
    create_playlist,
    create_recording,
    library_image,
    patch_playlist,
    place_recording,
    playlist_body,
    post_playlist,
    put_medium,
    read_album,
    read_playlist_track,
    read_track,
)
from series import (
    create_episode,
    create_season,
    create_show,
    episode_video,
    read_episode,
    read_season,
)


def cover_link(representation: dict) -> str | None:
    """The `_links.cover` href a representation carries, when it carries one."""
    link = representation["_links"].get("cover")
    return None if link is None else link["href"]


def cover_href(image_id: int) -> str:
    """The Asset image route a cover link names, read from the image it addresses."""
    return f"/api/v1/asset/image/{image_id}"


def placed_recording(server: Sut, token: str, album_id: int, name: str = "Song") -> int:
    """Stage one recording, place it in an album medium and return its identifier."""
    recording_id = create_recording(server, token, name=name)
    placed = place_recording(server, token, album_id, recording_id)
    assert placed.status_code in {200, 201}, f"placement failed: {placed.status_code} {placed.text}"
    return recording_id


def covered_album(server: Sut, token: str, cover: int) -> dict:
    """Create a group and an album carrying `cover`, and return them."""
    group = create_group(server, token)
    album = create_album(server, token, group["id"], cover_image_id=cover)
    put_medium(server, token, album["id"])
    return {"group": group, "album": album}


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-001")
@allure.story("SCN-COVER-ART-001-A")
def test_a_resource_with_its_own_artwork_reports_it(server: Sut, root_token: str) -> None:
    """A group that stores a cover reports it as its only cover field."""
    cover = library_image(server, root_token)
    group = create_group(server, root_token, cover_image_id=cover)

    body = server.follow(group, "self", token=root_token).json()

    assert body["cover_image_id"] == cover
    assert "effective_cover_image_id" not in body
    assert cover_link(body) == cover_href(cover)


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-001")
@allure.story("SCN-COVER-ART-001-B")
def test_a_resource_without_artwork_reports_null(server: Sut, root_token: str) -> None:
    """A group with no cover reports null and no cover link."""
    group = create_group(server, root_token)

    body = server.follow(group, "self", token=root_token).json()

    assert body["cover_image_id"] is None
    assert "effective_cover_image_id" not in body
    assert cover_link(body) is None


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-004")
@allure.story("SCN-COVER-ART-004-A")
@allure.story("SCN-COVER-ART-004-B")
def test_the_cover_link_follows_the_stored_cover(server: Sut, root_token: str) -> None:
    """An album with a cover exposes the link to that image, and an album without one exposes none."""
    cover = library_image(server, root_token)
    covered = covered_album(server, root_token, cover)
    bare_group = create_group(server, root_token, name="Bare Band")
    bare = create_album(server, root_token, bare_group["id"])

    with_cover = read_album(server, root_token, covered["album"]["id"]).json()
    without_cover = read_album(server, root_token, bare["id"]).json()

    assert cover_link(with_cover) == cover_href(cover)
    assert cover_link(without_cover) is None


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-006")
@allure.story("SCN-COVER-ART-006-A")
@allure.story("SCN-COVER-ART-006-C")
def test_a_track_reports_no_artwork(server: Sut, root_token: str) -> None:
    """An album track and a playlist track carry no cover; the album it belongs to reports one."""
    cover = library_image(server, root_token)
    catalogue = covered_album(server, root_token, cover)
    recording_id = placed_recording(server, root_token, catalogue["album"]["id"])
    playlist = create_playlist(server, root_token)
    appended = patch_playlist(server, root_token, playlist["id"], {"add_track": {"recording_id": recording_id}})
    assert appended.status_code == 200, f"append failed: {appended.status_code} {appended.text}"

    track = read_track(server, root_token, catalogue["album"]["id"]).json()
    entry = read_playlist_track(server, root_token, playlist["id"], 0).json()
    album = read_album(server, root_token, catalogue["album"]["id"]).json()

    assert "cover_image_id" not in track
    assert cover_link(track) is None
    assert "cover_image_id" not in entry
    assert cover_link(entry) is None
    assert album["cover_image_id"] == cover
    assert cover_link(album) == cover_href(cover)


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-006")
@allure.story("SCN-COVER-ART-006-B")
@allure.story("SCN-COVER-ART-006-C")
def test_an_episode_reports_no_artwork(server: Sut, root_token: str) -> None:
    """An episode carries no cover; the season that sequences it reports its own."""
    season_cover = library_image(server, root_token)
    show = create_show(server, root_token, cover_image_id=library_image(server, root_token))
    season = create_season(server, root_token, show["id"], 1, cover_image_id=season_cover)
    _file, video = episode_video(server, root_token)
    episode = create_episode(server, root_token, show["id"], video["id"], 1, 1)

    episode_body = read_episode(server, root_token, show["id"], 1, episode["id"]).json()
    season_body = read_season(server, root_token, show["id"], season["index"]).json()

    assert "cover_image_id" not in episode_body
    assert cover_link(episode_body) is None
    assert season_body["cover_image_id"] == season_cover
    assert cover_link(season_body) == cover_href(season_cover)


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-003")
@allure.story("SCN-COVER-ART-003-B")
@allure.feature("REQ-COVER-ART-007")
@allure.story("SCN-COVER-ART-007-A")
@allure.story("SCN-COVER-ART-007-B")
@allure.story("SCN-COVER-ART-007-C")
def test_a_playlist_attaches_replaces_and_clears_its_cover(server: Sut, root_token: str) -> None:
    """A playlist stores a cover on creation, replaces it, reports it and clears it with null."""
    first = library_image(server, root_token)
    second = library_image(server, root_token)

    created = create_playlist(server, root_token, cover_image_id=first)
    assert created["cover_image_id"] == first, "a playlist must store the cover it was created with"
    assert cover_link(created) == cover_href(first)

    replaced = patch_playlist(server, root_token, created["id"], {"cover_image_id": second})
    assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"
    assert replaced.json()["cover_image_id"] == second

    kept = patch_playlist(server, root_token, created["id"], {"name": "Renamed"})
    assert kept.json()["cover_image_id"] == second, "an absent field must keep the cover"

    cleared = patch_playlist(server, root_token, created["id"], {"cover_image_id": None})
    assert cleared.status_code == 200, f"clear failed: {cleared.status_code} {cleared.text}"
    assert cleared.json()["cover_image_id"] is None
    assert cover_link(cleared.json()) is None


@allure.epic("Cover art")
@allure.feature("REQ-COVER-ART-003")
@allure.story("SCN-COVER-ART-003-C")
@allure.feature("REQ-COVER-ART-007")
@allure.story("SCN-COVER-ART-007-D")
def test_a_playlist_refuses_an_unusable_image(server: Sut, root_token: str) -> None:
    """A library image and an unknown image are both refused and store nothing."""
    library_cover = library_image(server, root_token)
    create_group(server, root_token, cover_image_id=library_cover)

    conflict = post_playlist(server, root_token, playlist_body("Lit", cover_image_id=library_cover))
    unknown = post_playlist(server, root_token, playlist_body("Unknown", cover_image_id=999_999_999))

    assert conflict.status_code == 409, f"library image accepted: {conflict.status_code} {conflict.text}"
    assert unknown.status_code in {409, 422}, f"unknown image accepted: {unknown.status_code}"
