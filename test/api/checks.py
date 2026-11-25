"""Pure bodies of the HAL checks the fixture fuzz run registers.

The bodies take the primitives they inspect — the content type, the status, the
request URL and the decoded body — so they run in this project's venv, which has
no Schemathesis installed. `hooks.py` imports them and registers the thin
wrappers the fuzz run uses.

A check returns the list of problems it found; an empty list is a passing
response. The wrapper turns a non-empty list into a named assertion failure.
"""

from urllib.parse import parse_qs, unquote, urlsplit

HAL_CONTENT_TYPE = "application/hal+json"

# The pagination query parameters a collection view carries.
PAGINATION = ("limit", "offset")

# The page size a collection uses when `limit` is absent.
DEFAULT_LIMIT = 20

# The collection routes whose page items are embedded partial representations
# (REQ-API-016). Their paths are relative to the `/api/v1` base the document
# declares.
CATALOG_COLLECTIONS = (
    "/library/movie",
    "/library/series",
    "/library/music/album",
    "/library/music/group",
)

# The members an embedded catalog item carries: the browse fields plus its links.
EMBEDDED_ITEM_MEMBERS = frozenset({"id", "name", "cover_image_id", "_links"})

# Headers the serving stack emits for a response but the document does not
# declare per status: they describe the HTTP message, not the operation.
STANDARD_HEADERS = frozenset({"content-type", "content-length", "date", "connection", "transfer-encoding", "server"})


def _media_type(content_type: str | None) -> str:
    """Return the media type without its parameters, lowercased."""
    if not content_type:
        return ""
    return content_type.split(";", 1)[0].strip().lower()


def _links_of(body: object) -> dict | None:
    """Return the `_links` object of a representation, when it carries one."""
    if not isinstance(body, dict):
        return None
    links = body.get("_links")
    return links if isinstance(links, dict) else None


def _href_of(link: object) -> str | None:
    """Return a link object's non-empty `href`, when it carries one."""
    if not isinstance(link, dict):
        return None
    href = link.get("href")
    return href if isinstance(href, str) and href else None


def valid_hal_links(
    *,
    content_type: str | None,
    status_code: int,
    request_url: str,
    body: object,
) -> list[str]:
    """Return the problems in a HAL representation's links (`REQ-API-003`, `004`, `008`).

    A HAL representation carries `_links.self`, every link is a non-empty
    relative target, and a `4xx` envelope's `self` is the request path with the
    query ignored. A response served with another media type is not a HAL
    representation and is not inspected.
    """
    if _media_type(content_type) != HAL_CONTENT_TYPE:
        return []
    if not isinstance(body, dict):
        return ["the HAL body is not a JSON object"]
    problems: list[str] = []
    links = _links_of(body)
    if links is None:
        return ["a HAL representation must carry `_links` as an object"]
    if "self" not in links:
        problems.append("`_links.self` is missing")
    for relation, link in links.items():
        if not isinstance(link, dict):
            problems.append(f"`_links.{relation}` is not a link object")
            continue
        href = _href_of(link)
        if href is None:
            problems.append(f"`_links.{relation}.href` is empty")
            continue
        templated = link.get("templated") is True
        if templated:
            if "{" not in href or "}" not in href:
                problems.append(f"`_links.{relation}` is templated but its href carries no expression: {href!r}")
        else:
            if "{" in href or "}" in href:
                problems.append(f"`_links.{relation}` carries a URI template without `templated: true`: {href!r}")
            elif not href.startswith("/"):
                problems.append(f"`_links.{relation}.href` is not a relative target: {href!r}")
    if 400 <= status_code < 500:
        request_path = urlsplit(request_url).path
        self_href = _href_of(links.get("self"))
        if self_href != request_path:
            problems.append(
                f"the error envelope's `_links.self` is {self_href!r}, not the request path {request_path!r}"
            )
    return problems


def valid_collection_links(
    *,
    content_type: str | None,
    request_url: str,
    body: object,
) -> list[str]:
    """Return the problems in a paginated collection's links (`REQ-API-007`).

    A collection is classified by its `search` link, never by a field name. The
    `search` link is templated and names the collection; `self` preserves the
    request filters; `next` preserves them and advances by the page size; and
    `prev` never appears at offset `0`. The query-preservation rule applies only
    when `self` carries `limit`/`offset`, so an unfiltered collection such as
    `list_users` is outside it.
    """
    if _media_type(content_type) != HAL_CONTENT_TYPE:
        return []
    links = _links_of(body)
    if links is None or "search" not in links:
        return []
    request = urlsplit(request_url)
    request_query = parse_qs(request.query, keep_blank_values=True)
    problems: list[str] = []

    search = links["search"]
    if not isinstance(search, dict) or search.get("templated") is not True:
        problems.append("`_links.search` is not templated")
        declared: set[str] = set()
    else:
        template = _href_of(search)
        if template is None:
            problems.append("`_links.search.href` is empty")
            declared = set()
        else:
            if template.split("{", 1)[0] != request.path:
                problems.append(f"`_links.search` names {template!r}, not the collection {request.path!r}")
            declared = _template_filters(template)

    active = _active_filters(request_query, declared)
    for name in sorted(declared):
        if name in body:
            problems.append(f"the filter {name!r} is repeated as a payload field")
    self_link = links.get("self")
    self_href = _href_of(self_link)
    if self_href is None:
        return problems
    self_url = urlsplit(self_href)
    if self_url.path != request.path:
        problems.append(f"`_links.self` names {self_url.path!r}, not the request path {request.path!r}")
    self_query = parse_qs(self_url.query, keep_blank_values=True)
    items = _page_items(body)
    if "limit" not in self_query and len(items) > DEFAULT_LIMIT:
        problems.append(f"a page without `limit` carries {len(items)} items, more than the default {DEFAULT_LIMIT}")
    paginated = any(name in self_query for name in PAGINATION)
    if not paginated:
        return problems
    for name, values in active.items():
        if self_query.get(name) != values:
            problems.append(f"`_links.self` drops the {name!r} filter")

    limit = _page_value(self_query, "limit", DEFAULT_LIMIT)
    offset = _page_value(self_query, "offset", 0)

    if "next" in links:
        problems.extend(
            _navigation_problems(
                links["next"],
                relation="next",
                request=request,
                filters=active,
                limit=limit,
                offset=offset + limit,
            )
        )
    if "prev" in links:
        if offset == 0:
            problems.append("`_links.prev` appears at offset 0")
        else:
            problems.extend(
                _navigation_problems(
                    links["prev"],
                    relation="prev",
                    request=request,
                    filters=active,
                    limit=limit,
                    offset=max(offset - limit, 0),
                )
            )
    return problems


def valid_canonical_self(
    *,
    content_type: str | None,
    status_code: int,
    request_url: str,
    body: object,
) -> list[str]:
    """Return the problems in a resource read's `_links.self` (`REQ-API-004`, `SCN-API-004-A`).

    A successful read whose path ends in the numeric identifier it addresses
    reads that resource directly, so `self` is the request path without a
    query: that path is the resource's canonical URI. An alias read ends in a
    literal segment and reports the resource behind the alias instead
    (`SCN-API-004-B`), so the wrapper does not submit it; a failure is
    `SCN-API-004-E`, and a response served with another media type is not a
    representation.
    """
    if not 200 <= status_code < 300:
        return []
    if _media_type(content_type) != HAL_CONTENT_TYPE:
        return []
    links = _links_of(body)
    if links is None:
        return []
    self_href = _href_of(links.get("self"))
    if self_href is None:
        return []
    request = urlsplit(request_url)
    target = urlsplit(self_href)
    # Both paths are compared decoded: the server percent-encodes a path
    # parameter one way and the transport another.
    if unquote(target.path) != unquote(request.path):
        return [f"`_links.self` names {target.path!r}, not the addressed resource {request.path!r}"]
    if target.query:
        return [f"`_links.self` carries the query {target.query!r}; a canonical resource URI carries none"]
    return []


def _page_items(body: object) -> list:
    """Return a collection page's items, whether at the root or embedded under `_embedded`."""
    if not isinstance(body, dict):
        return []
    items = body.get("items")
    if items is None:
        embedded = body.get("_embedded")
        items = embedded.get("items") if isinstance(embedded, dict) else None
    return items if isinstance(items, list) else []


def valid_embedded_payloads(
    *,
    content_type: str | None,
    status_code: int,
    request_url: str,
    body: object,
) -> list[str]:
    """Return the problems in a catalog collection's embedded items (`REQ-API-005-C`, `016`).

    Only the four catalog collections embed their page items, under
    `_embedded.items`; every other collection keeps them at the root under
    `items`. An embedded item is the partial representation: its identifier,
    name, cover image identifier and links, with no detail-only member.
    """
    if not 200 <= status_code < 300 or _media_type(content_type) != HAL_CONTENT_TYPE:
        return []
    if not isinstance(body, dict):
        return ["the HAL body is not a JSON object"]
    path = urlsplit(request_url).path.rstrip("/")
    catalog = any(path.endswith(f"{collection}") for collection in CATALOG_COLLECTIONS)
    embedded = body.get("_embedded")
    if not catalog:
        if embedded is not None:
            return ["`_embedded` appears outside the four catalog collections"]
        return []
    if not isinstance(embedded, dict):
        return ["a catalog collection must carry `_embedded.items`"]
    problems: list[str] = []
    if "items" in body:
        problems.append("the catalog collection repeats its items at the root")
    items = embedded.get("items")
    if not isinstance(items, list):
        return [*problems, "`_embedded.items` is missing"]
    for index, item in enumerate(items):
        if not isinstance(item, dict):
            problems.append(f"`_embedded.items[{index}]` is not an object")
            continue
        for member in sorted(EMBEDDED_ITEM_MEMBERS - {"_links", "_embedded"} - item.keys()):
            problems.append(f"`_embedded.items[{index}]` is missing the browse member {member!r}")
        detail_only = sorted(item.keys() - EMBEDDED_ITEM_MEMBERS)
        for member in detail_only:
            problems.append(f"`_embedded.items[{index}]` carries the detail-only member {member!r}")
        links = item.get("_links")
        if not isinstance(links, dict):
            problems.append(f"`_embedded.items[{index}]._links` is not an object")
            continue
        if "self" not in links:
            problems.append(f"`_embedded.items[{index}]` carries no `_links.self`")
        if item.get("cover_image_id") is not None and "cover" not in links:
            problems.append(f"`_embedded.items[{index}]` carries a cover image but no `_links.cover`")
    return problems


def valid_success_responses(
    *,
    status_code: int,
    headers: object,
    body: object,
) -> list[str]:
    """Return the problems in a successful response's outcome (`REQ-API-012`).

    A creation answers `201` with `Location` naming the resource, and a deletion
    answers `204` with no body.
    """
    problems: list[str] = []
    if status_code == 201 and not _header_value(headers, "location"):
        problems.append("a 201 response declares no `Location` naming the created resource")
    if status_code == 204 and body:
        problems.append("a 204 response carries a body")
    return problems


def valid_declared_headers(
    *,
    status_code: int,
    headers: object,
    declared: object,
) -> list[str]:
    """Return the problems in a response's emitted headers (`REQ-API-009-B`, `019`).

    Every application header the response emits is declared on that status in
    the published document. The message headers the serving stack adds are not
    the operation's, so they are outside the rule. A `401` always carries the
    bearer challenge.
    """
    declared_names = {str(name).lower() for name in declared}
    problems: list[str] = []
    for name in _header_names(headers):
        lowered = name.lower()
        if lowered in declared_names or lowered in STANDARD_HEADERS:
            continue
        problems.append(f"the response header {name!r} is not declared on status {status_code}")
    if status_code == 401:
        challenge = _header_value(headers, "www-authenticate") or ""
        if not challenge.strip().lower().startswith("bearer"):
            problems.append("a 401 response carries no `WWW-Authenticate: Bearer` challenge")
    if status_code == 405 and not _header_value(headers, "allow"):
        problems.append("a 405 response carries no `Allow` header")
    if status_code == 429 and not _header_value(headers, "retry-after"):
        problems.append("a 429 response carries no `Retry-After` header")
    return problems


def _header_names(headers: object) -> list[str]:
    """Return the names an emitted header mapping carries."""
    if isinstance(headers, dict):
        return [str(name) for name in headers]
    names = getattr(headers, "keys", None)
    return [str(name) for name in names()] if names is not None else []


def _header_value(headers: object, name: str) -> str | None:
    """Return one emitted header's value, joining a list-valued header."""
    if not isinstance(headers, dict):
        return None
    for key, value in headers.items():
        if str(key).lower() != name:
            continue
        if isinstance(value, (list, tuple)):
            return ", ".join(str(item) for item in value)
        return str(value)
    return None


def _template_filters(template: str) -> set[str]:
    """Return the query-variable names one URI template declares."""
    if "{" not in template:
        return set()
    expression = template.split("{", 1)[1].split("}", 1)[0]
    if not expression.startswith(("?", "&")):
        return set()
    return {name.split(":", 1)[0].rstrip("*") for name in expression[1:].split(",") if name}


def _active_filters(query: dict[str, list[str]], declared: set[str]) -> dict[str, list[str]]:
    """Return the request filters the collection declares and the client supplied.

    A query parameter the collection does not declare is ignored by the server,
    and an empty value names no filter, so neither is preserved by a link.
    """
    return {
        name: values
        for name, values in query.items()
        if name in declared and name not in PAGINATION and any(value.strip() for value in values)
    }


def _page_value(query: dict[str, list[str]], name: str, default: int) -> int:
    """Read one pagination value, falling back to `default` when it is absent."""
    values = query.get(name)
    if not values:
        return default
    try:
        return int(values[0])
    except ValueError:
        return default


def _navigation_problems(
    link: object,
    *,
    relation: str,
    request,
    filters: dict[str, list[str]],
    limit: int,
    offset: int,
) -> list[str]:
    """Return the problems in a `next` or `prev` link's target."""
    problems: list[str] = []
    href = _href_of(link)
    if href is None:
        return [f"`_links.{relation}.href` is empty"]
    target = urlsplit(href)
    if target.path != request.path:
        problems.append(f"`_links.{relation}` names {target.path!r}, not the collection {request.path!r}")
        return problems
    query = parse_qs(target.query, keep_blank_values=True)
    for name, values in filters.items():
        if query.get(name) != values:
            problems.append(f"`_links.{relation}` drops the {name!r} filter")
    if _page_value(query, "limit", DEFAULT_LIMIT) != limit:
        problems.append(f"`_links.{relation}` changes the page size")
    if _page_value(query, "offset", 0) != offset:
        problems.append(f"`_links.{relation}` does not address offset {offset}, only {_page_value(query, 'offset', 0)}")
    return problems
