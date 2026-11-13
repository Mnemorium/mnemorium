"""Unit cases for the HAL helpers the system suite navigates with.

The helpers are pure: they expand the RFC 6570 subset the API publishes and read
a named link out of a representation. They are proved here, without a container,
before any feature module uses them to follow the served document.
"""

import allure
import pytest
from harness import expand_template, link_of


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-B")
def test_simple_expansion_fills_a_named_variable() -> None:
    """The credential template expands to the operation's addressed path."""
    template = "/api/v1/identity/credential/{id}"

    assert expand_template(template, id=7) == "/api/v1/identity/credential/7"


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-B")
def test_simple_expansion_percent_encodes_the_value() -> None:
    """A value with a reserved character is encoded rather than spliced in."""
    template = "/api/v1/asset/upload/{id}/chunk/{chunk_number}"

    assert expand_template(template, id="a b", chunk_number=0) == "/api/v1/asset/upload/a%20b/chunk/0"


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-B")
def test_query_expansion_builds_the_filter_string() -> None:
    """A query template names the filters and expands them in the declared order."""
    template = "/api/v1/library/movie{?name,limit,offset}"

    assert (
        expand_template(template, name="Arrival", limit=20, offset=0)
        == "/api/v1/library/movie?name=Arrival&limit=20&offset=0"
    )


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-B")
def test_query_expansion_percent_encodes_the_values() -> None:
    """A filter value is percent-encoded in the query expansion."""
    template = "/api/v1/library/movie{?name}"

    assert expand_template(template, name="A/B & C") == "/api/v1/library/movie?name=A%2FB%20%26%20C"


@allure.epic("API")
@allure.feature("REQ-API-015")
@allure.story("SCN-API-015-B")
def test_a_missing_variable_names_the_template_variable() -> None:
    """An unpublished value is a test error, not a silently truncated path."""
    with pytest.raises(AssertionError, match="'id'"):
        expand_template("/api/v1/identity/credential/{id}")


@allure.epic("API")
@allure.feature("REQ-API-004")
@allure.story("SCN-API-004-A")
def test_link_of_returns_the_named_href() -> None:
    """A representation's named link is read without rebuilding the route."""
    representation = {"_links": {"self": {"href": "/api/v1/user/3"}}}

    assert link_of(representation, "self") == "/api/v1/user/3"


@allure.epic("API")
@allure.feature("REQ-API-004")
@allure.story("SCN-API-004-A")
def test_link_of_rejects_a_missing_relation() -> None:
    """A representation that does not publish the relation fails the case by name."""
    with pytest.raises(AssertionError, match="'cover'"):
        link_of({"_links": {"self": {"href": "/api/v1/user/3"}}}, "cover")


@allure.epic("API")
@allure.feature("REQ-API-004")
@allure.story("SCN-API-004-A")
def test_link_of_checks_the_template_flag() -> None:
    """A template is returned as one only when the caller expects a template."""
    representation = {"_links": {"credential": {"href": "/api/v1/identity/credential/{id}", "templated": True}}}

    assert link_of(representation, "credential", templated=True) == "/api/v1/identity/credential/{id}"
    with pytest.raises(AssertionError, match="plain link"):
        link_of(representation, "credential")
