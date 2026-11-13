"""Black-box proof of the `series` capability.

A show is created, listed as a partial item, read in full with its season summaries, updated partially, and holds
seasons and episodes addressed by their parent-scoped index. This module proves what a schema cannot: that a show's
seasons and episodes survive every write that does not address them, that a repeated show name creates a second show,
that an unknown genre is stored on demand, that the listing is partial and ordered, that the index survives the whole
32-bit range, that a replaced episode releases its previous video, that a refusal stores nothing, and that a caller who
may read but not edit is refused while the stored hierarchy survives.

The declarative contract — the media types, the status of each operation, the representation's fields, its `_links` and
the request schema — is validated by Schemathesis from the served document. Every route comes from the API index
(`Sut.href`), from a representation's named link or from the response's own `_links.self`, so the suite keeps its single
`/api/v1` base literal in `harness.Sut.index`.

The image and video helpers come from the feature modules that own them, so this module uploads real media and never
fabricates a typed identifier.
"""

import time

import allure
from conftest import add_to_gallery, create_gallery, provision
from gallery import image_file
from harness import Sut, link_of
from video import assert_summary, typed_video, upload_video

# The show resource kind the resource-policy route addresses.
SERIES = "SERIES"

# A genre the catalogue has never stored, distinct from every seeded genre.
UNKNOWN_GENRE = "Telenovela"

# The largest index a 32-bit unsigned position admits.
MAX_U32_INDEX = 4_294_967_295


def show_body(**overrides: object) -> dict[str, object]:
    """A show-creation payload, with optional overrides."""
    body: dict[str, object] = {"name": "Twin Peaks"}
    body.update(overrides)
    return body


def post_show(server: Sut, token: str, body: dict[str, object]):
    """POST a show creation and return the raw response."""
    return server.request("POST", server.href("series"), token=token, json=body)


def create_show(server: Sut, token: str, **overrides: object) -> dict:
    """Create a show and return its representation."""
    response = post_show(server, token, show_body(**overrides))
    assert response.status_code == 201, f"show creation failed: {response.status_code} {response.text}"
    return response.json()


def list_shows(server: Sut, token: str, query: str = ""):
    """GET the show collection with an optional query string."""
    return server.request("GET", f"{server.href('series')}{query}", token=token)


def read_show(server: Sut, token: str, series_id: int):
    """GET one show by identifier."""
    return server.request("GET", f"{server.href('series')}/{series_id}", token=token)


def patch_show(server: Sut, token: str, series_id: int, body: dict[str, object]):
    """PATCH one show's metadata and return the raw response."""
    return server.request("PATCH", f"{server.href('series')}/{series_id}", token=token, json=body)


def season_href(server: Sut, series_id: int, season_index: object) -> str:
    """The route of one season of `series_id`."""
    return f"{server.href('series')}/{series_id}/season/{season_index}"


def episode_href(server: Sut, series_id: int, season_index: object, episode_index: object) -> str:
    """The route of one episode of a season of `series_id`."""
    return f"{season_href(server, series_id, season_index)}/episode/{episode_index}"


def put_season(server: Sut, token: str, series_id: int, season_index: object, body: dict):
    """PUT a season write and return the raw response."""
    return server.request("PUT", season_href(server, series_id, season_index), token=token, json=body)


def create_season(server: Sut, token: str, series_id: int, season_index: object = 1, **overrides: object) -> dict:
    """Create a season at a free index and return its representation."""
    body: dict[str, object] = {"name": "Season One"}
    body.update(overrides)
    response = put_season(server, token, series_id, season_index, body)
    assert response.status_code == 201, f"season creation failed: {response.status_code} {response.text}"
    return response.json()


def read_season(server: Sut, token: str, series_id: int, season_index: object):
    """GET one season of a show."""
    return server.request("GET", season_href(server, series_id, season_index), token=token)


def patch_season(server: Sut, token: str, series_id: int, season_index: object, body: dict):
    """PATCH one season and return the raw response."""
    return server.request("PATCH", season_href(server, series_id, season_index), token=token, json=body)


def list_episodes(server: Sut, token: str, series_id: int, season_index: object):
    """GET the episodes of one season."""
    return server.request("GET", f"{season_href(server, series_id, season_index)}/episode", token=token)


def put_episode(
    server: Sut,
    token: str,
    series_id: int,
    video_id: object,
    season_index: object = 1,
    episode_index: object = 1,
    **overrides: object,
) -> dict:
    """PUT an episode write and return the raw response."""
    body: dict[str, object] = {"name": "Pilot", "video_id": video_id}
    body.update(overrides)
    return server.request(
        "PUT",
        episode_href(server, series_id, season_index, episode_index),
        token=token,
        json=body,
    )


def create_episode(
    server: Sut,
    token: str,
    series_id: int,
    video_id: object,
    season_index: object = 1,
    episode_index: object = 1,
    **overrides: object,
) -> dict:
    """Create an episode and return its representation."""
    response = put_episode(
        server,
        token,
        series_id,
        video_id,
        season_index,
        episode_index,
        **overrides,
    )
    assert response.status_code == 201, f"episode creation failed: {response.status_code} {response.text}"
    return response.json()


def read_episode(server: Sut, token: str, series_id: int, season_index: object, episode_index: object):
    """GET one episode of a season of a show."""
    return server.request("GET", episode_href(server, series_id, season_index, episode_index), token=token)


def patch_episode(server: Sut, token: str, series_id: int, episode_index: object, body: dict):
    """PATCH one episode and return the raw response."""
    return server.request(
        "PATCH",
        episode_href(server, series_id, 1, episode_index),
        token=token,
        json=body,
    )


def collection_ids(response) -> list[int]:
    """The identifiers a show collection lists."""
    return [item["id"] for item in response.json()["_embedded"]["items"]]


def episode_ids(response) -> list[int]:
    """The episode indexes a season collection lists."""
    return [item["id"] for item in response.json()["items"]]


def image_id(server: Sut, token: str, file: dict) -> int:
    """Follow a file's `image` link and return the image identifier."""
    response = server.follow(file, "image", token=token)
    assert response.status_code == 200, f"image read failed: {response.status_code} {response.text}"
    return int(response.json()["id"])


def library_image(server: Sut, token: str) -> int:
    """Upload one image no resource has attached yet and return its identifier."""
    return image_id(server, token, image_file(server, token, "cover.png"))


def gallery_image(server: Sut, token: str) -> int:
    """Upload one image, use it as gallery media and return its identifier."""
    file = image_file(server, token, "gallery-cover.png")
    gallery = create_gallery(server, token, f"Media {time.time_ns()}")
    add_to_gallery(server, token, gallery, file["id"])
    return image_id(server, token, file)


def episode_video(server: Sut, token: str, name: str = "episode.mp4") -> tuple[dict, dict]:
    """Upload one video and return its file and typed representations."""
    file = upload_video(server, token, name)
    return file, typed_video(server, token, file)


def deny_viewer(server: Sut, token: str, user_id: int, series_id: int) -> None:
    """Deny `user_id` the Viewer bundle on one show, so it may neither discover nor view it."""
    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{SERIES}/{series_id}",
        token=token,
        json={
            "visibility": "PUBLIC",
            "grants": [
                {
                    "principal": {"kind": "USER", "id": user_id},
                    "role": "VIEWER",
                    "effect": "DENY",
                }
            ],
        },
    )
    assert response.status_code == 200, f"deny failed: {response.status_code} {response.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-008")
@allure.story("SCN-SERIES-008-A")
def test_a_new_show_is_created(server: Sut, root_token: str) -> None:
    """A show is created from its metadata and answers 201 with a Location and no seasons."""
    cover = library_image(server, root_token)

    response = post_show(
        server,
        root_token,
        show_body(
            name="Twin Peaks",
            plot="An FBI agent visits a small town",
            country_of_origin="us",
            first_air_date="1990-04-08",
            genres=["Drama", "Comedy"],
            cover_image_id=cover,
        ),
    )

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    show = response.json()
    assert show["id"] > 0
    assert show["name"] == "Twin Peaks"
    assert show["plot"] == "An FBI agent visits a small town"
    assert show["country_of_origin"] == "US"
    assert show["first_air_date"] == "1990-04-08"
    assert show["genres"] == ["COMEDY", "DRAMA"]
    assert show["cover_image_id"] == cover
    assert show["seasons"] == []
    assert response.headers["Location"] == link_of(show, "self")

    listed = list_shows(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [show["id"]]


@allure.epic("Series")
@allure.feature("REQ-SERIES-008")
@allure.story("SCN-SERIES-008-B")
def test_a_repeated_name_creates_a_second_show(server: Sut, root_token: str) -> None:
    """A name another show already uses creates a second show rather than returning the first."""
    first = create_show(server, root_token, name="Twin Peaks")

    second = create_show(server, root_token, name="Twin Peaks")

    assert second["id"] != first["id"]
    assert read_show(server, root_token, first["id"]).json()["name"] == "Twin Peaks"
    listed = list_shows(server, root_token)
    assert collection_ids(listed) == sorted([first["id"], second["id"]])


@allure.epic("Series")
@allure.feature("REQ-SERIES-008")
@allure.story("SCN-SERIES-008-C")
def test_an_unknown_genre_is_stored_and_linked(server: Sut, root_token: str) -> None:
    """A genre the catalogue does not hold is stored and the show reports it."""
    created = create_show(server, root_token, genres=[UNKNOWN_GENRE])

    assert created["genres"] == [UNKNOWN_GENRE.upper()]
    read = read_show(server, root_token, created["id"])
    assert read.status_code == 200, f"read failed: {read.status_code} {read.text}"
    assert read.json()["genres"] == [UNKNOWN_GENRE.upper()]


@allure.epic("Series")
@allure.feature("REQ-SERIES-008")
@allure.story("SCN-SERIES-008-D")
def test_unusable_creation_input_is_refused(server: Sut, root_token: str) -> None:
    """An empty name or genre, malformed date or non-two-letter country answers 422 and stores nothing."""
    bodies = [
        show_body(name=""),
        show_body(genres=[""]),
        show_body(first_air_date="08/04/1990"),
        show_body(country_of_origin="U.S.A."),
    ]

    for body in bodies:
        response = post_show(server, root_token, body)
        assert response.status_code == 422, f"unusable input accepted: {response.status_code} {response.text}"

    listed = list_shows(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [], "a refused creation stored a show"


@allure.epic("Series")
@allure.feature("REQ-SERIES-008")
@allure.story("SCN-SERIES-008-E")
def test_a_cover_that_is_not_library_artwork_is_refused(server: Sut, root_token: str) -> None:
    """An image already used as gallery media answers 409 and creates no show."""
    _alice_id, alice = provision(server, root_token, "alice")
    attached = gallery_image(server, alice)

    response = post_show(server, root_token, show_body(cover_image_id=attached))

    assert response.status_code == 409, f"gallery media accepted: {response.status_code} {response.text}"
    listed = list_shows(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [], "a refused creation stored a show"


@allure.epic("Series")
@allure.feature("REQ-SERIES-008")
@allure.story("SCN-SERIES-008-F")
def test_a_standard_user_cannot_create_a_show(server: Sut, root_token: str) -> None:
    """A Standard user's creation answers 403 and stores nothing."""
    _alice_id, alice = provision(server, root_token, "alice")

    response = post_show(server, alice, show_body())

    assert response.status_code == 403, f"standard creation accepted: {response.status_code} {response.text}"
    listed = list_shows(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [], "a refused creation stored a show"


@allure.epic("Series")
@allure.feature("REQ-SERIES-001")
@allure.story("SCN-SERIES-001-A")
@allure.story("SCN-SERIES-001-D")
@allure.feature("REQ-SERIES-002")
@allure.story("SCN-SERIES-002-A")
def test_the_collection_lists_partial_shows_in_identifier_order(server: Sut, root_token: str) -> None:
    """Two shows list as partial items by ascending identifier, with their cover links."""
    cover = library_image(server, root_token)
    first = create_show(server, root_token, name="First", cover_image_id=cover)
    second = create_show(server, root_token, name="Second")

    response = list_shows(server, root_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    items = response.json()["_embedded"]["items"]
    assert collection_ids(response) == [first["id"], second["id"]]

    covered, uncovered = items
    assert set(covered) == {"id", "name", "cover_image_id", "_links"}, "a listing item must be partial"
    assert covered["id"] == first["id"]
    assert covered["name"] == "First"
    assert covered["cover_image_id"] == cover
    assert link_of(covered, "self") == f"{server.href('series')}/{first['id']}"

    asset = server.follow(covered, "cover", token=root_token)
    assert asset.status_code == 200, f"cover link failed: {asset.status_code} {asset.text}"
    assert asset.json()["id"] == cover
    assert "cover" not in uncovered["_links"]
    assert uncovered["cover_image_id"] is None


@allure.epic("Series")
@allure.feature("REQ-SERIES-001")
@allure.story("SCN-SERIES-001-B")
def test_the_name_filter_and_page_are_applied(server: Sut, root_token: str) -> None:
    """The name filter narrows the collection and limit/offset select one page."""
    ids = [create_show(server, root_token, name=name)["id"] for name in ("Alpha", "Beta", "Gamma")]

    filtered = list_shows(server, root_token, "?name=Beta")
    assert filtered.status_code == 200, f"filter failed: {filtered.status_code} {filtered.text}"
    assert collection_ids(filtered) == [ids[1]]

    paged = list_shows(server, root_token, "?limit=1&offset=1")
    assert paged.status_code == 200, f"page failed: {paged.status_code} {paged.text}"
    assert collection_ids(paged) == [ids[1]]


@allure.epic("Series")
@allure.feature("REQ-SERIES-001")
@allure.story("SCN-SERIES-001-C")
def test_a_caller_that_discovers_no_show_lists_an_empty_collection(server: Sut, root_token: str) -> None:
    """A caller denied the only show discovers none and receives an empty collection."""
    alice_id, alice = provision(server, root_token, "alice")
    show = create_show(server, root_token)
    deny_viewer(server, root_token, alice_id, show["id"])

    response = list_shows(server, alice)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.json()["_embedded"]["items"] == []


@allure.epic("Series")
@allure.feature("REQ-SERIES-001")
@allure.story("SCN-SERIES-001-E")
@allure.story("SCN-SERIES-001-F")
def test_an_out_of_range_limit_and_a_malformed_query_are_refused(server: Sut, root_token: str) -> None:
    """A limit outside 1..100 answers 422 and an undecodable query answers 400."""
    for query in ("?limit=0", "?limit=101"):
        response = list_shows(server, root_token, query)
        assert response.status_code == 422, f"limit accepted: {response.status_code} {response.text}"

    malformed = list_shows(server, root_token, "?limit=abc")
    assert malformed.status_code == 400, f"query accepted: {malformed.status_code} {malformed.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-002")
@allure.story("SCN-SERIES-002-A")
@allure.story("SCN-SERIES-002-B")
def test_the_detail_read_carries_the_show_metadata_and_season_summaries(server: Sut, root_token: str) -> None:
    """The read carries the metadata, ordered genres, cover and a summary of every season by index."""
    cover = library_image(server, root_token)
    season_cover = library_image(server, root_token)
    show = create_show(
        server,
        root_token,
        name="Twin Peaks",
        plot="An FBI agent visits a small town",
        country_of_origin="gb",
        first_air_date="1990-04-08",
        genres=["Drama", "Comedy"],
        cover_image_id=cover,
    )
    create_season(server, root_token, show["id"], 2, name="Season Two", cover_image_id=season_cover)
    first_season = create_season(server, root_token, show["id"], 1, name="Season One")
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"])

    response = read_show(server, root_token, show["id"])

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["name"] == "Twin Peaks"
    assert body["plot"] == "An FBI agent visits a small town"
    assert body["country_of_origin"] == "GB"
    assert body["first_air_date"] == "1990-04-08"
    assert body["genres"] == ["COMEDY", "DRAMA"]
    assert body["cover_image_id"] == cover
    assert link_of(body, "cover").endswith(f"/{cover}")

    seasons = body["seasons"]
    assert [season["index"] for season in seasons] == [1, 2]
    assert [season["name"] for season in seasons] == ["Season One", "Season Two"]
    assert [season["episode_count"] for season in seasons] == [1, 0]
    assert [season["cover_image_id"] for season in seasons] == [None, season_cover]
    assert link_of(seasons[0], "self") == link_of(first_season, "self")
    assert link_of(seasons[0], "episodes") == f"{link_of(first_season, 'self')}/episode"


@allure.epic("Series")
@allure.feature("REQ-SERIES-002")
@allure.story("SCN-SERIES-002-C")
def test_a_show_without_a_date_or_country_reports_null(server: Sut, root_token: str) -> None:
    """A show that stores neither a first air date nor a country reports both as null."""
    show = create_show(server, root_token, name="Undated")

    response = read_show(server, root_token, show["id"])

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["first_air_date"] is None
    assert body["country_of_origin"] is None
    assert body["seasons"] == []


@allure.epic("Series")
@allure.feature("REQ-SERIES-002")
@allure.story("SCN-SERIES-002-D")
def test_an_unknown_or_unviewable_show_is_not_found(server: Sut, root_token: str) -> None:
    """An unknown show and one the caller may not view both answer 404."""
    alice_id, alice = provision(server, root_token, "alice")
    show = create_show(server, root_token)
    deny_viewer(server, root_token, alice_id, show["id"])

    unknown = read_show(server, root_token, show["id"] + 1_000_000)
    unviewable = read_show(server, alice, show["id"])

    assert unknown.status_code == 404, f"unknown show answered {unknown.status_code}: {unknown.text}"
    assert unviewable.status_code == 404, f"unviewable show answered {unviewable.status_code}: {unviewable.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-002")
@allure.story("SCN-SERIES-002-E")
def test_a_malformed_show_identifier_is_refused(server: Sut, root_token: str) -> None:
    """A series_id that is not an identifier answers 400."""
    response = read_show(server, root_token, "not-an-identifier")

    assert response.status_code == 400, f"malformed id answered {response.status_code}: {response.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-003")
@allure.story("SCN-SERIES-003-A")
def test_the_index_is_the_identity_of_a_season_and_an_episode(server: Sut, root_token: str) -> None:
    """A season and an episode are addressed and reported by their parent-scoped index."""
    show = create_show(server, root_token)
    season = create_season(server, root_token, show["id"], 7, name="Season Seven")
    _file, video = episode_video(server, root_token)
    episode = create_episode(server, root_token, show["id"], video["id"], 7, 5)

    assert season["index"] == 7
    assert season["series_id"] == show["id"]
    assert link_of(season, "self") == season_href(server, show["id"], 7)
    assert episode["id"] == 5
    assert episode["season_index"] == 7
    assert episode["series_id"] == show["id"]
    assert link_of(episode, "self") == episode_href(server, show["id"], 7, 5)

    read = read_season(server, root_token, show["id"], 7)
    assert read.status_code == 200, f"season read failed: {read.status_code} {read.text}"
    assert read.json()["index"] == 7


@allure.epic("Series")
@allure.feature("REQ-SERIES-003")
@allure.story("SCN-SERIES-003-B")
def test_the_largest_32_bit_index_is_addressable(server: Sut, root_token: str) -> None:
    """A season and an episode stored at the largest 32-bit index are addressed and returned."""
    show = create_show(server, root_token)
    _file, video = episode_video(server, root_token)

    season = create_season(server, root_token, show["id"], MAX_U32_INDEX, name="The Last Season")
    episode = create_episode(server, root_token, show["id"], video["id"], MAX_U32_INDEX, MAX_U32_INDEX)

    assert season["index"] == MAX_U32_INDEX
    assert episode["id"] == MAX_U32_INDEX
    read_season_response = read_season(server, root_token, show["id"], MAX_U32_INDEX)
    assert read_season_response.status_code == 200, read_season_response.text
    assert read_season_response.json()["episode_count"] == 1
    read_episode_response = read_episode(server, root_token, show["id"], MAX_U32_INDEX, MAX_U32_INDEX)
    assert read_episode_response.status_code == 200, read_episode_response.text
    assert read_episode_response.json()["name"] == "Pilot"


@allure.epic("Series")
@allure.feature("REQ-SERIES-004")
@allure.story("SCN-SERIES-004-A")
@allure.story("SCN-SERIES-004-B")
def test_a_season_is_read_with_its_fields_and_scoped_to_its_show(server: Sut, root_token: str) -> None:
    """Two shows each hold an independent season 1, and each reads with its own fields."""
    cover = library_image(server, root_token)
    first = create_show(server, root_token, name="First Show")
    second = create_show(server, root_token, name="Second Show")
    first_season = create_season(server, root_token, first["id"], 1, name="First Season", cover_image_id=cover)
    create_season(server, root_token, second["id"], 1, name="Other Season")
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, first["id"], video["id"])

    response = read_season(server, root_token, first["id"], 1)

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    season = response.json()
    assert season["series_id"] == first["id"]
    assert season["index"] == 1
    assert season["name"] == "First Season"
    assert season["episode_count"] == 1
    assert season["cover_image_id"] == cover
    assert link_of(season, "series") == link_of(first_season, "series")
    assert link_of(season, "episodes") == f"{link_of(first_season, 'self')}/episode"
    assert link_of(season, "cover").endswith(f"/{cover}")

    other = read_season(server, root_token, second["id"], 1)
    assert other.status_code == 200, f"read failed: {other.status_code} {other.text}"
    assert other.json()["name"] == "Other Season"
    assert other.json()["series_id"] == second["id"]


@allure.epic("Series")
@allure.feature("REQ-SERIES-004")
@allure.story("SCN-SERIES-004-C")
def test_an_unknown_or_foreign_season_is_not_found(server: Sut, root_token: str) -> None:
    """An unknown show, an index the show does not hold and a season of another show all answer 404."""
    first = create_show(server, root_token, name="First Show")
    second = create_show(server, root_token, name="Second Show")
    create_season(server, root_token, first["id"], 1)
    create_season(server, root_token, second["id"], 2)

    unknown_show = read_season(server, root_token, first["id"] + 1_000_000, 1)
    unknown_season = read_season(server, root_token, first["id"], 9)
    foreign = read_season(server, root_token, first["id"], 2)

    assert unknown_show.status_code == 404, f"unknown show answered {unknown_show.status_code}: {unknown_show.text}"
    assert unknown_season.status_code == 404, (
        f"missing season answered {unknown_season.status_code}: {unknown_season.text}"
    )
    assert foreign.status_code == 404, f"foreign season answered {foreign.status_code}: {foreign.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-004")
@allure.story("SCN-SERIES-004-D")
@allure.story("SCN-SERIES-005-D")
@allure.story("SCN-SERIES-006-D")
def test_a_malformed_index_or_identifier_is_refused(server: Sut, root_token: str) -> None:
    """A path segment that is not an identifier or a 32-bit unsigned index answers 400."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"], 1)
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"])

    season_index = read_season(server, root_token, show["id"], "not-an-index")
    season_show = read_season(server, root_token, "not-an-identifier", 1)
    episode_list = list_episodes(server, root_token, show["id"], "not-an-index")
    episode_show = list_episodes(server, root_token, "not-an-identifier", 1)
    episode_index = read_episode(server, root_token, show["id"], 1, "not-an-index")
    episode_season = read_episode(server, root_token, show["id"], "not-an-index", 1)
    episode_show_read = read_episode(server, root_token, "not-an-identifier", 1, 1)

    for response in (
        season_index,
        season_show,
        episode_list,
        episode_show,
        episode_index,
        episode_season,
        episode_show_read,
    ):
        assert response.status_code == 400, f"malformed segment answered {response.status_code}: {response.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-005")
@allure.story("SCN-SERIES-005-A")
@allure.story("SCN-SERIES-005-B")
def test_the_episodes_of_a_season_are_listed_with_their_fields(server: Sut, root_token: str) -> None:
    """The episode collection carries each episode's index, fields, video summary and links."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    dated_file, dated_video = episode_video(server, root_token, "dated.mp4")
    undated_file, undated_video = episode_video(server, root_token, "undated.mp4")
    create_episode(
        server,
        root_token,
        show["id"],
        dated_video["id"],
        1,
        1,
        name="Pilot",
        plot="An opening.",
        air_date="1990-04-08",
    )
    create_episode(server, root_token, show["id"], undated_video["id"], 1, 2, name="Second")

    response = list_episodes(server, root_token, show["id"], 1)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    items = response.json()["items"]
    assert episode_ids(response) == [1, 2]

    pilot, second = items
    assert pilot["name"] == "Pilot"
    assert pilot["plot"] == "An opening."
    assert pilot["air_date"] == "1990-04-08"
    assert pilot["series_id"] == show["id"]
    assert pilot["season_index"] == 1
    assert_summary(pilot, dated_file, dated_video)
    assert link_of(pilot, "series") == f"{server.href('series')}/{show['id']}"
    assert link_of(pilot, "season") == season_href(server, show["id"], 1)
    assert second["air_date"] is None
    assert_summary(second, undated_file, undated_video)


@allure.epic("Series")
@allure.feature("REQ-SERIES-005")
@allure.story("SCN-SERIES-005-C")
def test_episodes_of_an_unknown_or_foreign_season_are_not_found(server: Sut, root_token: str) -> None:
    """An unknown show, an index the show does not hold and a season of another show all answer 404."""
    first = create_show(server, root_token, name="First Show")
    second = create_show(server, root_token, name="Second Show")
    create_season(server, root_token, first["id"], 1)
    create_season(server, root_token, second["id"], 2)

    unknown_show = list_episodes(server, root_token, first["id"] + 1_000_000, 1)
    unknown_season = list_episodes(server, root_token, first["id"], 9)
    foreign = list_episodes(server, root_token, first["id"], 2)

    assert unknown_show.status_code == 404, f"unknown show answered {unknown_show.status_code}: {unknown_show.text}"
    assert unknown_season.status_code == 404, (
        f"missing season answered {unknown_season.status_code}: {unknown_season.text}"
    )
    assert foreign.status_code == 404, f"foreign season answered {foreign.status_code}: {foreign.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-006")
@allure.story("SCN-SERIES-006-A")
def test_an_episode_is_read_with_its_fields(server: Sut, root_token: str) -> None:
    """The read carries the episode's index, metadata, video summary and links."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    file, video = episode_video(server, root_token)
    create_episode(
        server,
        root_token,
        show["id"],
        video["id"],
        name="Pilot",
        plot="An opening.",
        air_date="1990-04-08",
    )

    response = read_episode(server, root_token, show["id"], 1, 1)

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    episode = response.json()
    assert episode["id"] == 1
    assert episode["name"] == "Pilot"
    assert episode["plot"] == "An opening."
    assert episode["air_date"] == "1990-04-08"
    assert episode["series_id"] == show["id"]
    assert episode["season_index"] == 1
    assert link_of(episode, "self") == episode_href(server, show["id"], 1, 1)
    assert_summary(episode, file, video)


@allure.epic("Series")
@allure.feature("REQ-SERIES-006")
@allure.story("SCN-SERIES-006-B")
def test_an_episode_without_an_air_date_reports_null(server: Sut, root_token: str) -> None:
    """An episode that stores no air date reports `air_date` as null."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], name="Undated")

    response = read_episode(server, root_token, show["id"], 1, 1)

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    assert response.json()["air_date"] is None


@allure.epic("Series")
@allure.feature("REQ-SERIES-006")
@allure.story("SCN-SERIES-006-C")
def test_an_episode_outside_the_season_is_not_found(server: Sut, root_token: str) -> None:
    """An episode of another season, and an unknown show, season or episode all answer 404."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"], 1)
    create_season(server, root_token, show["id"], 2)
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], 2, 1, name="Second Season Pilot")

    other_season = read_episode(server, root_token, show["id"], 1, 1)
    unknown_episode = read_episode(server, root_token, show["id"], 2, 9)
    unknown_season = read_episode(server, root_token, show["id"], 9, 1)
    unknown_show = read_episode(server, root_token, show["id"] + 1_000_000, 1, 1)

    for response in (other_season, unknown_episode, unknown_season, unknown_show):
        assert response.status_code == 404, f"answered {response.status_code}: {response.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-007")
@allure.story("SCN-SERIES-007-A")
def test_seasons_and_episodes_are_listed_in_ascending_index_order(server: Sut, root_token: str) -> None:
    """Seasons and episodes list by ascending index whatever order they were written in."""
    show = create_show(server, root_token)
    for season_index in (3, 1, 2):
        create_season(server, root_token, show["id"], season_index, name=f"Season {season_index}")
        for episode_index in (2, 1):
            _file, video = episode_video(server, root_token, f"s{season_index}e{episode_index}.mp4")
            create_episode(
                server,
                root_token,
                show["id"],
                video["id"],
                season_index,
                episode_index,
                name=f"Episode {episode_index}",
            )

    read = read_show(server, root_token, show["id"])
    assert read.status_code == 200, f"read failed: {read.status_code} {read.text}"
    assert [season["index"] for season in read.json()["seasons"]] == [1, 2, 3]

    for season_index in (1, 2, 3):
        episodes = list_episodes(server, root_token, show["id"], season_index)
        assert episodes.status_code == 200, f"list failed: {episodes.status_code} {episodes.text}"
        assert episode_ids(episodes) == [1, 2]


@allure.epic("Series")
@allure.feature("REQ-SERIES-009")
@allure.story("SCN-SERIES-009-A")
@allure.story("SCN-SERIES-009-C")
def test_the_show_metadata_is_updated_partially(server: Sut, root_token: str) -> None:
    """A patch stores each submitted field and leaves every season and episode unchanged."""
    cover = library_image(server, root_token)
    show = create_show(
        server,
        root_token,
        name="Old Name",
        plot="Old plot",
        country_of_origin="US",
        first_air_date="1990-04-08",
        genres=["Drama"],
    )
    create_season(server, root_token, show["id"], 1, name="Season One")
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], 1, 1, name="Pilot", air_date="1990-04-09")

    response = patch_show(
        server,
        root_token,
        show["id"],
        {
            "name": "New Name",
            "plot": "New plot",
            "country_of_origin": "gb",
            "first_air_date": "1991-01-02",
            "genres": ["Comedy"],
            "cover_image_id": cover,
        },
    )

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["name"] == "New Name"
    assert body["plot"] == "New plot"
    assert body["country_of_origin"] == "GB"
    assert body["first_air_date"] == "1991-01-02"
    assert body["genres"] == ["COMEDY"]
    assert body["cover_image_id"] == cover

    stored = read_show(server, root_token, show["id"]).json()
    assert stored["name"] == "New Name"
    assert [(season["index"], season["name"], season["episode_count"]) for season in stored["seasons"]] == [
        (1, "Season One", 1)
    ]
    episode = read_episode(server, root_token, show["id"], 1, 1)
    assert episode.status_code == 200, f"episode read failed: {episode.status_code} {episode.text}"
    assert episode.json()["name"] == "Pilot"
    assert episode.json()["air_date"] == "1990-04-09"
    assert episode.json()["video"]["id"] == video["id"]


@allure.epic("Series")
@allure.feature("REQ-SERIES-009")
@allure.story("SCN-SERIES-009-B")
def test_null_clears_a_show_optional_value(server: Sut, root_token: str) -> None:
    """A null clears the first air date, the country and the cover and the read reports each as null."""
    cover = library_image(server, root_token)
    show = create_show(
        server,
        root_token,
        country_of_origin="US",
        first_air_date="1990-04-08",
        cover_image_id=cover,
    )

    response = patch_show(
        server,
        root_token,
        show["id"],
        {"first_air_date": None, "country_of_origin": None, "cover_image_id": None},
    )

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["first_air_date"] is None
    assert body["country_of_origin"] is None
    assert body["cover_image_id"] is None
    assert "cover" not in body["_links"]

    stored = read_show(server, root_token, show["id"]).json()
    assert stored["first_air_date"] is None
    assert stored["country_of_origin"] is None
    assert stored["cover_image_id"] is None


@allure.epic("Series")
@allure.feature("REQ-SERIES-009")
@allure.story("SCN-SERIES-009-D")
def test_a_standard_user_may_read_but_not_edit_a_show(server: Sut, root_token: str) -> None:
    """A Standard user reads the show but its patch answers 403 and stores nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    show = create_show(server, root_token, name="Untouched")

    readable = read_show(server, alice, show["id"])
    refused = patch_show(server, alice, show["id"], {"name": "Hijacked"})

    assert readable.status_code == 200, f"standard read failed: {readable.status_code} {readable.text}"
    assert refused.status_code == 403, f"standard patch accepted: {refused.status_code} {refused.text}"
    assert read_show(server, root_token, show["id"]).json()["name"] == "Untouched"


@allure.epic("Series")
@allure.feature("REQ-SERIES-009")
@allure.story("SCN-SERIES-009-E")
def test_an_unknown_or_unviewable_show_cannot_be_patched(server: Sut, root_token: str) -> None:
    """An unknown show and one the caller may not view both answer 404 and apply nothing."""
    alice_id, alice = provision(server, root_token, "alice")
    show = create_show(server, root_token, name="Untouched")
    deny_viewer(server, root_token, alice_id, show["id"])

    unknown = patch_show(server, root_token, show["id"] + 1_000_000, {"name": "Other"})
    unviewable = patch_show(server, alice, show["id"], {"name": "Hijacked"})

    assert unknown.status_code == 404, f"unknown show answered {unknown.status_code}: {unknown.text}"
    assert unviewable.status_code == 404, f"unviewable show answered {unviewable.status_code}: {unviewable.text}"
    assert read_show(server, root_token, show["id"]).json()["name"] == "Untouched"


@allure.epic("Series")
@allure.feature("REQ-SERIES-010")
@allure.story("SCN-SERIES-010-A")
@allure.story("SCN-SERIES-010-B")
@allure.story("SCN-SERIES-010-C")
def test_a_season_is_created_then_replaced_keeping_its_episodes(server: Sut, root_token: str) -> None:
    """A free index creates a season with 201; the same index replaces it with 200 and keeps the episodes."""
    cover = library_image(server, root_token)
    show = create_show(server, root_token)

    created = put_season(server, root_token, show["id"], 1, {"name": "Season One"})

    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"
    season = created.json()
    assert season["index"] == 1
    assert season["name"] == "Season One"
    assert season["episode_count"] == 0
    assert created.headers["Location"] == link_of(season, "self")

    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], 1, 1, name="Pilot")

    replaced = put_season(
        server,
        root_token,
        show["id"],
        1,
        {"name": "First Season", "cover_image_id": cover},
    )

    assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"
    body = replaced.json()
    assert body["index"] == 1
    assert body["name"] == "First Season"
    assert body["cover_image_id"] == cover
    assert body["episode_count"] == 1
    episode = read_episode(server, root_token, show["id"], 1, 1)
    assert episode.status_code == 200, f"episode read failed: {episode.status_code} {episode.text}"
    assert episode.json()["name"] == "Pilot"
    assert episode.json()["video"]["id"] == video["id"]


@allure.epic("Series")
@allure.feature("REQ-SERIES-010")
@allure.story("SCN-SERIES-010-D")
def test_a_season_write_of_an_unknown_show_or_malformed_index_is_refused(server: Sut, root_token: str) -> None:
    """An unknown show answers 404 and an index that is not a 32-bit unsigned integer answers 400."""
    show = create_show(server, root_token)

    unknown_show = put_season(server, root_token, show["id"] + 1_000_000, 1, {"name": "Season One"})
    malformed = put_season(server, root_token, show["id"], "not-an-index", {"name": "Season One"})

    assert unknown_show.status_code == 404, f"unknown show answered {unknown_show.status_code}: {unknown_show.text}"
    assert malformed.status_code == 400, f"malformed index answered {malformed.status_code}: {malformed.text}"
    assert read_show(server, root_token, show["id"]).json()["seasons"] == []


@allure.epic("Series")
@allure.feature("REQ-SERIES-010")
@allure.story("SCN-SERIES-010-E")
def test_a_season_cover_that_is_not_library_artwork_is_refused(server: Sut, root_token: str) -> None:
    """An image already used as gallery media answers 409 and creates no season."""
    _alice_id, alice = provision(server, root_token, "alice")
    attached = gallery_image(server, alice)
    show = create_show(server, root_token)

    response = put_season(server, root_token, show["id"], 1, {"name": "Season One", "cover_image_id": attached})

    assert response.status_code == 409, f"gallery media accepted: {response.status_code} {response.text}"
    assert read_show(server, root_token, show["id"]).json()["seasons"] == []


@allure.epic("Series")
@allure.feature("REQ-SERIES-010")
@allure.story("SCN-SERIES-010-F")
def test_a_standard_user_cannot_write_a_season(server: Sut, root_token: str) -> None:
    """A Standard user's season write answers 403 and creates no season."""
    _alice_id, alice = provision(server, root_token, "alice")
    show = create_show(server, root_token)

    response = put_season(server, alice, show["id"], 1, {"name": "Season One"})

    assert response.status_code == 403, f"standard write accepted: {response.status_code} {response.text}"
    assert read_show(server, root_token, show["id"]).json()["seasons"] == []


@allure.epic("Series")
@allure.feature("REQ-SERIES-011")
@allure.story("SCN-SERIES-011-A")
@allure.story("SCN-SERIES-011-C")
def test_a_season_is_renamed_and_re_covered_keeping_its_episodes(server: Sut, root_token: str) -> None:
    """A patch stores the name and cover, returns the season and leaves the episodes in place."""
    cover = library_image(server, root_token)
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"], 1, name="Season One")
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], 1, 1, name="Pilot")

    response = patch_season(server, root_token, show["id"], 1, {"name": "First Season", "cover_image_id": cover})

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    season = response.json()
    assert season["name"] == "First Season"
    assert season["cover_image_id"] == cover
    assert season["episode_count"] == 1
    episode = read_episode(server, root_token, show["id"], 1, 1)
    assert episode.status_code == 200, f"episode read failed: {episode.status_code} {episode.text}"
    assert episode.json()["name"] == "Pilot"
    assert episode.json()["video"]["id"] == video["id"]


@allure.epic("Series")
@allure.feature("REQ-SERIES-011")
@allure.story("SCN-SERIES-011-B")
def test_null_clears_a_season_name_and_cover(server: Sut, root_token: str) -> None:
    """A null clears the season's name and cover and the read reports both as null."""
    cover = library_image(server, root_token)
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"], 1, name="Season One", cover_image_id=cover)

    response = patch_season(server, root_token, show["id"], 1, {"name": None, "cover_image_id": None})

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["name"] is None
    assert body["cover_image_id"] is None
    assert "cover" not in body["_links"]

    stored = read_season(server, root_token, show["id"], 1).json()
    assert stored["name"] is None
    assert stored["cover_image_id"] is None


@allure.epic("Series")
@allure.feature("REQ-SERIES-011")
@allure.story("SCN-SERIES-011-D")
def test_an_unknown_or_malformed_season_cannot_be_patched(server: Sut, root_token: str) -> None:
    """An index the show does not hold answers 404 and a malformed index answers 400."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"], 1, name="Season One")

    unknown = patch_season(server, root_token, show["id"], 9, {"name": "Other"})
    malformed = patch_season(server, root_token, show["id"], "not-an-index", {"name": "Other"})

    assert unknown.status_code == 404, f"unknown season answered {unknown.status_code}: {unknown.text}"
    assert malformed.status_code == 400, f"malformed index answered {malformed.status_code}: {malformed.text}"
    assert read_season(server, root_token, show["id"], 1).json()["name"] == "Season One"


@allure.epic("Series")
@allure.feature("REQ-SERIES-011")
@allure.story("SCN-SERIES-011-E")
def test_a_standard_user_may_read_but_not_edit_a_season(server: Sut, root_token: str) -> None:
    """A Standard user reads the season but its patch answers 403 and stores nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"], 1, name="Season One")

    readable = read_season(server, alice, show["id"], 1)
    refused = patch_season(server, alice, show["id"], 1, {"name": "Hijacked"})

    assert readable.status_code == 200, f"standard read failed: {readable.status_code} {readable.text}"
    assert refused.status_code == 403, f"standard patch accepted: {refused.status_code} {refused.text}"
    assert read_season(server, root_token, show["id"], 1).json()["name"] == "Season One"


@allure.epic("Series")
@allure.feature("REQ-SERIES-012")
@allure.story("SCN-SERIES-012-A")
def test_an_episode_is_created_at_a_free_index(server: Sut, root_token: str) -> None:
    """A free index creates the episode and answers 201 with a Location and the representation."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    file, video = episode_video(server, root_token)

    response = put_episode(
        server,
        root_token,
        show["id"],
        video["id"],
        name="Pilot",
        plot="An opening.",
        air_date="1990-04-08",
    )

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    episode = response.json()
    assert episode["id"] == 1
    assert episode["name"] == "Pilot"
    assert episode["plot"] == "An opening."
    assert episode["air_date"] == "1990-04-08"
    assert response.headers["Location"] == link_of(episode, "self")
    assert_summary(episode, file, video)


@allure.epic("Series")
@allure.feature("REQ-SERIES-012")
@allure.story("SCN-SERIES-012-B")
def test_an_episode_is_replaced_keeping_its_address(server: Sut, root_token: str) -> None:
    """A second write at an existing index stores the new metadata, keeps the address and answers 200."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)
    created = create_episode(server, root_token, show["id"], video["id"], name="Pilot")

    response = put_episode(
        server,
        root_token,
        show["id"],
        video["id"],
        name="Pilot (corrected)",
        plot="A corrected plot.",
        air_date="1990-04-09",
    )

    assert response.status_code == 200, f"replace failed: {response.status_code} {response.text}"
    episode = response.json()
    assert link_of(episode, "self") == link_of(created, "self")
    assert episode["name"] == "Pilot (corrected)"
    assert episode["plot"] == "A corrected plot."
    assert episode["air_date"] == "1990-04-09"
    assert episode["video"]["id"] == video["id"]
    assert episode_ids(list_episodes(server, root_token, show["id"], 1)) == [1]


@allure.epic("Series")
@allure.feature("REQ-SERIES-012")
@allure.story("SCN-SERIES-012-C")
def test_replacing_an_episodes_video_unattaches_the_previous_one(server: Sut, root_token: str) -> None:
    """A different video replaces the stored one and the previous video becomes unattached."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _first_file, first_video = episode_video(server, root_token, "first.mp4")
    _second_file, second_video = episode_video(server, root_token, "second.mp4")
    create_episode(server, root_token, show["id"], first_video["id"], name="Pilot")

    replaced = put_episode(server, root_token, show["id"], second_video["id"], name="Pilot")

    assert replaced.status_code == 200, f"replace failed: {replaced.status_code} {replaced.text}"
    assert replaced.json()["video"]["id"] == second_video["id"]
    stored = read_episode(server, root_token, show["id"], 1, 1).json()
    assert stored["video"]["id"] == second_video["id"]

    released = put_episode(server, root_token, show["id"], first_video["id"], episode_index=2, name="Second")
    assert released.status_code == 201, f"the replaced video stayed attached: {released.status_code} {released.text}"


@allure.epic("Series")
@allure.feature("REQ-SERIES-012")
@allure.story("SCN-SERIES-012-D")
def test_a_video_attached_to_another_episode_is_refused(server: Sut, root_token: str) -> None:
    """A video another episode already takes answers 409 and creates nothing."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], name="Pilot")

    response = put_episode(server, root_token, show["id"], video["id"], episode_index=2, name="Second")

    assert response.status_code == 409, f"attached video accepted: {response.status_code} {response.text}"
    assert episode_ids(list_episodes(server, root_token, show["id"], 1)) == [1]


@allure.epic("Series")
@allure.feature("REQ-SERIES-012")
@allure.story("SCN-SERIES-012-E")
def test_unusable_episode_input_is_refused(server: Sut, root_token: str) -> None:
    """An unknown video, empty name or malformed air date answers 422 and creates nothing."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)
    unknown_video = video["id"] + 1_000_000

    responses = [
        put_episode(server, root_token, show["id"], unknown_video),
        put_episode(server, root_token, show["id"], video["id"], name=""),
        put_episode(server, root_token, show["id"], video["id"], air_date="08/04/1990"),
    ]

    for response in responses:
        assert response.status_code == 422, f"unusable input accepted: {response.status_code} {response.text}"
    assert episode_ids(list_episodes(server, root_token, show["id"], 1)) == []


@allure.epic("Series")
@allure.feature("REQ-SERIES-012")
@allure.story("SCN-SERIES-012-F")
def test_an_episode_write_of_an_unknown_parent_or_malformed_index_is_refused(server: Sut, root_token: str) -> None:
    """An unknown show or season answers 404 and an index that is not a 32-bit unsigned integer answers 400."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)

    unknown_show = put_episode(server, root_token, show["id"] + 1_000_000, video["id"])
    unknown_season = put_episode(server, root_token, show["id"], video["id"], 9)
    malformed_season = put_episode(server, root_token, show["id"], video["id"], "not-an-index")
    malformed_episode = put_episode(server, root_token, show["id"], video["id"], episode_index="not-an-index")

    assert unknown_show.status_code == 404, f"unknown show answered {unknown_show.status_code}: {unknown_show.text}"
    assert unknown_season.status_code == 404, (
        f"unknown season answered {unknown_season.status_code}: {unknown_season.text}"
    )
    assert malformed_season.status_code == 400, (
        f"malformed season answered {malformed_season.status_code}: {malformed_season.text}"
    )
    assert malformed_episode.status_code == 400, (
        f"malformed episode answered {malformed_episode.status_code}: {malformed_episode.text}"
    )
    assert episode_ids(list_episodes(server, root_token, show["id"], 1)) == []


@allure.epic("Series")
@allure.feature("REQ-SERIES-012")
@allure.story("SCN-SERIES-012-G")
def test_a_standard_user_cannot_write_an_episode(server: Sut, root_token: str) -> None:
    """A Standard user's episode write answers 403 and creates no episode."""
    _alice_id, alice = provision(server, root_token, "alice")
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)

    response = put_episode(server, alice, show["id"], video["id"])

    assert response.status_code == 403, f"standard write accepted: {response.status_code} {response.text}"
    assert episode_ids(list_episodes(server, root_token, show["id"], 1)) == []


@allure.epic("Series")
@allure.feature("REQ-SERIES-013")
@allure.story("SCN-SERIES-013-A")
@allure.story("SCN-SERIES-013-C")
def test_the_episode_metadata_is_updated_partially(server: Sut, root_token: str) -> None:
    """A patch stores each submitted field and leaves the backing video unchanged."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)
    create_episode(
        server,
        root_token,
        show["id"],
        video["id"],
        name="Pilot",
        plot="Old plot",
        air_date="1990-04-08",
    )

    response = patch_episode(
        server,
        root_token,
        show["id"],
        1,
        {"name": "Pilot (corrected)", "plot": "New plot", "air_date": "1990-04-09"},
    )

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    episode = response.json()
    assert episode["name"] == "Pilot (corrected)"
    assert episode["plot"] == "New plot"
    assert episode["air_date"] == "1990-04-09"
    assert episode["video"]["id"] == video["id"]

    stored = read_episode(server, root_token, show["id"], 1, 1).json()
    assert stored["name"] == "Pilot (corrected)"
    assert stored["video"]["id"] == video["id"]


@allure.epic("Series")
@allure.feature("REQ-SERIES-013")
@allure.story("SCN-SERIES-013-B")
def test_null_clears_the_air_date(server: Sut, root_token: str) -> None:
    """A null clears the episode's air date and the read reports it as null."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], air_date="1990-04-08")

    response = patch_episode(server, root_token, show["id"], 1, {"air_date": None})

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    assert response.json()["air_date"] is None
    assert read_episode(server, root_token, show["id"], 1, 1).json()["air_date"] is None


@allure.epic("Series")
@allure.feature("REQ-SERIES-013")
@allure.story("SCN-SERIES-013-D")
def test_an_unknown_episode_or_denied_caller_cannot_be_patched(server: Sut, root_token: str) -> None:
    """An unknown episode answers 404, a Standard user answers 403 and the stored episode survives."""
    _alice_id, alice = provision(server, root_token, "alice")
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], name="Pilot")

    unknown = patch_episode(server, root_token, show["id"], 9, {"name": "Other"})
    denied = patch_episode(server, alice, show["id"], 1, {"name": "Hijacked"})

    assert unknown.status_code == 404, f"unknown episode answered {unknown.status_code}: {unknown.text}"
    assert denied.status_code == 403, f"standard patch accepted: {denied.status_code}: {denied.text}"
    assert read_episode(server, root_token, show["id"], 1, 1).json()["name"] == "Pilot"


@allure.epic("Series")
@allure.feature("REQ-SERIES-013")
@allure.story("SCN-SERIES-013-E")
def test_a_malformed_index_cannot_be_patched(server: Sut, root_token: str) -> None:
    """A season or episode index that is not a 32-bit unsigned integer answers 400 and applies nothing."""
    show = create_show(server, root_token)
    create_season(server, root_token, show["id"])
    _file, video = episode_video(server, root_token)
    create_episode(server, root_token, show["id"], video["id"], name="Pilot")

    malformed_season = server.request(
        "PATCH",
        episode_href(server, show["id"], "not-an-index", 1),
        token=root_token,
        json={"name": "Other"},
    )
    malformed_episode = patch_episode(server, root_token, show["id"], "not-an-index", {"name": "Other"})

    assert malformed_season.status_code == 400, (
        f"malformed season answered {malformed_season.status_code}: {malformed_season.text}"
    )
    assert malformed_episode.status_code == 400, (
        f"malformed episode answered {malformed_episode.status_code}: {malformed_episode.text}"
    )
    assert read_episode(server, root_token, show["id"], 1, 1).json()["name"] == "Pilot"
