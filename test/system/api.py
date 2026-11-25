"""Black-box proof of the `api` capability: its hypermedia semantics and the published document as a whole."""

import json
import re
from typing import Any
from urllib.parse import urlsplit

import pytest
from conftest import (
    FIXTURE_DIRECTORY,
    attempt,
    limiter_config,
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
from labels import alternate_flow, business_rule, error_handling, positive, scenario, security
from mnemorium_client.api.library import list_movies, post_movie
from mnemorium_client.api.system import get_api_index
from mnemorium_client.api.user import get_me
from mnemorium_client.models import Link, MovieResponse, PostMovieRequest

VIDEO_FIXTURES = ("clip.mp4", "episode.mp4")


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


HTTP_METHODS = frozenset({"get", "post", "put", "patch", "delete", "head", "options", "trace"})

# The bounded contexts the document publishes routes for.
CONTEXTS = frozenset({"asset", "identity", "library", "user"})

# The property suffixes whose values are dates or timestamps on the wire.
DATE_SUFFIX = "_date"
TIMESTAMP_SUFFIX = "_at"

# The names a request property or parameter carries when its values are a closed vocabulary.
VOCABULARIES = frozenset(
    {
        "action",
        "effect",
        "kind",
        "orientation",
        "production_type",
        "provider",
        "resource_kind",
        "role",
        "scan_type",
        "type",
        "visibility",
    }
)

# A whole-value alternation of literals, which is a closed vocabulary published as a pattern.
CLOSED_PATTERN = re.compile(r"^\^\(([A-Za-z0-9_|]+)\)\$$")

# The media type a stored-content response keeps.
WILDCARD = "*/*"


def openapi(server: Sut) -> dict[str, Any]:
    """Read the published document once."""
    response = server.request("GET", "/doc/openapi.json")
    assert response.status_code == 200, f"the document failed: {response.status_code} {response.text}"
    return response.json()


def component(document: dict[str, Any], schema: object) -> dict[str, Any]:
    """Resolve a `$ref` (and a nullable `oneOf`) to the schema it names."""
    schemas = document.get("components", {}).get("schemas", {})
    seen: set[str] = set()
    while isinstance(schema, dict):
        reference = schema.get("$ref")
        if isinstance(reference, str):
            name = reference.rsplit("/", 1)[-1]
            if name in seen:
                return schema
            seen.add(name)
            schema = schemas.get(name, {})
            continue
        alternatives = schema.get("oneOf")
        if isinstance(alternatives, list):
            concrete = [entry for entry in alternatives if entry.get("type") != "null"]
            if len(concrete) == 1:
                schema = concrete[0]
                continue
        break
    return schema if isinstance(schema, dict) else {}


def walk(node: object, where: str = "document"):
    """Yield every mapping in a document with a path for the failure message."""
    if isinstance(node, dict):
        yield where, node
        for key, value in node.items():
            yield from walk(value, f"{where}.{key}")
    elif isinstance(node, list):
        for index, value in enumerate(node):
            yield from walk(value, f"{where}[{index}]")


def iter_operations(document: dict[str, Any]):
    """Yield `(path, method, operation)` for every operation the document declares."""
    for path, item in document["paths"].items():
        for method, operation in item.items():
            if method.lower() in HTTP_METHODS and isinstance(operation, dict):
                yield path, method.lower(), operation


def iter_request_fields(document: dict[str, Any]):
    """Yield `(where, name, schema)` for every request body property and parameter."""
    for path, method, operation in iter_operations(document):
        label = f"{method.upper()} {path}"
        schema = operation.get("requestBody", {}).get("content", {}).get("application/json", {}).get("schema", {})
        for where, name, definition in iter_schema_properties(document, schema):
            yield f"{label} body.{where}", name, definition
        for parameter in operation.get("parameters", []):
            name = parameter.get("name")
            if parameter.get("in") in ("path", "query") and isinstance(name, str):
                yield f"{label} {parameter['in']}.{name}", name, parameter.get("schema", {})


def iter_schema_properties(document: dict[str, Any], schema: object, prefix: str = ""):
    """Yield `(where, name, schema)` for every property of a schema, nested objects included."""
    resolved = component(document, schema)
    for name, definition in resolved.get("properties", {}).items():
        yield f"{prefix}{name}", name, definition
        yield from iter_schema_properties(document, definition, f"{prefix}{name}.")


def expand_path(path: str) -> str:
    """Fill every path parameter with a value the route accepts."""
    return re.sub(r"\{[^}]+\}", "1", path)


@business_rule
@scenario("SCN-API-001-C")
@scenario("SCN-API-014-A")
@scenario("SCN-API-014-B")
def test_the_document_is_the_contract(server: Sut) -> None:
    """One server names the base, every context and operation is declared, and every declared path is served."""
    document = openapi(server)
    servers = document["servers"]
    paths = document["paths"]

    assert len(servers) == 1, f"the document declares {len(servers)} servers"
    assert urlsplit(servers[0]["url"]).path == API_PREFIX, servers[0]["url"]
    assert all(path.startswith("/") for path in paths), "a path is not a relative reference"
    assert not any(path.startswith(API_PREFIX) for path in paths), "a path repeats the base"
    assert {"/", "/health"} <= set(paths), "the index and the probe are not declared"

    schemes = document.get("components", {}).get("securitySchemes", {})
    assert "bearer_auth" in schemes, "the bearer scheme is not declared"
    assert schemes["bearer_auth"].get("type") == "http", schemes["bearer_auth"]
    assert schemes["bearer_auth"].get("scheme") == "bearer", schemes["bearer_auth"]

    contexts = {path.split("/", 2)[1] for path in paths if path.count("/") >= 2 and not path.startswith("/health")}
    assert contexts >= CONTEXTS, f"a context publishes no route: {sorted(CONTEXTS - contexts)}"

    for path, method, operation in iter_operations(document):
        label = f"{method.upper()} {path}"
        assert operation.get("operationId"), f"{label} declares no operationId"
        assert operation.get("security") is not None, f"{label} declares no security requirement"
        requirement = operation["security"]
        assert isinstance(requirement, list), f"{label} declares a malformed security requirement"
        assert all(isinstance(entry, dict) for entry in requirement), f"{label} declares a malformed requirement"
        assert all(set(entry) in (set(), {"bearer_auth"}) for entry in requirement), f"{label} declares another scheme"
        assert operation.get("responses"), f"{label} declares no response"

        # The route is served by the same registration that published it, so a
        # request reaches the router rather than the routing fallback. An
        # operation outside the business base (the probe) declares its own
        # server, which is the one the request uses.
        operation_servers = operation.get("servers") or servers
        operation_base = urlsplit(operation_servers[0]["url"]).path.rstrip("/")
        response = server.request("GET", f"{operation_base}{expand_path(path)}")
        assert response.status_code != 404, f"{label} is declared but not served: {response.status_code}"

    # The index is the published entry point: a link it carries is a declared
    # path, so the served surface and the document agree at every context.
    declared = {expand_path(path) for path in paths}
    index = server.index()
    for relation, link in index["_links"].items():
        href = expand_template(link["href"], id=1) if link.get("templated") else link["href"]
        path = urlsplit(href).path
        if path == API_PREFIX:
            path = "/"
        elif path.startswith(f"{API_PREFIX}/"):
            path = path[len(API_PREFIX) :]
        assert path in declared, f"the index link {relation!r} names {href!r}, which the document does not declare"


@positive
@scenario("SCN-API-017-A")
@scenario("SCN-API-017-B")
def test_every_date_and_timestamp_declares_its_format(server: Sut) -> None:
    """A date property carries `format: date` and a timestamp `format: date-time`."""
    document = openapi(server)
    problems: list[str] = []
    for where, schema in walk(document):
        name = where.rsplit(".", 1)[-1]
        declared = schema.get("format")
        if name.endswith(DATE_SUFFIX) and declared != "date":
            problems.append(f"{where} is a date without `format: date` (found {declared!r})")
        if name.endswith(TIMESTAMP_SUFFIX) and declared != "date-time":
            problems.append(f"{where} is a timestamp without `format: date-time` (found {declared!r})")
    assert not problems, "\n".join(problems)


@business_rule
@scenario("SCN-API-018-A")
@scenario("SCN-API-018-B")
def test_every_closed_vocabulary_is_an_enum(server: Sut) -> None:
    """Every closed-vocabulary request property and parameter publishes its values as an enum."""
    document = openapi(server)
    problems: list[str] = []
    for where, name, schema in iter_request_fields(document):
        resolved = component(document, schema)
        if name in VOCABULARIES and not resolved.get("enum"):
            problems.append(f"{where} publishes no enum")
        pattern = resolved.get("pattern")
        if isinstance(pattern, str) and CLOSED_PATTERN.fullmatch(pattern):
            problems.append(f"{where} publishes the closed vocabulary {pattern!r} as a pattern, not an enum")
    assert not problems, "\n".join(problems)


@business_rule
@scenario("SCN-API-019-B")
@scenario("SCN-API-019-C")
def test_every_published_header_declares_its_schema(server: Sut) -> None:
    """Every declared header carries a schema; a creation declares `Location` and a rejection its challenge."""
    document = openapi(server)
    problems: list[str] = []
    for path, method, operation in iter_operations(document):
        label = f"{method.upper()} {path}"
        for status, response in operation["responses"].items():
            headers = response.get("headers", {})
            for name, header in headers.items():
                if not header.get("schema"):
                    problems.append(f"{label} {status} declares {name!r} without a schema")
            if str(status) == "201" and "Location" not in headers:
                problems.append(f"{label} answers 201 without a declared Location")
            if str(status) == "401" and not headers.get("WWW-Authenticate", {}).get("schema"):
                problems.append(f"{label} answers 401 without a declared WWW-Authenticate")
    assert not problems, "\n".join(problems)


@positive
@scenario("SCN-API-020-A")
@scenario("SCN-API-020-B")
def test_content_bodies_are_binary_and_wildcard(server: Sut) -> None:
    """A content route publishes its body as a binary string and keeps the wildcard media type."""
    document = openapi(server)
    problems: list[str] = []
    for path, method, operation in iter_operations(document):
        if not path.endswith("/content"):
            continue
        for status, response in operation["responses"].items():
            # Only a successful response carries the stored bytes; a rejection
            # carries the error envelope and an empty outcome carries no body.
            if not str(status).startswith("2"):
                continue
            content = response.get("content", {})
            if WILDCARD not in content:
                problems.append(f"{method.upper()} {path} {status} does not keep the wildcard media type")
                continue
            schema = component(document, content[WILDCARD].get("schema", {}))
            if schema.get("type") != "string" or schema.get("format") != "binary":
                problems.append(f"{method.upper()} {path} {status} publishes {schema} instead of a binary string")
    assert not problems, "\n".join(problems)


@positive
@scenario("SCN-API-001-A")
def test_a_business_route_carries_the_base(server: Sut, root_token: str) -> None:
    """A business endpoint answers under `/api/v1` and has no route at the service root."""
    under_base = server.request("GET", f"{API_PREFIX}/library/gallery", token=root_token)
    at_root = server.request("GET", "/library/gallery", token=root_token)

    assert under_base.status_code == 200, f"base route failed: {under_base.status_code} {under_base.text}"
    assert at_root.status_code == 404, f"the route escaped the base: {at_root.status_code} {at_root.text}"


@positive
@scenario("SCN-API-001-B")
@scenario("SCN-API-004-D")
def test_the_probe_and_the_documentation_sit_outside_the_base(server: Sut) -> None:
    """The liveness probe and the documentation answer at the service root, not under the base."""
    for path in ("/health", "/doc", "/doc/openapi.json"):
        response = server.request("GET", path)
        assert response.status_code == 200, f"{path} failed: {response.status_code} {response.text}"

    health = server.request("GET", "/health")
    assert health.json()["_links"]["self"]["href"] == "/health", "the probe reports another operation"

    under_base = server.request("GET", f"{API_PREFIX}/health")
    assert under_base.status_code == 404, f"the probe resolved under the base: {under_base.status_code}"


@error_handling
@scenario("SCN-API-008-A")
def test_an_unknown_route_uses_the_envelope(server: Sut) -> None:
    """A route under the base that names nothing answers a `4xx` HAL envelope with a stable message."""
    path = f"{API_PREFIX}/does-not-exist"
    response = server.request("GET", path)

    assert response.status_code == 404, f"unknown route answered {response.status_code}: {response.text}"
    assert response.headers["Content-Type"].startswith("application/hal+json")
    body = response.json()
    assert set(body) == {"error", "_links"}, "the unknown route did not answer the envelope"
    assert isinstance(body["error"], str) and body["error"]
    assert body["_links"]["self"]["href"] == path


@security
@scenario("SCN-API-009-C")
@pytest.mark.own_instance(reason="process_state")
def test_a_login_burst_answers_the_rate_envelope(sut: Sut) -> None:
    """The login limiter answers `429` with the envelope and `Retry-After`."""
    server = start_instance(sut, config=limiter_config())
    assert attempt(server, "ghost-a").status_code == 401
    throttled = attempt(server, "ghost-b")

    assert throttled.status_code == 429, f"the burst answered {throttled.status_code}"
    assert throttled.headers.get("Retry-After"), "the rate envelope carries no Retry-After"
    body = json.loads(throttled.content)
    assert set(body) == {"error", "_links"}, "the rate rejection did not answer the envelope"
    assert body["_links"]["self"]["href"] == f"{API_PREFIX}/identity/login"


@positive
@scenario("SCN-API-015-A")
def test_the_index_is_public_and_names_itself(server: Sut) -> None:
    """An unauthenticated caller reaches the index, whose self link is the API base."""
    response = get_api_index.sync_detailed(client=server.client())

    assert response.status_code == 200, f"index failed: {response.status_code} {response.content!r}"
    assert plain_href(response.parsed.field_links.self_) == API_PREFIX


@business_rule
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


@business_rule
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


@alternate_flow
@scenario("SCN-API-004-B")
def test_the_current_user_reports_the_canonical_self(server: Sut, root_token: str) -> None:
    """The `/user/me` alias reports the account's canonical user resource."""
    response = get_me.sync_detailed(client=server.client(root_token))

    assert response.status_code == 200, f"current user failed: {response.status_code} {response.content!r}"
    current = response.parsed
    assert plain_href(current.field_links.self_) == f"{server.href('user')}/{current.id}"


@positive
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


@business_rule
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
