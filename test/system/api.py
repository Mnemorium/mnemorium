"""Black-box proof of the `api` capability's hypermedia semantics.

The declarative contract — media types, statuses, the error envelope and the
required `_links`/`_embedded` fields — is validated by Schemathesis from the
served document. These cases prove only what a schema cannot: that the entry
point is public and caller independent, that every link it publishes resolves,
and that following a representation's links reaches the resource it names.
"""

import allure
from conftest import FIXTURE_DIRECTORY, linked_id, provision, upload
from harness import API_PREFIX, SECRET_PASSWORD, Sut, expand_template, link_of

VIDEO_FIXTURES = ("clip.mp4", "episode.mp4")

# The index relations a client may follow without a credential.
PUBLIC_INDEX_RELATIONS = frozenset({"self", "login", "register", "token", "health"})


def create_movie(server: Sut, token: str, name: str, fixture: str = "clip.mp4", variant: int = 0) -> dict:
    """Upload a video, create a film on it and return the created representation."""
    video = (FIXTURE_DIRECTORY / fixture).read_bytes() + b"\x00" * variant
    file = upload(server, token, f"{name}-{fixture}-{variant}", video, "video/mp4")
    video_id = linked_id(link_of(file, "video"))
    created = server.request(
        "POST",
        server.href("movie"),
        token=token,
        json={"name": name, "video_id": video_id},
    )
    assert created.status_code == 201, f"movie failed: {created.status_code} {created.text}"
    return created.json()


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-A")
def test_the_index_is_public_and_names_itself(server: Sut) -> None:
    """An unauthenticated caller reaches the index, whose self link is the API base."""
    response = server.request("GET", API_PREFIX)

    assert response.status_code == 200, f"index failed: {response.status_code} {response.text}"
    assert link_of(response.json(), "self") == API_PREFIX


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-B")
def test_every_index_link_resolves_to_a_served_route(server: Sut, root_token: str) -> None:
    """Following each published link reaches a route rather than the routing fallback."""
    index = server.index()
    credential = link_of(index, "credential", templated=True)
    assert credential.endswith("/credential/{id}")

    for rel, link in index["_links"].items():
        href = expand_template(link["href"], id=1) if link.get("templated") else link["href"]
        response = server.request("GET", href, token=root_token)
        assert response.status_code != 404, f"{rel!r} -> {href!r} is unserved: {response.text}"
        assert response.status_code < 500, f"{rel!r} -> {href!r} failed: {response.status_code} {response.text}"


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-C")
def test_a_protected_index_link_answers_the_401_envelope(server: Sut) -> None:
    """Following an index link to a protected entry unauthenticated answers the 401 envelope."""
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


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-D")
def test_the_index_is_identical_for_two_callers(server: Sut, root_token: str) -> None:
    """The Root Admin and a Standard User read the same public document."""
    provision(server, root_token, "alice")
    alice_token = server.login("alice", SECRET_PASSWORD)

    root_index = server.request("GET", API_PREFIX, token=root_token)
    alice_index = server.request("GET", API_PREFIX, token=alice_token)

    assert root_index.status_code == alice_index.status_code == 200
    assert root_index.json() == alice_index.json()


@allure.epic("API")
@allure.feature("REQ-API-004")
@allure.story("SCN-API-004-B")
def test_the_current_user_reports_the_canonical_self(server: Sut, root_token: str) -> None:
    """The `/user/me` alias reports the account's canonical user resource."""
    response = server.request("GET", server.href("current_user"), token=root_token)

    assert response.status_code == 200, f"current user failed: {response.status_code} {response.text}"
    body = response.json()
    assert link_of(body, "self") == f"{server.href('user')}/{body['id']}"


@allure.epic("API")
@allure.feature("REQ-API-016")
@allure.story("SCN-API-016-D")
def test_an_embedded_catalog_item_reaches_its_detail(server: Sut, root_token: str) -> None:
    """The partial item's self link reaches the full film representation."""
    created = create_movie(server, root_token, "Arrival")
    page = server.request("GET", server.href("movie"), token=root_token)

    assert page.status_code == 200, f"catalog failed: {page.status_code} {page.text}"
    items = page.json()["_embedded"]["items"]
    item = next(item for item in items if item["id"] == created["id"])
    detail = server.request("GET", link_of(item, "self"), token=root_token)

    assert detail.status_code == 200, f"detail failed: {detail.status_code} {detail.text}"
    body = detail.json()
    assert body["id"] == created["id"]
    assert body["name"] == created["name"]
    assert body["video"]["id"] == created["video"]["id"]


@allure.epic("API")
@allure.feature("REQ-API-007")
@allure.story("SCN-API-007-B")
def test_a_collection_page_keeps_its_filter_and_page(server: Sut, root_token: str) -> None:
    """`self`, `next` and `search` describe the filtered page the caller requested."""
    create_movie(server, root_token, "Arrival", VIDEO_FIXTURES[0])
    create_movie(server, root_token, "Arrival", VIDEO_FIXTURES[1], variant=1)
    collection = server.href("movie")

    page = server.request("GET", f"{collection}?name=Arrival&limit=1&offset=0", token=root_token)

    assert page.status_code == 200, f"page failed: {page.status_code} {page.text}"
    body = page.json()
    self_href = link_of(body, "self")
    assert "name=Arrival" in self_href and "limit=1" in self_href and "offset=0" in self_href
    search = link_of(body, "search", templated=True)
    assert search == f"{collection}{{?name,limit,offset}}"
    next_href = link_of(body, "next")
    assert "name=Arrival" in next_href and "limit=1" in next_href and "offset=1" in next_href
