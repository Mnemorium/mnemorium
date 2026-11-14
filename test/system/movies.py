"""Black-box proof of the `movies` capability.

A film is created from a video, listed as a partial item, read in full and updated partially without changing that
video. This module proves what a schema cannot: that an unused video creates exactly one film, that an attached video
creates none, that an unknown genre is stored on demand, that the listing is partial and ordered, that a refusal stores
nothing, and that a caller who may read but not edit is refused while the stored film survives.

The declarative contract — the media types, the status of each operation, the representation's fields, its `_links` and
the request schema — is validated by Schemathesis from the served document. Every route comes from the API index
(`Sut.href`), from a representation's named link or from the response's own `_links.self`, so the suite keeps its single
`/api/v1` base literal in `harness.Sut.index`.

The video, image and gallery helpers come from the feature modules that own them, so this module uploads real media and
never fabricates a typed identifier.
"""

import time

import allure
from conftest import add_to_gallery, create_gallery, provision
from gallery import image_file
from harness import Sut, link_of
from video import assert_summary, create_episode, typed_video, upload_video

# The film resource kind the resource-policy route addresses.
MOVIE = "MOVIE"

# A genre the catalogue has never stored, distinct from every seeded genre.
UNKNOWN_GENRE = "Melodrama"


def film_body(video_id: int, **overrides: object) -> dict[str, object]:
    """A film-creation payload naming `video_id`, with optional overrides."""
    body: dict[str, object] = {"name": "Some Like It Hot", "video_id": video_id}
    body.update(overrides)
    return body


def post_movie(server: Sut, token: str, body: dict[str, object]):
    """POST a film creation and return the raw response."""
    return server.request("POST", server.href("movie"), token=token, json=body)


def create_film(server: Sut, token: str, video_id: int, **overrides: object) -> dict:
    """Create a film and return its representation."""
    response = post_movie(server, token, film_body(video_id, **overrides))
    assert response.status_code == 201, f"film creation failed: {response.status_code} {response.text}"
    return response.json()


def list_movies(server: Sut, token: str, query: str = ""):
    """GET the film collection with an optional query string."""
    return server.request("GET", f"{server.href('movie')}{query}", token=token)


def read_movie(server: Sut, token: str, movie_id: int):
    """GET one film by identifier."""
    return server.request("GET", f"{server.href('movie')}/{movie_id}", token=token)


def patch_movie(server: Sut, token: str, movie_id: int, body: dict[str, object]):
    """PATCH one film's metadata and return the raw response."""
    return server.request("PATCH", f"{server.href('movie')}/{movie_id}", token=token, json=body)


def collection_ids(response) -> list[int]:
    """The identifiers a film collection lists."""
    return [item["id"] for item in response.json()["_embedded"]["items"]]


def film_video(server: Sut, token: str, name: str = "film.mp4") -> tuple[dict, dict]:
    """Upload one video and return its file and typed representations."""
    file = upload_video(server, token, name)
    return file, typed_video(server, token, file)


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


def deny_viewer(server: Sut, token: str, user_id: int, movie_id: int) -> None:
    """Deny `user_id` the Viewer bundle on one film, so it may neither discover nor view it."""
    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{MOVIE}/{movie_id}",
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


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-004")
@allure.story("SCN-MOVIES-004-A")
def test_a_new_film_is_created_from_a_free_video(server: Sut, root_token: str) -> None:
    """An unused video creates the film and answers 201 with a Location."""
    _file, video = film_video(server, root_token)

    response = post_movie(
        server,
        root_token,
        film_body(
            video["id"],
            name="Some Like It Hot",
            plot="Two musicians on the run",
            release_date="1959-08-17",
            country_of_origin="us",
            genres=["Comedy"],
        ),
    )

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    film = response.json()
    assert film["id"] > 0
    assert film["name"] == "Some Like It Hot"
    assert film["plot"] == "Two musicians on the run"
    assert film["country_of_origin"] == "US"
    assert film["release_date"] == "1959-08-17"
    assert film["genres"] == ["COMEDY"]
    assert film["video"]["id"] == video["id"]
    assert response.headers["Location"] == link_of(film, "self")

    listed = list_movies(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [film["id"]]


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-004")
@allure.story("SCN-MOVIES-004-B")
def test_a_repeated_creation_returns_the_existing_film(server: Sut, root_token: str) -> None:
    """The same video answers 200 with the film it already backs and applies no change."""
    _file, video = film_video(server, root_token)
    created = create_film(server, root_token, video["id"], name="The Original")

    response = post_movie(
        server,
        root_token,
        film_body(video["id"], name="A Different Title", plot="A different plot"),
    )

    assert response.status_code == 200, f"repeat failed: {response.status_code} {response.text}"
    film = response.json()
    assert film["id"] == created["id"]
    assert film["name"] == "The Original"
    assert film["plot"] == ""
    assert collection_ids(list_movies(server, root_token)) == [created["id"]]


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-004")
@allure.story("SCN-MOVIES-004-C")
def test_a_video_attached_to_an_episode_is_refused(server: Sut, root_token: str) -> None:
    """A video an episode already takes answers 409 and creates no film."""
    _file, video = film_video(server, root_token)
    create_episode(server, root_token, video["id"])

    response = post_movie(server, root_token, film_body(video["id"]))

    assert response.status_code == 409, f"attached video accepted: {response.status_code} {response.text}"
    listed = list_movies(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [], "a refused creation stored a film"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-004")
@allure.story("SCN-MOVIES-004-D")
def test_an_unknown_genre_is_stored_and_linked(server: Sut, root_token: str) -> None:
    """A genre the catalogue does not hold is stored and the film reports it."""
    _file, video = film_video(server, root_token)

    created = create_film(server, root_token, video["id"], genres=[UNKNOWN_GENRE])

    assert created["genres"] == [UNKNOWN_GENRE.upper()]
    read = read_movie(server, root_token, created["id"])
    assert read.status_code == 200, f"read failed: {read.status_code} {read.text}"
    assert read.json()["genres"] == [UNKNOWN_GENRE.upper()]


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-004")
@allure.story("SCN-MOVIES-004-E")
def test_unusable_creation_input_is_refused(server: Sut, root_token: str) -> None:
    """An unknown video, empty name or genre, malformed date or country answers 422 and stores nothing."""
    _file, video = film_video(server, root_token)
    unknown_video = video["id"] + 1_000_000

    bodies = [
        film_body(unknown_video),
        film_body(video["id"], name=""),
        film_body(video["id"], genres=[""]),
        film_body(video["id"], release_date="17/08/1959"),
        film_body(video["id"], country_of_origin="U.S.A."),
    ]
    for body in bodies:
        response = post_movie(server, root_token, body)
        assert response.status_code == 422, f"unusable input accepted: {response.status_code} {response.text}"

    listed = list_movies(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [], "a refused creation stored a film"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-004")
@allure.story("SCN-MOVIES-004-F")
def test_a_cover_that_is_not_library_artwork_is_refused(server: Sut, root_token: str) -> None:
    """An image already used as gallery media answers 409 and creates no film."""
    _alice_id, alice = provision(server, root_token, "alice")
    attached = gallery_image(server, alice)
    _file, video = film_video(server, root_token)

    response = post_movie(server, root_token, film_body(video["id"], cover_image_id=attached))

    assert response.status_code == 409, f"gallery media accepted: {response.status_code} {response.text}"
    listed = list_movies(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [], "a refused creation stored a film"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-004")
@allure.story("SCN-MOVIES-004-G")
def test_a_standard_user_cannot_create_a_film(server: Sut, root_token: str) -> None:
    """A Standard user's creation answers 403 and stores nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    _file, video = film_video(server, root_token)

    response = post_movie(server, alice, film_body(video["id"]))

    assert response.status_code == 403, f"standard creation accepted: {response.status_code} {response.text}"
    listed = list_movies(server, root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.text}"
    assert collection_ids(listed) == [], "a refused creation stored a film"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-001")
@allure.story("SCN-MOVIES-001-A")
@allure.story("SCN-MOVIES-001-D")
@allure.feature("REQ-MOVIES-002")
@allure.story("SCN-MOVIES-002-A")
def test_the_collection_lists_partial_films_in_identifier_order(server: Sut, root_token: str) -> None:
    """Two films list as partial items by ascending identifier, with their cover links."""
    cover = library_image(server, root_token)
    _first_file, first_video = film_video(server, root_token, "first.mp4")
    _second_file, second_video = film_video(server, root_token, "second.mp4")
    first = create_film(server, root_token, first_video["id"], name="First", cover_image_id=cover)
    second = create_film(server, root_token, second_video["id"], name="Second")

    response = list_movies(server, root_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    body = response.json()
    items = body["_embedded"]["items"]
    assert collection_ids(response) == sorted([first["id"], second["id"]])
    assert collection_ids(response) == [first["id"], second["id"]]

    covered, uncovered = items
    assert covered["id"] == first["id"]
    assert covered["name"] == "First"
    assert covered["cover_image_id"] == cover
    assert link_of(covered, "self") == f"{server.href('movie')}/{first['id']}"
    for omitted in ("plot", "genres", "video"):
        assert omitted not in covered, f"a listing item carries {omitted}"

    asset = server.follow(covered, "cover", token=root_token)
    assert asset.status_code == 200, f"cover link failed: {asset.status_code} {asset.text}"
    assert asset.json()["id"] == cover
    assert "cover" not in uncovered["_links"]
    assert uncovered["cover_image_id"] is None


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-001")
@allure.story("SCN-MOVIES-001-B")
def test_the_name_filter_and_page_are_applied(server: Sut, root_token: str) -> None:
    """The name filter narrows the collection and limit/offset select one page."""
    names = ["Alpha", "Beta", "Gamma"]
    ids = []
    for index, name in enumerate(names):
        _file, video = film_video(server, root_token, f"page-{index}.mp4")
        ids.append(create_film(server, root_token, video["id"], name=name)["id"])

    filtered = list_movies(server, root_token, "?name=Beta")
    assert filtered.status_code == 200, f"filter failed: {filtered.status_code} {filtered.text}"
    assert collection_ids(filtered) == [ids[1]]

    paged = list_movies(server, root_token, "?limit=1&offset=1")
    assert paged.status_code == 200, f"page failed: {paged.status_code} {paged.text}"
    assert collection_ids(paged) == [ids[1]]


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-001")
@allure.story("SCN-MOVIES-001-C")
def test_a_caller_that_discovers_no_film_lists_an_empty_collection(server: Sut, root_token: str) -> None:
    """A caller denied the only film discovers none and receives an empty collection."""
    alice_id, alice = provision(server, root_token, "alice")
    _file, video = film_video(server, root_token)
    film = create_film(server, root_token, video["id"])
    deny_viewer(server, root_token, alice_id, film["id"])

    response = list_movies(server, alice)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.json()["_embedded"]["items"] == []


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-001")
@allure.story("SCN-MOVIES-001-E")
@allure.story("SCN-MOVIES-001-F")
def test_an_out_of_range_limit_and_a_malformed_query_are_refused(server: Sut, root_token: str) -> None:
    """A limit outside 1..100 answers 422 and an undecodable query answers 400."""
    for query in ("?limit=0", "?limit=101"):
        response = list_movies(server, root_token, query)
        assert response.status_code == 422, f"limit accepted: {response.status_code} {response.text}"

    malformed = list_movies(server, root_token, "?limit=abc")
    assert malformed.status_code == 400, f"query accepted: {malformed.status_code} {malformed.text}"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-003")
@allure.story("SCN-MOVIES-003-A")
def test_the_detail_read_carries_the_film_metadata(server: Sut, root_token: str) -> None:
    """The read carries the metadata, ordered genres, video summary and cover."""
    cover = library_image(server, root_token)
    file, video = film_video(server, root_token)
    created = create_film(
        server,
        root_token,
        video["id"],
        name="Some Like It Hot",
        plot="Two musicians on the run",
        country_of_origin="gb",
        release_date="1959-08-17",
        genres=["Drama", "Comedy"],
        cover_image_id=cover,
    )

    response = read_movie(server, root_token, created["id"])

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    film = response.json()
    assert film["name"] == "Some Like It Hot"
    assert film["plot"] == "Two musicians on the run"
    assert film["country_of_origin"] == "GB"
    assert film["release_date"] == "1959-08-17"
    assert film["genres"] == ["COMEDY", "DRAMA"]
    assert film["cover_image_id"] == cover
    assert_summary(film, file, video)
    asset = server.follow(film, "cover", token=root_token)
    assert asset.status_code == 200, f"cover link failed: {asset.status_code} {asset.text}"
    assert asset.json()["id"] == cover


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-003")
@allure.story("SCN-MOVIES-003-B")
def test_a_film_without_a_date_or_country_reports_null(server: Sut, root_token: str) -> None:
    """A film that stores neither a release date nor a country reports both as null."""
    _file, video = film_video(server, root_token)
    created = create_film(server, root_token, video["id"], name="Undated")

    response = read_movie(server, root_token, created["id"])

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    film = response.json()
    assert film["release_date"] is None
    assert film["country_of_origin"] is None


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-003")
@allure.story("SCN-MOVIES-003-C")
def test_an_unknown_or_unviewable_film_is_not_found(server: Sut, root_token: str) -> None:
    """An unknown film and one the caller may not view both answer 404."""
    alice_id, alice = provision(server, root_token, "alice")
    _file, video = film_video(server, root_token)
    film = create_film(server, root_token, video["id"])
    deny_viewer(server, root_token, alice_id, film["id"])

    unknown = read_movie(server, root_token, film["id"] + 1_000_000)
    unviewable = read_movie(server, alice, film["id"])

    assert unknown.status_code == 404, f"unknown film answered {unknown.status_code}: {unknown.text}"
    assert unviewable.status_code == 404, f"unviewable film answered {unviewable.status_code}: {unviewable.text}"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-003")
@allure.story("SCN-MOVIES-003-D")
def test_a_malformed_film_identifier_is_refused(server: Sut, root_token: str) -> None:
    """A movie_id that is not an identifier answers 400."""
    response = read_movie(server, root_token, "not-an-identifier")

    assert response.status_code == 400, f"malformed id answered {response.status_code}: {response.text}"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-005")
@allure.story("SCN-MOVIES-005-A")
@allure.story("SCN-MOVIES-005-C")
def test_the_metadata_is_updated_partially(server: Sut, root_token: str) -> None:
    """A patch stores each submitted field and leaves the backing video unchanged."""
    cover = library_image(server, root_token)
    _file, video = film_video(server, root_token)
    created = create_film(
        server,
        root_token,
        video["id"],
        name="Old Name",
        plot="Old plot",
        country_of_origin="US",
        release_date="1959-08-17",
        genres=["Drama"],
    )

    response = patch_movie(
        server,
        root_token,
        created["id"],
        {
            "name": "New Name",
            "plot": "New plot",
            "country_of_origin": "gb",
            "release_date": "1960-01-02",
            "genres": ["Comedy"],
            "cover_image_id": cover,
        },
    )

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    film = response.json()
    assert film["name"] == "New Name"
    assert film["plot"] == "New plot"
    assert film["country_of_origin"] == "GB"
    assert film["release_date"] == "1960-01-02"
    assert film["genres"] == ["COMEDY"]
    assert film["cover_image_id"] == cover
    assert film["video"]["id"] == video["id"]

    stored = read_movie(server, root_token, created["id"]).json()
    assert stored["name"] == "New Name"
    assert stored["video"]["id"] == video["id"]


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-005")
@allure.story("SCN-MOVIES-005-B")
def test_null_clears_an_optional_value(server: Sut, root_token: str) -> None:
    """A null clears the release date, country and cover and the read reports each as null."""
    cover = library_image(server, root_token)
    _file, video = film_video(server, root_token)
    created = create_film(
        server,
        root_token,
        video["id"],
        country_of_origin="US",
        release_date="1959-08-17",
        cover_image_id=cover,
    )

    response = patch_movie(
        server,
        root_token,
        created["id"],
        {"release_date": None, "country_of_origin": None, "cover_image_id": None},
    )

    assert response.status_code == 200, f"patch failed: {response.status_code} {response.text}"
    film = response.json()
    assert film["release_date"] is None
    assert film["country_of_origin"] is None
    assert film["cover_image_id"] is None
    assert "cover" not in film["_links"]

    stored = read_movie(server, root_token, created["id"]).json()
    assert stored["release_date"] is None
    assert stored["country_of_origin"] is None
    assert stored["cover_image_id"] is None


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-005")
@allure.story("SCN-MOVIES-005-D")
def test_a_standard_user_may_read_but_not_edit(server: Sut, root_token: str) -> None:
    """A Standard user reads the film but its patch answers 403 and stores nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    _file, video = film_video(server, root_token)
    created = create_film(server, root_token, video["id"], name="Untouched")

    readable = read_movie(server, alice, created["id"])
    refused = patch_movie(server, alice, created["id"], {"name": "Hijacked"})

    assert readable.status_code == 200, f"standard read failed: {readable.status_code} {readable.text}"
    assert refused.status_code == 403, f"standard patch accepted: {refused.status_code} {refused.text}"
    assert read_movie(server, root_token, created["id"]).json()["name"] == "Untouched"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-005")
@allure.story("SCN-MOVIES-005-E")
def test_an_unknown_or_unviewable_film_cannot_be_patched(server: Sut, root_token: str) -> None:
    """An unknown film and one the caller may not view both answer 404 and apply nothing."""
    alice_id, alice = provision(server, root_token, "alice")
    _file, video = film_video(server, root_token)
    created = create_film(server, root_token, video["id"], name="Untouched")
    deny_viewer(server, root_token, alice_id, created["id"])

    unknown = patch_movie(server, root_token, created["id"] + 1_000_000, {"name": "Other"})
    unviewable = patch_movie(server, alice, created["id"], {"name": "Hijacked"})

    assert unknown.status_code == 404, f"unknown film answered {unknown.status_code}: {unknown.text}"
    assert unviewable.status_code == 404, f"unviewable film answered {unviewable.status_code}: {unviewable.text}"
    assert read_movie(server, root_token, created["id"]).json()["name"] == "Untouched"


@allure.epic("Movies")
@allure.feature("REQ-MOVIES-005")
@allure.story("SCN-MOVIES-005-F")
def test_a_malformed_film_identifier_cannot_be_patched(server: Sut, root_token: str) -> None:
    """A movie_id that is not an identifier answers 400 and applies nothing."""
    response = patch_movie(server, root_token, "not-an-identifier", {"name": "Other"})

    assert response.status_code == 400, f"malformed id answered {response.status_code}: {response.text}"
