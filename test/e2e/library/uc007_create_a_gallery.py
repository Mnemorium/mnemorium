"""E2E tests for UC-007 - Create a Gallery.

The happy path and both alternative flows (invalid name, existing name) run
against the running container.
"""

import time
from collections.abc import Callable

import pytest
import requests

_GALLERY_PATH = "/api/v1/library/gallery"

# The gallery name business rule caps the name at 100 characters.
_MAX_NAME_LENGTH = 100


def test_create_a_gallery_returns_the_created_gallery(
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    me: Callable[[str], requests.Response],
) -> None:
    name = f"Holidays {time.time_ns()}"

    response = create_gallery(standard_user_token, name)

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert body["name"] == name
    assert isinstance(body["id"], int) and body["id"] > 0
    assert body["item_count"] == 0
    assert body["items"] == []
    caller = me(standard_user_token)
    assert caller.status_code == 200
    assert body["owner_id"] == caller.json()["id"]
    self_href = f"{_GALLERY_PATH}/{body['id']}"
    assert body["_links"]["self"]["href"] == self_href
    assert response.headers.get("Location") == self_href


def test_create_a_gallery_defaults_to_private(
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
) -> None:
    response = create_gallery(standard_user_token, f"Private {time.time_ns()}")

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    assert response.json()["is_public"] is False


def test_create_a_public_gallery_marks_it_public(
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
) -> None:
    response = create_gallery(standard_user_token, f"Public {time.time_ns()}", is_public=True)

    assert response.status_code == 201, f"create failed: {response.status_code} {response.text}"
    assert response.json()["is_public"] is True


@pytest.mark.parametrize("name", ["", "   "])
def test_create_a_gallery_with_an_empty_name_is_rejected(
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
    name: str,
) -> None:
    response = create_gallery(standard_user_token, name)

    assert_error_body(response, 422)


def test_create_a_gallery_with_a_too_long_name_is_rejected(
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    response = create_gallery(standard_user_token, "x" * (_MAX_NAME_LENGTH + 1))

    assert_error_body(response, 422)


def test_create_a_gallery_with_an_existing_name_is_rejected(
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    assert_error_body: Callable[[requests.Response, int], None],
) -> None:
    name = f"Duplicate {time.time_ns()}"
    created = create_gallery(standard_user_token, name)
    assert created.status_code == 201, f"create failed: {created.status_code} {created.text}"

    response = create_gallery(standard_user_token, name)

    assert_error_body(response, 409)


def test_create_a_gallery_with_an_existing_name_is_allowed_for_another_owner(
    create_gallery: Callable[[str, str, bool], requests.Response],
    standard_user_token: str,
    second_standard_user_token: str,
) -> None:
    name = f"Shared {time.time_ns()}"
    first = create_gallery(standard_user_token, name)
    assert first.status_code == 201, f"create failed: {first.status_code} {first.text}"

    second = create_gallery(second_standard_user_token, name)

    assert second.status_code == 201, f"create failed: {second.status_code} {second.text}"
    assert second.json()["name"] == name
