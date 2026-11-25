"""Schemathesis hooks and checks for the fixture API run.

One carve-out remains: the *header name* the generator draws is a valid JSON
string but not always a valid HTTP token, and `requests` (the transport
Schemathesis uses) refuses to send such a header at all. That is a transport
limitation, not a server behaviour, so the cases carrying it are dropped.

The link checks are thin wrappers over the pure bodies in `checks.py`, so the
fuzz configuration never restates their rules. Three probes reach what
Schemathesis' own generation cannot: the request-body contract on every
operation that takes a JSON body, the closed query set on every operation, the
bearer requirement on every protected operation, and the no-op of a hypermedia
member in a request. The link and response checks wrap the same pure bodies.
"""

import re
import sys
from pathlib import Path

# Schemathesis loads this file by path, so the sibling pure module is not on
# `sys.path` yet.
sys.path.insert(0, str(Path(__file__).resolve().parent))

import schemathesis
from checks import (
    HAL_CONTENT_TYPE,
    valid_canonical_self,
    valid_collection_links,
    valid_declared_headers,
    valid_embedded_payloads,
    valid_hal_links,
    valid_success_responses,
)

_HTTP_TOKEN = re.compile(r"^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$")
_JSON_MEDIA_TYPE = "application/json"
# An undeclared query parameter and a hypermedia member a probe adds to a generated request.
_UNDECLARED_QUERY = "mnemorium_undeclared"
_HYPERMEDIA_MEMBERS = {"_links": {"self": {"href": "/probe"}}, "_embedded": {"items": []}}
# Statuses that read a hypermedia member as a request-shape failure instead of ignoring it.
_HYPERMEDIA_REFUSALS = frozenset({400, 415, 422, 428})
# Operations whose request-body contract has already been probed in this process.
_PROBED_OPERATIONS: set[str] = set()
# Operations whose undeclared-query, protection or hypermedia contract has been probed.
_PROBED_QUERIES: set[str] = set()
_PROBED_PROTECTION: set[str] = set()
_PROBED_HYPERMEDIA: set[str] = set()


@schemathesis.hook
def filter_case(ctx, case):
    """Drop a case whose generated header name is not an HTTP token.

    `requests` rejects a header name outside the token grammar before it reaches
    the server, so the case proves nothing about the served document.
    """
    headers = case.headers or {}
    return all(_HTTP_TOKEN.fullmatch(str(name)) for name in headers)


def _content_type(response) -> str | None:
    """Return the response's content type from the lowercase, list-valued header."""
    values = response.headers.get("content-type")
    return values[0] if values else None


def _hal_body(response, content_type: str | None):
    """Return the decoded body of a HAL response, or `None` for any other type."""
    if not content_type or content_type.split(";", 1)[0].strip().lower() != HAL_CONTENT_TYPE:
        return None
    try:
        return response.json()
    except ValueError:
        return None


@schemathesis.check
def valid_hal_links_check(ctx, response, case) -> None:
    """A HAL representation carries `_links.self`, and every link is non-empty (`REQ-API-003`, `004`, `008`)."""
    content_type = _content_type(response)
    problems = valid_hal_links(
        content_type=content_type,
        status_code=response.status_code,
        request_url=str(response.request.url),
        body=_hal_body(response, content_type),
    )
    if problems:
        raise AssertionError("\n".join(problems))


@schemathesis.check
def valid_collection_links_check(ctx, response, case) -> None:
    """A paginated collection's links preserve the view (`REQ-API-007`)."""
    content_type = _content_type(response)
    problems = valid_collection_links(
        content_type=content_type,
        request_url=str(response.request.url),
        body=_hal_body(response, content_type),
    )
    if problems:
        raise AssertionError("\n".join(problems))


@schemathesis.check
def valid_embedded_payloads_check(ctx, response, case) -> None:
    """A catalog page embeds partial items and no other collection embeds (`REQ-API-005-C`, `016`)."""
    if str(case.method).upper() != "GET":
        return
    content_type = _content_type(response)
    problems = valid_embedded_payloads(
        content_type=content_type,
        status_code=response.status_code,
        request_url=str(response.request.url),
        body=_hal_body(response, content_type),
    )
    if problems:
        raise AssertionError("\n".join(problems))


@schemathesis.check
def valid_success_responses_check(ctx, response, case) -> None:
    """A creation names its resource in `Location`; a deletion carries no body (`REQ-API-012`)."""
    problems = valid_success_responses(
        status_code=response.status_code,
        headers=response.headers,
        body=response.content or None,
    )
    if problems:
        raise AssertionError("\n".join(problems))


@schemathesis.check
def valid_declared_headers_check(ctx, response, case) -> None:
    """Every emitted application header is declared on its status (`REQ-API-009-B`, `019`)."""
    declared = _declared_headers(case.operation, response.status_code)
    if declared is None:
        return
    problems = valid_declared_headers(
        status_code=response.status_code,
        headers=response.headers,
        declared=declared,
    )
    if problems:
        raise AssertionError("\n".join(problems))


def _declared_headers(operation, status_code: int) -> list[str] | None:
    """The header names the document declares on the operation's response for a status."""
    response = operation.responses.find_by_status_code(status_code)
    if response is None:
        return None
    return [header.name for _, header in response.headers.items()]


@schemathesis.check
def canonical_self_check(ctx, response, case) -> None:
    """A read that addresses a resource directly names it in `_links.self` (`REQ-API-004`)."""
    if str(case.method).upper() != "GET" or not _addresses_a_resource(case.operation):
        return
    content_type = _content_type(response)
    problems = valid_canonical_self(
        content_type=content_type,
        status_code=response.status_code,
        request_url=str(response.request.url),
        body=_hal_body(response, content_type),
    )
    if problems:
        raise AssertionError("\n".join(problems))


def _addresses_a_resource(operation) -> bool:
    """Whether the operation reads a stored resource by its numeric identifier.

    Such a read ends in the identifier's path parameter, which is an integer;
    a provider reference is a string, and a path ending in a literal segment is
    a collection or an alias rather than a direct read.
    """
    last = operation.path.rsplit("/", 1)[-1]
    if not (last.startswith("{") and last.endswith("}")):
        return False
    name = last[1:-1]
    for parameter in operation.iter_parameters():
        if parameter.definition.get("in") == "path" and parameter.name == name:
            return parameter.definition.get("schema", {}).get("type") == "integer"
    return False


@schemathesis.check
def request_body_contract_check(ctx, response, case) -> None:
    """Probe the request-body contract on every operation that takes a JSON body (`REQ-API-002`).

    The check runs once per operation, from the first authenticated case that
    carries a JSON body. Each probe reuses that case's credentials and asserts
    one refusal the fuzz run cannot generate for itself:

    - a JSON body sent as `text/plain` answers `415`;
    - an unparseable body answers `400`;
    - a body missing a required member answers `422`.
    """
    operation = case.operation
    if not _is_declared_method(case):
        return
    if case.media_type != _JSON_MEDIA_TYPE or operation.label in _PROBED_OPERATIONS:
        return
    if not isinstance(case.body, (dict, list)):
        return
    # The first case an operation gets carries no credentials; it checks whether
    # the operation is guarded, so it cannot speak about the body contract.
    if _declares_security(operation) and response.status_code in (401, 403):
        return
    _PROBED_OPERATIONS.add(operation.label)

    wrong_media_type = case.derive(keep_auth=True)
    wrong_media_type.headers["Content-Type"] = "text/plain"
    _expect_refusal(operation, "a non-JSON content type", 415, wrong_media_type.call())

    unparseable = case.derive(keep_auth=True)
    _expect_refusal(operation, "an unparseable body", 400, unparseable.call(data='{"mnemorium": '))

    missing_member = _body_missing_a_required_member(operation, case.body)
    if missing_member is not None:
        wrong_shape = case.derive(keep_auth=True)
        wrong_shape.body = missing_member
        _expect_refusal(operation, "a body missing a required member", 422, wrong_shape.call())


def _body_missing_a_required_member(operation, body) -> dict | None:
    """A copy of `body` without one declared required member, or `None` when the schema requires none."""
    if not isinstance(body, dict):
        return None
    for name in _required_properties(operation):
        if name in body:
            return {key: value for key, value in body.items() if key != name}
    return None


def _required_properties(operation) -> list[str]:
    """The required property names of the operation's JSON body schema."""
    for payload in operation.body:
        definition = payload.definition if isinstance(payload.definition, dict) else {}
        schema = definition.get("schema")
        if not isinstance(schema, dict):
            continue
        bundled = schema.get("x-bundled")
        reference = schema.get("$ref")
        if isinstance(reference, str) and isinstance(bundled, dict):
            schema = bundled.get(reference.rsplit("/", 1)[-1], {})
        required = schema.get("required") if isinstance(schema, dict) else None
        if isinstance(required, list):
            return [name for name in required if isinstance(name, str)]
    return []


def _is_declared_method(case) -> bool:
    """Whether the case uses the method the operation declares.

    Schemathesis also sends an unsupported method to probe the routing layer;
    such a case answers `405` and says nothing about the operation's contract.
    """
    return str(case.method).upper() == str(case.operation.method).upper()


def _declares_security(operation) -> bool:
    """Whether the operation declares a security parameter at all."""
    parameters = getattr(operation.security, "iter_parameters", None)
    return parameters is not None and any(True for _ in parameters())


def _expect_refusal(operation, probe: str, status: int, response) -> None:
    """Raise when a request-body probe did not receive its declared refusal."""
    if response.status_code == status:
        return
    content = getattr(response, "content", b"")[:200]
    raise AssertionError(f"{operation.label}: {probe} answered {response.status_code}, expected {status}: {content!r}")


@schemathesis.check
def undeclared_query_check(ctx, response, case) -> None:
    """Probe the closed query set once per operation (`REQ-API-022`).

    Schemathesis only generates the parameters the document declares, so a
    query parameter the operation does not declare is never sent. The probe
    adds one to an otherwise generated request and expects `400`.
    """
    operation = case.operation
    if not _is_declared_method(case) or response.status_code == 405:
        return
    if operation.label in _PROBED_QUERIES:
        return
    # The probe speaks only about the query set: the case must have been
    # accepted on its business data, or its refusal masks the added parameter.
    if not 200 <= response.status_code < 400:
        return
    _PROBED_QUERIES.add(operation.label)
    probe = case.derive(keep_auth=True)
    query = dict(probe.query)
    if _UNDECLARED_QUERY in query:
        return
    query[_UNDECLARED_QUERY] = "probe"
    probe.query = query
    _expect_refusal(operation, "an undeclared query parameter", 400, probe.call())


@schemathesis.check
def protected_operation_check(ctx, response, case) -> None:
    """Probe the bearer requirement once per protected operation (`REQ-API-023`).

    The probe derives a request with no credentials and expects the `401`
    envelope with its challenge.
    """
    operation = case.operation
    if not _is_declared_method(case):
        return
    if operation.label in _PROBED_PROTECTION or not _declares_bearer_auth(operation):
        return
    _PROBED_PROTECTION.add(operation.label)
    probe = case.derive(keep_auth=False)
    probe.headers.pop("Authorization", None)
    result = probe.call()
    if result.status_code != 401:
        content = getattr(result, "content", b"")[:200]
        raise AssertionError(
            f"{operation.label}: an unauthenticated request answered {result.status_code}, expected 401: {content!r}"
        )
    challenge = _header(result.headers, "www-authenticate") or ""
    if not challenge.lower().startswith("bearer"):
        raise AssertionError(f"{operation.label}: the 401 response carries no `WWW-Authenticate: Bearer` challenge")


@schemathesis.check
def hypermedia_body_check(ctx, response, case) -> None:
    """Probe that a hypermedia member in a request changes nothing (`REQ-API-002-D`).

    From a case whose business data the server accepted, the probe replays it
    with `_links` and `_embedded` added. The request is decided on its business
    data, so the extra members never turn it into a shape refusal.
    """
    operation = case.operation
    if not _is_declared_method(case):
        return
    if operation.label in _PROBED_HYPERMEDIA:
        return
    if case.media_type != _JSON_MEDIA_TYPE or not isinstance(case.body, dict):
        return
    if not 200 <= response.status_code < 300:
        return
    _PROBED_HYPERMEDIA.add(operation.label)
    probe = case.derive(keep_auth=True)
    probe.body = {**case.body, **_HYPERMEDIA_MEMBERS}
    result = probe.call()
    if result.status_code in _HYPERMEDIA_REFUSALS:
        content = getattr(result, "content", b"")[:200]
        raise AssertionError(
            f"{operation.label}: a request carrying `_links`/`_embedded` answered {result.status_code}, "
            f"which reads them as a shape failure: {content!r}"
        )


def _declares_bearer_auth(operation) -> bool:
    """Whether the operation's own declaration requires the bearer scheme."""
    raw = getattr(operation.definition, "raw", {}) or {}
    security = raw.get("security")
    if not isinstance(security, list):
        return False
    return any(isinstance(requirement, dict) and "bearer_auth" in requirement for requirement in security)


def _header(headers, name: str) -> str | None:
    """Return one response header's value, joining a list-valued header."""
    value = headers.get(name)
    if isinstance(value, (list, tuple)):
        return ", ".join(str(item) for item in value)
    return str(value) if value is not None else None
