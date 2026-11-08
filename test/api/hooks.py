"""Schemathesis hooks for the fixture API run.

The generator can produce values the served schema does not describe precisely
enough: a header *name* that is not an HTTP token, a date that is not a calendar
date, a username that breaks the lowercase rule, or one half of the paired
season/episode query. The server correctly rejects each one; drop those cases so
the run reports only requests the schema actually models.
"""

import datetime
import re

import schemathesis

_HTTP_TOKEN = re.compile(r"^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$")
_COUNTRY = re.compile(r"^[A-Za-z]{2}$")


def _valid_date(value):
    """Return whether `value` is a calendar date in the API's `YYYY-MM-DD` form."""
    if not isinstance(value, str):
        return True
    try:
        datetime.date.fromisoformat(value)
    except ValueError:
        return False
    return True


def _valid_username(value):
    """Return whether `value` satisfies the username policy."""
    if not isinstance(value, str):
        return True
    return value == value.strip() and not any(character.isupper() for character in value)


def _valid_body(value):
    """Return whether a JSON value carries no value the server must reject."""
    if isinstance(value, dict):
        for key, item in value.items():
            if key.endswith("_date") and not _valid_date(item):
                return False
            if (
                key == "country_of_origin"
                and item is not None
                and (not isinstance(item, str) or not _COUNTRY.fullmatch(item))
            ):
                return False
            if key == "username" and not _valid_username(item):
                return False
            if not _valid_body(item):
                return False
        return True
    if isinstance(value, list):
        return all(_valid_body(item) for item in value)
    return True


def _usable_position(value):
    """Return whether a position value is a non-negative integer."""
    if isinstance(value, bool):
        return False
    if isinstance(value, int):
        return value >= 0
    return isinstance(value, str) and value.isdigit()


def _valid_position(query):
    """Return whether a season/episode pair is either fully absent or fully usable."""
    season = query.get("season")
    episode = query.get("episode")
    if season is None and episode is None:
        return True
    if season is None or episode is None:
        return False
    return _usable_position(season) and _usable_position(episode)


@schemathesis.hook
def filter_case(ctx, case):
    """Drop a case whose generated values are outside the served contract."""
    headers = case.headers or {}
    if any(not _HTTP_TOKEN.fullmatch(str(name)) for name in headers):
        return False
    if not _valid_body(case.body):
        return False
    if case.operation.path == "/library/series/identify/{provider}/{reference}":
        return _valid_position(case.query or {})
    return True
