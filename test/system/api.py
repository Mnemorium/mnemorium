"""Black-box proof of the `api` capability's hypermedia semantics."""

import pytest
from conftest import (
    FIXTURE_DIRECTORY,
    linked_id,
    login,
    provision,
    start_instance,
    upload,
)
from harness import (
    API_PREFIX,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    Sut,
    expand_template,
    link_of,
)
from labels import scenario
from mnemorium_client.api.library import list_movies, post_movie
from mnemorium_client.api.system import get_api_index
from mnemorium_client.api.user import get_me
from mnemorium_client.models import Link, MovieResponse, PostMovieRequest

VIDEO_FIXTURES = ("clip.mp4", "episode.mp4")

# The index relations a client may follow without a credential.
PUBLIC_INDEX_RELATIONS = frozenset({"self", "login", "register", "token", "health"})


def plain_href(link: Link) -> str:
    """The href of a link a representation publishes as a reference rather than a URI template."""
    assert link.templated is not True, f"the link {link.href!r} is a URI template"
    return link.href


def create_movie(
    server: Sut,
    token: str,
    name: str,
    fixture: str = "clip.mp4",
    variant: int = 0,
) -> MovieResponse:
    """Upload a video, create a film on it and return the created representation."""
    video = (FIXTURE_DIRECTORY / fixture).read_bytes() + b"\x00" * variant
    file = upload(server, token, f"{name}-{fixture}-{variant}", video, "video/mp4")
    video_id = linked_id(link_of(file, "video"))
    created = post_movie.sync_detailed(
        client=server.client(token),
        body=PostMovieRequest(name=name, video_id=video_id),
    )
    assert created.status_code == 201, f"movie failed: {created.status_code} {created.content!r}"
    film = created.parsed
    assert isinstance(film, MovieResponse), f"movie failed: {created.content!r}"
    return film


@scenario("SCN-API-015-A")
def test_the_index_is_public_and_names_itself(server: Sut) -> None:
    """An unauthenticated caller reaches the index, whose self link is the API base."""
    response = get_api_index.sync_detailed(client=server.client())

    assert response.status_code == 200, f"index failed: {response.status_code} {response.content!r}"
    assert plain_href(response.parsed.field_links.self_) == API_PREFIX


@scenario("SCN-API-015-B")
def test_every_index_link_resolves_to_a_served_route(server: Sut, root_token: str) -> None:
    """Following each published link reaches a route rather than the routing fallback."""
    # The walk follows the href each relation publishes, so the requests stay hand-built.
    index = server.index()
    credential = link_of(index, "credential", templated=True)
    assert credential.endswith("/credential/{id}")

    for rel, link in index["_links"].items():
        href = expand_template(link["href"], id=1) if link.get("templated") else link["href"]
        response = server.request("GET", href, token=root_token)
        assert response.status_code != 404, f"{rel!r} -> {href!r} is unserved: {response.text}"
        assert response.status_code < 500, f"{rel!r} -> {href!r} failed: {response.status_code} {response.text}"


@scenario("SCN-API-015-C")
def test_a_protected_index_link_answers_the_401_envelope(server: Sut) -> None:
    """Following an index link to a protected entry unauthenticated answers the 401 envelope."""
    # The entry points are the ones the index publishes, so each is followed by its own href.
    index = server.index()
    protected = set(index["_links"]) - PUBLIC_INDEX_RELATIONS
    assert protected, "the index publishes no protected entry"

    for rel in sorted(protected):
        link = index["_links"][rel]
        href = expand_template(link["href"], id=1) if link.get("templated") else link["href"]
        response = server.request("GET", href)

        assert response.status_code == 401, f"{rel!r} -> {href!r}: {response.status_code} {response.text}"
        assert response.headers["Content-Type"].startswith("application/hal+json")
        body = response.json()
        assert set(body) == {"error", "_links"}, f"{rel!r} -> {href!r} is not the envelope"
        assert isinstance(body["error"], str) and body["error"]
        assert body["_links"]["self"]["href"] == href


@scenario("SCN-API-015-D")
@pytest.mark.own_instance(reason="exact_collection")
def test_the_index_is_identical_for_two_callers(sut: Sut) -> None:
    """The Root Admin and a Standard User read the same public document."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    provision(server, root_token, "alice")
    alice_token = login(server, "alice", SECRET_PASSWORD)

    root_index = get_api_index.sync_detailed(client=server.client(root_token))
    alice_index = get_api_index.sync_detailed(client=server.client(alice_token))

    assert root_index.status_code == alice_index.status_code == 200
    assert root_index.parsed.to_dict() == alice_index.parsed.to_dict()


@scenario("SCN-API-004-B")
def test_the_current_user_reports_the_canonical_self(server: Sut, root_token: str) -> None:
    """The `/user/me` alias reports the account's canonical user resource."""
    response = get_me.sync_detailed(client=server.client(root_token))

    assert response.status_code == 200, f"current user failed: {response.status_code} {response.content!r}"
    current = response.parsed
    assert plain_href(current.field_links.self_) == f"{server.href('user')}/{current.id}"


@scenario("SCN-API-016-D")
@pytest.mark.own_instance(reason="exact_collection")
def test_an_embedded_catalog_item_reaches_its_detail(sut: Sut) -> None:
    """The partial item's self link reaches the full film representation."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    created = create_movie(server, root_token, "Arrival")
    page = list_movies.sync_detailed(client=server.client(root_token), limit=100)

    assert page.status_code == 200, f"catalog failed: {page.status_code} {page.content!r}"
    item = next(item for item in page.parsed.field_embedded.items if item.id == created.id)
    # The scenario is the published self link, so it is followed instead of the item route being rebuilt.
    detail = server.request("GET", plain_href(item.field_links.self_), token=root_token)

    assert detail.status_code == 200, f"detail failed: {detail.status_code} {detail.text}"
    film = MovieResponse.from_dict(detail.json())
    assert film.id == created.id
    assert film.name == created.name
    assert film.video.id == created.video.id


@scenario("SCN-API-007-B")
@pytest.mark.own_instance(reason="exact_collection")
def test_a_collection_page_keeps_its_filter_and_page(sut: Sut) -> None:
    """`self`, `next` and `search` describe the filtered page the caller requested."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    create_movie(server, root_token, "Arrival", VIDEO_FIXTURES[0])
    create_movie(server, root_token, "Arrival", VIDEO_FIXTURES[1], variant=1)
    collection = server.href("movie")

    page = list_movies.sync_detailed(
        client=server.client(root_token),
        name="Arrival",
        limit=1,
        offset=0,
    )

    assert page.status_code == 200, f"page failed: {page.status_code} {page.content!r}"
    links = page.parsed.field_links
    self_href = plain_href(links.self_)
    assert "name=Arrival" in self_href and "limit=1" in self_href and "offset=0" in self_href
    search = links.search
    assert search.templated is True, "the search link is not declared as a URI template"
    assert search.href == f"{collection}{{?name,limit,offset}}"
    next_link = links.next_
    assert isinstance(next_link, Link), "the first page publishes no next link"
    next_href = plain_href(next_link)
    assert "name=Arrival" in next_href and "limit=1" in next_href and "offset=1" in next_href
