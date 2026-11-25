"""Black-box proof of the `movies` capability."""

import datetime
import json
from itertools import count

import pytest
from conftest import auth_path, login, provision, start_instance
from gallery import video_file
from harness import ROOT_ADMIN_USERNAME, Sut
from labels import TestType, alternate_flow, business_rule, negative, positive, scenario, security
from mnemorium_client.api.library import (
    get_movie,
    list_movies,
    patch_movie,
    post_movie,
)
from mnemorium_client.models import GetPermissionSummaryResourceKind, PatchMovieRequest, PostMovieRequest

# The seeded slots this module reads, documented in `seed/sql/0003_movies.sql`.
LISTED_FIRST = 13011
LISTED_SECOND = 13012
PAGE_FILMS = (13013, 13014, 13015)
DETAIL_FILM = 13016
BARE_FILM = 13017
REPEAT_FILM = 13018
PATCH_FILM = 13019
CLEAR_FILM = 13020

LISTED_COVER = 13218
DETAIL_COVER = 13219
PATCH_COVER = 13220

DETAIL_VIDEO = 13206
REPEAT_VIDEO = 13208
PATCH_VIDEO = 13209
ATTACHED_VIDEO = 13211
NEW_VIDEO = 13212
UNKNOWN_GENRE_VIDEO = 13213
EMPTY_NAME_VIDEO = 13214
EMPTY_GENRE_VIDEO = 13215
MALFORMED_DATE_VIDEO = 13216
MALFORMED_COUNTRY_VIDEO = 13217

UNKNOWN_ID = 999_999_999
UNKNOWN_GENRE = "Melodrama"

# What every seeded video declares, and the summary a film embeds.
CODEC = "h264"
DURATION_MS = 200.0
WIDTH = 16
HEIGHT = 16
SCAN_TYPE = "PROGRESSIVE"
SUMMARY_FIELDS = {"id", "file_id", "codec", "duration_ms", "width", "height", "scan_type"}

_ACCOUNTS = count()


def fresh_video(server: Sut, token: str) -> int:
    """Upload a video no film uses and return its typed identifier."""
    file = video_file(server, token, "film-creation.mp4")
    response = server.follow(file, "video", token=token)
    assert response.status_code == 200, f"video read failed: {response.status_code} {response.text}"
    return response.json()["id"]


def movie_collection(server: Sut, token: str, query: str = ""):
    """GET the film collection with an optional query string."""
    return server.request("GET", f"{server.href('movie')}{query}", token=token)


@business_rule
@scenario("SCN-MOVIES-001-A")
@scenario("SCN-MOVIES-001-D")
@scenario("SCN-MOVIES-002-A")
def test_the_collection_lists_partial_films_in_identifier_order(server: Sut, root_token: str) -> None:
    """The collection is ascending by identifier and each item is the partial representation."""
    response = list_movies.sync_detailed(client=server.client(root_token), limit=100)
    assert response.status_code == 200, f"list failed: {response.status_code} {response.content!r}"
    items = json.loads(response.content)["_embedded"]["items"]
    ids = [item["id"] for item in items]

    assert ids == sorted(ids), "the collection is not ordered by identifier"
    assert ids.index(LISTED_FIRST) < ids.index(LISTED_SECOND)

    covered = next(item for item in items if item["id"] == LISTED_FIRST)
    uncovered = next(item for item in items if item["id"] == LISTED_SECOND)
    assert set(covered) == {"cover_image_id", "id", "name", "_links"}
    assert covered["name"] == "Listed One"
    assert covered["cover_image_id"] == LISTED_COVER
    cover = server.follow(covered, "cover", token=root_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert json.loads(cover.content)["id"] == LISTED_COVER
    assert uncovered.get("cover_image_id") is None
    assert "cover" not in uncovered["_links"]


@business_rule
@scenario("SCN-MOVIES-001-B")
def test_the_name_filter_and_page_are_applied(server: Sut, root_token: str) -> None:
    """The name filter narrows the collection and limit/offset select one page."""
    filtered = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Page Film",
        limit=100,
    )
    assert filtered.status_code == 200, f"filter failed: {filtered.status_code} {filtered.content!r}"
    assert [item["id"] for item in json.loads(filtered.content)["_embedded"]["items"]] == list(PAGE_FILMS)

    paged = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Page Film",
        limit=1,
        offset=1,
    )
    assert paged.status_code == 200, f"page failed: {paged.status_code} {paged.content!r}"
    assert [item["id"] for item in json.loads(paged.content)["_embedded"]["items"]] == [PAGE_FILMS[1]]


@business_rule
@scenario("SCN-MOVIES-001-C")
@pytest.mark.own_instance(reason="exact_collection")
def test_a_caller_that_discovers_no_film_lists_an_empty_collection(sut: Sut) -> None:
    """A caller denied the only film discovers none and receives an empty collection."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    alice_id, alice = provision(server, root_token, f"filmdenied{next(_ACCOUNTS)}")
    created = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(name="Hidden Film", video_id=fresh_video(server, root_token)),
    )
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"
    film = json.loads(created.content)

    before = movie_collection(server, alice)
    denied = server.request(
        "PUT",
        f"{server.href('permission')}/resource/MOVIE/{film['id']}",
        token=root_token,
        json={
            "visibility": "PUBLIC",
            "grants": [{"principal": {"kind": "USER", "id": alice_id}, "role": "VIEWER", "effect": "DENY"}],
        },
    )
    after = movie_collection(server, alice)

    assert [item["id"] for item in json.loads(before.content)["_embedded"]["items"]] == [film["id"]]
    assert denied.status_code == 200, f"deny failed: {denied.status_code} {denied.text}"
    assert after.status_code == 200
    assert json.loads(after.content)["_embedded"]["items"] == []


@scenario("SCN-MOVIES-001-E", TestType.BOUNDARY)
@scenario("SCN-MOVIES-001-F", TestType.NEGATIVE)
def test_an_out_of_range_limit_and_a_malformed_query_are_refused(server: Sut, root_token: str) -> None:
    """A limit outside 1..100 answers 422 and an undecodable query answers 400, each with one message."""
    out_of_range = [movie_collection(server, root_token, query) for query in ("?limit=0", "?limit=101")]
    assert {response.status_code for response in out_of_range} == {422}
    assert len({json.loads(response.content)["error"] for response in out_of_range}) == 1

    malformed = [movie_collection(server, root_token, query) for query in ("?limit=abc", "?offset=abc")]
    assert {response.status_code for response in malformed} == {400}
    assert len({json.loads(response.content)["error"] for response in malformed}) == 1


@positive
@scenario("SCN-MOVIES-003-A")
@scenario("SCN-VIDEO-005-A")
def test_the_detail_read_carries_the_film_metadata(server: Sut, root_token: str) -> None:
    """The read carries the metadata, ordered genres, the video summary and the cover."""
    response = get_movie.sync_detailed(DETAIL_FILM, client=server.client(root_token))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    film = json.loads(response.content)

    assert film["name"] == "Some Like It Hot"
    assert film["plot"] == "Two musicians on the run"
    assert film["country_of_origin"] == "US"
    assert film["release_date"] == "1959-08-17"
    assert film["genres"] == ["COMEDY", "DRAMA"]
    assert film["cover_image_id"] == DETAIL_COVER

    summary = film["video"]
    assert set(summary) == SUMMARY_FIELDS
    assert summary["id"] == DETAIL_VIDEO
    assert summary["file_id"] == DETAIL_VIDEO - 100
    assert summary["codec"].lower() == CODEC
    assert abs(summary["duration_ms"] - DURATION_MS) <= 1.0
    assert summary["width"] == WIDTH
    assert summary["height"] == HEIGHT
    assert summary["scan_type"] == SCAN_TYPE

    cover = server.follow(film, "cover", token=root_token)
    assert cover.status_code == 200, f"cover read failed: {cover.status_code} {cover.text}"
    assert json.loads(cover.content)["id"] == DETAIL_COVER


@positive
@scenario("SCN-MOVIES-003-B")
def test_a_film_without_a_date_or_country_reports_null(server: Sut, root_token: str) -> None:
    """A film that stores neither a release date nor a country reports both as null."""
    response = get_movie.sync_detailed(BARE_FILM, client=server.client(root_token))
    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    film = json.loads(response.content)

    assert film["release_date"] is None
    assert film["country_of_origin"] is None


@negative
@scenario("SCN-MOVIES-003-D")
def test_a_malformed_film_identifier_is_refused(server: Sut, root_token: str) -> None:
    """A movie_id that is not an identifier answers 400 with the stable message."""
    response = server.request("GET", f"{server.href('movie')}/not-an-identifier", token=root_token)

    assert response.status_code == 400
    assert json.loads(response.content)["error"] == "invalid film identifier"


@positive
@scenario("SCN-MOVIES-004-A")
def test_a_new_film_is_created_from_a_free_video(server: Sut, root_token: str) -> None:
    """An unused video creates the film and answers 201 with a Location."""
    response = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(
            name="Newly Created Film",
            video_id=NEW_VIDEO,
            plot="Two musicians on the run",
            release_date=datetime.date(1959, 8, 17),
            country_of_origin="us",
            genres=["Comedy"],
        ),
    )
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    film = json.loads(response.content)

    assert film["id"] > 0
    assert film["name"] == "Newly Created Film"
    assert film["plot"] == "Two musicians on the run"
    assert film["country_of_origin"] == "US"
    assert film["release_date"] == "1959-08-17"
    assert film["genres"] == ["COMEDY"]
    assert film["video"]["id"] == NEW_VIDEO
    assert response.headers["Location"] == film["_links"]["self"]["href"]

    listed = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Newly Created Film",
    )
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
    assert [item["id"] for item in json.loads(listed.content)["_embedded"]["items"]] == [film["id"]]


@business_rule
@scenario("SCN-MOVIES-004-B")
def test_a_repeated_creation_returns_the_existing_film(server: Sut, root_token: str) -> None:
    """The same video answers 200 with the film it already backs and applies no change."""
    response = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(
            name="A Different Title",
            video_id=REPEAT_VIDEO,
            plot="A different plot",
        ),
    )
    assert response.status_code == 200, f"repeat failed: {response.status_code} {response.content!r}"
    film = json.loads(response.content)

    assert film["id"] == REPEAT_FILM
    assert film["name"] == "The Original"
    assert film["plot"] == ""
    listed = list_movies.sync_detailed(
        client=server.client(root_token),
        name="A Different Title",
    )
    assert json.loads(listed.content)["_embedded"]["items"] == []


@negative
@scenario("SCN-MOVIES-004-C")
def test_a_video_attached_to_an_episode_is_refused(server: Sut, root_token: str) -> None:
    """A video the seeded episode already takes answers 409 and creates no film."""
    response = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(name="Attached Film", video_id=ATTACHED_VIDEO),
    )
    assert response.status_code == 409, f"attached video accepted: {response.status_code} {response.content!r}"

    listed = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Attached Film",
    )
    assert json.loads(listed.content)["_embedded"]["items"] == [], "a refused creation stored a film"


@alternate_flow
@scenario("SCN-MOVIES-004-D")
def test_an_unknown_genre_is_stored_and_linked(server: Sut, root_token: str) -> None:
    """A genre the catalogue does not hold is stored and the film reports it."""
    response = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(
            name="Unknown Genre Film",
            video_id=UNKNOWN_GENRE_VIDEO,
            genres=[UNKNOWN_GENRE],
        ),
    )
    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    created = json.loads(response.content)

    assert created["genres"] == [UNKNOWN_GENRE.upper()]
    read = get_movie.sync_detailed(created["id"], client=server.client(root_token))
    assert read.status_code == 200, f"read failed: {read.status_code} {read.content!r}"
    assert json.loads(read.content)["genres"] == [UNKNOWN_GENRE.upper()]


@negative
@scenario("SCN-MOVIES-004-E")
def test_a_film_with_an_unknown_video_is_refused(server: Sut, root_token: str) -> None:
    """A video the instance does not hold answers 422 and stores nothing."""
    response = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(name="Missing Video Film", video_id=UNKNOWN_ID),
    )

    assert response.status_code == 422, f"unknown video accepted: {response.status_code} {response.content!r}"
    listed = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Missing Video Film",
    )
    assert json.loads(listed.content)["_embedded"]["items"] == [], "a refused creation stored a film"


@negative
@scenario("SCN-MOVIES-004-E")
def test_a_film_with_an_empty_or_padded_name_is_refused(server: Sut, root_token: str) -> None:
    """An empty name, or one that begins or ends with whitespace, answers 422 and stores nothing."""
    for name in ("", " Padded Film", "Padded Film "):
        response = post_movie.sync_detailed(
            client=server.client(root_token),
            body=PostMovieRequest(name=name, video_id=EMPTY_NAME_VIDEO),
        )

        assert response.status_code == 422, f"{name!r} accepted: {response.status_code} {response.content!r}"
        listed = list_movies.sync_detailed(client=server.client(root_token), name=name)
        assert json.loads(listed.content)["_embedded"]["items"] == [], "a refused creation stored a film"


@negative
@scenario("SCN-MOVIES-004-E")
def test_a_film_with_an_empty_genre_is_refused(server: Sut, root_token: str) -> None:
    """A genre identifier that is empty answers 422 and stores nothing."""
    response = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(
            name="Empty Genre Film",
            video_id=EMPTY_GENRE_VIDEO,
            genres=[""],
        ),
    )

    assert response.status_code == 422, f"empty genre accepted: {response.status_code} {response.content!r}"
    listed = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Empty Genre Film",
    )
    assert json.loads(listed.content)["_embedded"]["items"] == [], "a refused creation stored a film"


@negative
@scenario("SCN-MOVIES-004-E")
def test_a_film_with_a_malformed_date_is_refused(server: Sut, root_token: str) -> None:
    """A release date that is not YYYY-MM-DD answers 422 and stores nothing."""
    response = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(
            name="Malformed Date Film",
            video_id=MALFORMED_DATE_VIDEO,
            release_date="17/08/1959",
        ),
    )

    assert response.status_code == 422, f"malformed date accepted: {response.status_code} {response.content!r}"
    listed = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Malformed Date Film",
    )
    assert json.loads(listed.content)["_embedded"]["items"] == [], "a refused creation stored a film"


@negative
@scenario("SCN-MOVIES-004-E")
def test_a_film_with_a_malformed_country_is_refused(server: Sut, root_token: str) -> None:
    """A country of origin that is not a two-letter code answers 422 and stores nothing."""
    response = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(
            name="Malformed Country Film",
            video_id=MALFORMED_COUNTRY_VIDEO,
            country_of_origin="U.S.A.",
        ),
    )

    assert response.status_code == 422, f"malformed country accepted: {response.status_code} {response.content!r}"
    listed = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Malformed Country Film",
    )
    assert json.loads(listed.content)["_embedded"]["items"] == [], "a refused creation stored a film"


@positive
@scenario("SCN-MOVIES-005-A")
@scenario("SCN-MOVIES-005-C")
def test_the_metadata_is_updated_partially(server: Sut, root_token: str) -> None:
    """A patch stores each submitted field and leaves the backing video unchanged."""
    response = patch_movie.sync_detailed(
        PATCH_FILM,
        client=server.client(root_token),
        body=PatchMovieRequest(
            name="New Name",
            plot="New plot",
            country_of_origin="gb",
            release_date=datetime.date(1960, 1, 2),
            genres=["Comedy"],
            cover_image_id=PATCH_COVER,
        ),
    )
    assert response.status_code == 200, f"patch failed: {response.status_code} {response.content!r}"
    film = json.loads(response.content)

    assert film["name"] == "New Name"
    assert film["plot"] == "New plot"
    assert film["country_of_origin"] == "GB"
    assert film["release_date"] == "1960-01-02"
    assert film["genres"] == ["COMEDY"]
    assert film["cover_image_id"] == PATCH_COVER
    assert film["video"]["id"] == PATCH_VIDEO

    stored = get_movie.sync_detailed(PATCH_FILM, client=server.client(root_token))
    assert stored.status_code == 200, f"read failed: {stored.status_code} {stored.content!r}"
    stored_film = json.loads(stored.content)
    assert stored_film["name"] == "New Name"
    assert stored_film["video"]["id"] == PATCH_VIDEO


@negative
@scenario("SCN-MOVIES-005-G")
def test_a_padded_film_name_is_refused_on_update(server: Sut, root_token: str) -> None:
    """A patch name that begins or ends with whitespace answers 422 and applies nothing."""
    for name in (" Padded Film", "Padded Film ", " Padded Film "):
        response = patch_movie.sync_detailed(
            DETAIL_FILM,
            client=server.client(root_token),
            body=PatchMovieRequest(name=name),
        )
        assert response.status_code == 422, f"{name!r} accepted: {response.status_code} {response.content!r}"

    stored = get_movie.sync_detailed(DETAIL_FILM, client=server.client(root_token))
    assert stored.status_code == 200, f"read failed: {stored.status_code} {stored.content!r}"
    assert json.loads(stored.content)["name"] == "Some Like It Hot"


@negative
@scenario("SCN-MOVIES-005-F")
def test_a_malformed_film_identifier_cannot_be_patched(server: Sut, root_token: str) -> None:
    """A patch addressing a non-identifier answers 400 and changes no film."""
    response = server.request(
        "PATCH",
        f"{server.href('movie')}/not-an-identifier",
        token=root_token,
        json={"name": "Hijacked Film"},
    )
    listed = list_movies.sync_detailed(client=server.client(root_token), name="Hijacked Film")

    assert response.status_code == 400
    assert json.loads(response.content)["error"] == "invalid film identifier"
    assert json.loads(listed.content)["_embedded"]["items"] == []


@alternate_flow
@scenario("SCN-MOVIES-005-B")
def test_null_clears_an_optional_value(server: Sut, root_token: str) -> None:
    """A null clears the release date, country and cover and the read reports each as null."""
    response = patch_movie.sync_detailed(
        CLEAR_FILM,
        client=server.client(root_token),
        body=PatchMovieRequest(
            release_date=None,
            country_of_origin=None,
            cover_image_id=None,
        ),
    )
    assert response.status_code == 200, f"patch failed: {response.status_code} {response.content!r}"
    film = json.loads(response.content)

    assert film["release_date"] is None
    assert film["country_of_origin"] is None
    assert film["cover_image_id"] is None
    assert "cover" not in film["_links"]

    stored = get_movie.sync_detailed(CLEAR_FILM, client=server.client(root_token))
    assert stored.status_code == 200, f"read failed: {stored.status_code} {stored.content!r}"
    stored_film = json.loads(stored.content)
    assert stored_film["release_date"] is None
    assert stored_film["country_of_origin"] is None
    assert stored_film["cover_image_id"] is None


@security
@scenario("SCN-MOVIES-006-A")
@scenario("SCN-MOVIES-006-B")
@scenario("SCN-MOVIES-005-D")
def test_a_standard_user_reads_but_cannot_write_a_film(server: Sut, root_token: str) -> None:
    """A Standard user reads a film, while a rename answers 403 and applies nothing."""
    _, alice = provision(server, root_token, f"filmreader{next(_ACCOUNTS)}")

    read = get_movie.sync_detailed(DETAIL_FILM, client=server.client(alice))
    renamed = patch_movie.sync_detailed(
        DETAIL_FILM,
        client=server.client(alice),
        body=PatchMovieRequest(name="Hijacked Film"),
    )
    stored = get_movie.sync_detailed(DETAIL_FILM, client=server.client(root_token))

    assert read.status_code == 200, f"standard read failed: {read.status_code} {read.content!r}"
    assert renamed.status_code == 403, f"standard write accepted: {renamed.status_code} {renamed.content!r}"
    assert json.loads(stored.content)["name"] == "Some Like It Hot"


@security
@scenario("SCN-MOVIES-006-C")
def test_an_admin_curates_a_film(server: Sut, root_token: str) -> None:
    """An Admin writes the catalogue through the seeded role, leaving the value unchanged."""
    _, admin = provision(server, root_token, f"filmadmin{next(_ACCOUNTS)}", role="ADMIN")
    current = json.loads(get_movie.sync_detailed(DETAIL_FILM, client=server.client(root_token)).content)

    edited = patch_movie.sync_detailed(
        DETAIL_FILM,
        client=server.client(admin),
        body=PatchMovieRequest(name=current["name"]),
    )

    assert edited.status_code == 200, f"admin write refused: {edited.status_code} {edited.content!r}"
    assert json.loads(edited.content)["name"] == current["name"]


@security
@scenario("SCN-MOVIES-007-A")
@scenario("SCN-AUTHORIZATION-023-B")
def test_a_standard_user_cannot_create_a_film(server: Sut, root_token: str) -> None:
    """A Standard user creating a film answers 403 and creates nothing."""
    alice_id, alice = provision(server, root_token, f"filmcreator{next(_ACCOUNTS)}")
    name = f"Forbidden Film {alice_id}"

    created = post_movie.sync_detailed(
        client=server.client(alice),
        body=PostMovieRequest(name=name, video_id=fresh_video(server, root_token)),
    )
    listed = list_movies.sync_detailed(client=server.client(root_token), name=name)

    assert created.status_code == 403, f"standard creation accepted: {created.status_code} {created.content!r}"
    assert json.loads(listed.content)["_embedded"]["items"] == []


@security
@scenario("SCN-MOVIES-007-B")
def test_an_admin_creates_a_film(server: Sut, root_token: str) -> None:
    """An Admin creates a film under the catalogue and it is listed."""
    admin_id, admin = provision(server, root_token, f"filmadmin{next(_ACCOUNTS)}", role="ADMIN")
    name = f"Admin Film {admin_id}"

    created = post_movie.sync_detailed(
        client=server.client(admin),
        body=PostMovieRequest(name=name, video_id=fresh_video(server, root_token)),
    )
    listed = list_movies.sync_detailed(client=server.client(root_token), name=name)

    assert created.status_code == 201, f"admin creation failed: {created.status_code} {created.content!r}"
    film = json.loads(created.content)
    assert film["name"] == name
    assert [item["id"] for item in json.loads(listed.content)["_embedded"]["items"]] == [film["id"]]


@security
@scenario("SCN-MOVIES-007-C")
@scenario("SCN-MOVIES-004-G")
def test_a_denied_catalogue_refuses_film_creation(server: Sut, root_token: str) -> None:
    """A denied CreateChild on the catalogue refuses creation and the restored role admits it."""
    admin_id, admin = provision(server, root_token, f"filmadmin{next(_ACCOUNTS)}", role="ADMIN")
    href = f"{server.href('permission')}/resource/CATALOGUE/0"
    current = server.request("GET", href, token=root_token)
    assert current.status_code == 200, f"permission read failed: {current.status_code} {current.text}"
    visibility = json.loads(current.content)["visibility"]
    name = f"Denied Film {admin_id}"
    video_id = fresh_video(server, root_token)

    denied = server.request(
        "PUT",
        href,
        token=root_token,
        json={
            "visibility": visibility,
            "grants": [
                {
                    "principal": {"kind": "USER", "id": admin_id},
                    "role": "CONTRIBUTOR",
                    "effect": "DENY",
                    "child_kind": "MOVIE",
                }
            ],
        },
    )
    refused = post_movie.sync_detailed(
        client=server.client(admin),
        body=PostMovieRequest(name=name, video_id=video_id),
    )
    restored = server.request(
        "PUT",
        href,
        token=root_token,
        json={"visibility": visibility, "grants": []},
    )
    listed = list_movies.sync_detailed(client=server.client(root_token), name=name)

    assert denied.status_code == 200
    assert refused.status_code == 403, f"denied creation admitted: {refused.status_code} {refused.content!r}"
    assert restored.status_code == 200
    assert json.loads(listed.content)["_embedded"]["items"] == []


@business_rule
@scenario("SCN-AUTHORIZATION-002-F")
def test_a_film_follows_the_catalogue(server: Sut, root_token: str) -> None:
    """A created film's authorization parent is the catalogue."""
    created = post_movie.sync_detailed(
        client=server.client(root_token),
        body=PostMovieRequest(name=f"Auth Film {next(_ACCOUNTS)}", video_id=fresh_video(server, root_token)),
    )
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"
    film = json.loads(created.content)

    assert auth_path(server, root_token, GetPermissionSummaryResourceKind.MOVIE, film["id"]) == [
        "MOVIE",
        "CATALOGUE",
        "LIBRARY",
        "INSTANCE",
    ]
    assert auth_path(server, root_token, GetPermissionSummaryResourceKind.CATALOGUE, 0) == [
        "CATALOGUE",
        "LIBRARY",
        "INSTANCE",
    ]
