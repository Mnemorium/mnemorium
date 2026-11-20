"""Pure bodies of the two HAL checks the fixture fuzz run registers.

The bodies take the primitives they inspect — the content type, the status, the
request URL and the decoded body — so they run in this project's venv, which has
no Schemathesis installed. `hooks.py` imports them and registers the thin
wrappers the fuzz run uses.

A check returns the list of problems it found; an empty list is a passing
response. The wrapper turns a non-empty list into a named assertion failure.
"""

from urllib.parse import parse_qs, urlsplit

HAL_CONTENT_TYPE = "application/hal+json"

# The pagination query parameters a collection view carries.
PAGINATION = ("limit", "offset")

# The page size a collection uses when `limit` is absent.
DEFAULT_LIMIT = 20


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
        if relation == "self" and not href.startswith("/"):
            problems.append(f"`_links.self.href` is not a relative target: {href!r}")
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
    self_link = links.get("self")
    self_href = _href_of(self_link)
    if self_href is None:
        return problems
    self_url = urlsplit(self_href)
    if self_url.path != request.path:
        problems.append(f"`_links.self` names {self_url.path!r}, not the request path {request.path!r}")
    self_query = parse_qs(self_url.query, keep_blank_values=True)
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
