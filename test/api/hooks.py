"""Schemathesis hooks and checks for the fixture API run.

One carve-out remains: the *header name* the generator draws is a valid JSON
string but not always a valid HTTP token, and `requests` (the transport
Schemathesis uses) refuses to send such a header at all. That is a transport
limitation, not a server behaviour, so the cases carrying it are dropped.

The two custom checks are thin wrappers over the pure bodies in `checks.py`, so
`pytest test/api` runs them without Schemathesis installed.
"""

import re
import sys
from pathlib import Path

# Schemathesis loads this file by path, so the sibling pure module is not on
# `sys.path` yet.
sys.path.insert(0, str(Path(__file__).resolve().parent))

import schemathesis
from checks import HAL_CONTENT_TYPE, valid_collection_links, valid_hal_links

_HTTP_TOKEN = re.compile(r"^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$")


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
