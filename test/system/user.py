"""Black-box proof of the `user` capability.

Every case starts its own container from the image under test through the shared `harness` plugin and reads only what an
operator can: HTTP responses and the working directory. The caller is provisioned in setup — the Root Admin signs in
with the password the instance prints — but a case asserts the account outcome, not the gate that admitted the caller.
"""

import sqlite3

import allure
from conftest import (
    FIXTURE_DIRECTORY,
    add_to_gallery,
    create_gallery,
    file_profile,
    linked_id,
    provision,
    upload,
)
from harness import (
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    Sut,
    begin_upload,
    link_of,
    register,
)


@allure.epic("User")
@allure.feature("REQ-USER-004")
@allure.story("SCN-USER-004-G")
def test_email_case_does_not_distinguish_two_addresses(server: Sut, root_token: str) -> None:
    """A case variant of a stored address answers 409 and creates no second account."""
    alice = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert alice.status_code == 201, f"creation failed: {alice.status_code} {alice.text}"

    response = register(server, root_token, "bobby", "Alice@example.com", SECRET_PASSWORD)

    assert response.status_code == 409, f"case variant accepted: {response.status_code} {response.text}"
    assert response.json()["error"] == "email is already taken"

    listed = server.request("GET", f"{server.href('user')}?username=bobby", token=root_token)
    assert listed.status_code == 200, f"filter failed: {listed.status_code} {listed.text}"
    assert listed.json()["items"] == []


@allure.epic("User")
@allure.feature("REQ-USER-010")
@allure.story("SCN-USER-010-E")
def test_the_email_filter_ignores_case(server: Sut, root_token: str) -> None:
    """An email filter that differs from the stored address only by case finds the account."""
    carol = register(server, root_token, "carol", "carol@example.com", SECRET_PASSWORD)
    assert carol.status_code == 201, f"creation failed: {carol.status_code} {carol.text}"

    response = server.request(
        "GET",
        f"{server.href('user')}?email=Carol@Example.com",
        token=root_token,
    )

    assert response.status_code == 200, f"filter failed: {response.status_code} {response.text}"
    assert [item["username"] for item in response.json()["items"]] == ["carol"]


@allure.epic("User")
@allure.feature("REQ-USER-013")
@allure.story("SCN-USER-013-D")
def test_a_case_only_email_change_collides(server: Sut, root_token: str) -> None:
    """An update carrying another account's address in a different case answers 409 and changes nothing."""
    david = register(server, root_token, "david", "david@example.com", SECRET_PASSWORD)
    assert david.status_code == 201, f"creation failed: {david.status_code} {david.text}"
    erin = register(server, root_token, "erin", "erin@example.com", SECRET_PASSWORD)
    assert erin.status_code == 201, f"creation failed: {erin.status_code} {erin.text}"
    erin_id = erin.json()["id"]

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{erin_id}",
        token=root_token,
        json={"email": "David@Example.com"},
    )

    assert response.status_code == 409, f"case variant accepted: {response.status_code} {response.text}"
    stored = server.request("GET", f"{server.href('user')}/{erin_id}", token=root_token)
    assert stored.json()["email"] == "erin@example.com"


@allure.epic("User")
@allure.feature("REQ-USER-004")
def test_the_parser_governs_email_syntax(server: Sut, root_token: str) -> None:
    """The address parser admits what it accepts and answers 422 for what it rejects, with no constraint failure."""
    accepted = (
        "admin@mailserver1",
        '" "@example.org',
        '"john..doe"@example.org',
        "user%example.com@example.org",
    )

    for index, email in enumerate(accepted):
        response = register(server, root_token, f"parser{index}", email, SECRET_PASSWORD)
        assert response.status_code == 201, f"{email!r} was rejected: {response.status_code} {response.text}"

    rejected = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={
            "username": "parserrejected",
            "email": "Alice <alice@example.com>",
            "password": SECRET_PASSWORD,
            "role": "STANDARD",
        },
    )

    assert rejected.status_code == 422, f"display name accepted: {rejected.status_code} {rejected.text}"
    listed = server.request("GET", f"{server.href('user')}?username=parserrejected", token=root_token)
    assert listed.json()["items"] == []


@allure.epic("User")
@allure.feature("REQ-USER-001")
@allure.story("SCN-USER-001-A")
@allure.feature("REQ-USER-005")
@allure.story("SCN-USER-005-A")
def test_the_profile_carries_the_account_detail(server: Sut, root_token: str) -> None:
    """An account reads back with its identifier, username, email or no email, and role."""
    with_email = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert with_email.status_code == 201, f"creation failed: {with_email.status_code} {with_email.text}"
    without_email = register(server, root_token, "bobby", None, SECRET_PASSWORD)
    assert without_email.status_code == 201, f"creation failed: {without_email.status_code} {without_email.text}"

    for created, email in ((with_email, "alice@example.com"), (without_email, None)):
        profile = created.json()
        response = server.request("GET", f"{server.href('user')}/{profile['id']}", token=root_token)

        assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
        body = response.json()
        assert isinstance(body["id"], int) and body["id"] > 0
        assert body["username"] == profile["username"]
        assert body["email"] == email
        assert body["role"] == "STANDARD"


@allure.epic("User")
@allure.feature("REQ-USER-002")
@allure.story("SCN-USER-002-A")
def test_a_declared_role_is_stored(server: Sut, root_token: str) -> None:
    """A creation carrying ADMIN or STANDARD stores and returns that role."""
    for role in ("ADMIN", "STANDARD"):
        created = register(server, root_token, f"role{role.lower()}", None, SECRET_PASSWORD, role=role)
        assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"

        body = created.json()
        stored = server.request("GET", f"{server.href('user')}/{body['id']}", token=root_token)

        assert body["role"] == role
        assert stored.json()["role"] == role


@allure.epic("User")
@allure.feature("REQ-USER-002")
@allure.story("SCN-USER-002-B")
def test_an_undeclared_role_is_rejected(server: Sut, root_token: str) -> None:
    """A role outside the declared values answers 422 and stores nothing."""
    response = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={
            "username": "superuser",
            "email": None,
            "password": SECRET_PASSWORD,
            "role": "SUPERUSER",
        },
    )

    assert response.status_code == 422, f"undeclared role accepted: {response.status_code} {response.text}"
    listed = server.request("GET", f"{server.href('user')}?username=superuser", token=root_token)
    assert listed.json()["items"] == []


@allure.epic("User")
@allure.feature("REQ-USER-009")
@allure.story("SCN-USER-009-A")
def test_an_account_is_readable_by_identifier(server: Sut, root_token: str) -> None:
    """A request addressing an existing account's identifier answers 200 with its profile."""
    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    profile = created.json()

    response = server.request("GET", f"{server.href('user')}/{profile['id']}", token=root_token)

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    assert response.json() == profile


@allure.epic("User")
@allure.feature("REQ-USER-009")
@allure.story("SCN-USER-009-B")
def test_an_unknown_identifier_is_not_found(server: Sut, root_token: str) -> None:
    """An identifier that names no account answers 404."""
    response = server.request("GET", f"{server.href('user')}/999999", token=root_token)

    assert response.status_code == 404, f"unknown account answered {response.status_code}"


@allure.epic("User")
@allure.feature("REQ-USER-010")
@allure.story("SCN-USER-010-A")
def test_no_filter_returns_every_account(server: Sut, root_token: str) -> None:
    """The collection requested without a filter carries every account."""
    for username in ("alice", "bobby", "carol"):
        response = register(server, root_token, username, None, SECRET_PASSWORD)
        assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"

    response = server.request("GET", server.href("user"), token=root_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert {item["username"] for item in response.json()["items"]} == {
        ROOT_ADMIN_USERNAME,
        "alice",
        "bobby",
        "carol",
    }


@allure.epic("User")
@allure.feature("REQ-USER-010")
@allure.story("SCN-USER-010-B")
def test_filters_narrow_the_result(server: Sut, root_token: str) -> None:
    """Only the accounts matching every supplied filter appear."""
    registrations = (
        ("alice", "alice@example.com", "STANDARD"),
        ("bobby", "bobby@example.com", "ADMIN"),
        ("carol", "carol@example.org", "STANDARD"),
    )
    for username, email, role in registrations:
        response = register(server, root_token, username, email, SECRET_PASSWORD, role=role)
        assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"

    queries = (
        ("?username=alice", {"alice"}),
        ("?email=bobby@example.com", {"bobby"}),
        ("?role=ADMIN", {ROOT_ADMIN_USERNAME, "bobby"}),
        ("?role=STANDARD&email=carol@example.org", {"carol"}),
    )

    for query, expected in queries:
        response = server.request("GET", f"{server.href('user')}{query}", token=root_token)

        assert response.status_code == 200, f"{query} failed: {response.status_code} {response.text}"
        assert {item["username"] for item in response.json()["items"]} == expected


@allure.epic("User")
@allure.feature("REQ-USER-010")
@allure.story("SCN-USER-010-C")
def test_an_empty_result_carries_no_items(server: Sut, root_token: str) -> None:
    """A filter matching no account answers 200 with no items."""
    response = server.request("GET", f"{server.href('user')}?username=nobody", token=root_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.text}"
    assert response.json()["items"] == []


@allure.epic("User")
@allure.feature("REQ-USER-010")
@allure.story("SCN-USER-010-D")
def test_malformed_filters_are_rejected(server: Sut, root_token: str) -> None:
    """An out-of-bounds username, an invalid email or an undeclared role answers 422."""
    queries = (
        "?username=abc",
        f"?username={'x' * 101}",
        "?username=",
        "?email=not-an-email",
        "?role=SUPERUSER",
    )

    for query in queries:
        response = server.request("GET", f"{server.href('user')}{query}", token=root_token)

        assert response.status_code == 422, f"{query} answered {response.status_code}: {response.text}"


@allure.epic("User")
@allure.feature("REQ-USER-011")
@allure.story("SCN-USER-011-A")
def test_the_caller_reads_its_own_account(server: Sut, root_token: str) -> None:
    """A request for the caller's own account answers 200 with that account's profile."""
    response = server.request("GET", server.href("current_user"), token=root_token)

    assert response.status_code == 200, f"read failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["id"] == 0
    assert body["username"] == ROOT_ADMIN_USERNAME
    assert body["role"] == "ADMIN"


@allure.epic("User")
@allure.feature("REQ-USER-003")
@allure.story("SCN-USER-003-A")
def test_surrounding_whitespace_username_is_rejected(server: Sut, root_token: str) -> None:
    """A username that begins or ends with whitespace answers 422 and stores nothing."""
    for username in (" alice", "alice ", " alice "):
        response = server.request(
            "POST",
            server.href("register"),
            token=root_token,
            json={"username": username, "email": None, "password": SECRET_PASSWORD, "role": "STANDARD"},
        )
        assert response.status_code == 422, f"{username!r} was accepted: {response.status_code} {response.text}"

    listed = server.request("GET", server.href("user"), token=root_token)
    assert {item["username"] for item in listed.json()["items"]} == {ROOT_ADMIN_USERNAME}


@allure.epic("User")
@allure.feature("REQ-USER-003")
@allure.story("SCN-USER-003-B")
def test_uppercase_username_is_rejected(server: Sut, root_token: str) -> None:
    """A username containing an uppercase letter answers 422 and stores nothing."""
    for username in ("Alice", "aliceSmith"):
        response = server.request(
            "POST",
            server.href("register"),
            token=root_token,
            json={"username": username, "email": None, "password": SECRET_PASSWORD, "role": "STANDARD"},
        )
        assert response.status_code == 422, f"{username!r} was accepted: {response.status_code} {response.text}"

    listed = server.request("GET", server.href("user"), token=root_token)
    assert {item["username"] for item in listed.json()["items"]} == {ROOT_ADMIN_USERNAME}


@allure.epic("User")
@allure.feature("REQ-USER-003")
@allure.story("SCN-USER-003-C")
def test_username_boundary_lengths_are_accepted(server: Sut, root_token: str) -> None:
    """A username of exactly 4 or exactly 100 characters is created with that username."""
    for username in ("abcd", "u" * 100):
        response = register(server, root_token, username, None, SECRET_PASSWORD)

        assert response.status_code == 201, f"{len(username)}-character name rejected: {response.text}"
        assert response.json()["username"] == username


@allure.epic("User")
@allure.feature("REQ-USER-003")
@allure.story("SCN-USER-003-F")
def test_taken_username_conflicts(server: Sut, root_token: str) -> None:
    """A username another account holds answers 409 and stores nothing."""
    first = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.text}"

    response = register(server, root_token, "alice", "other@example.com", SECRET_PASSWORD)

    assert response.status_code == 409, f"duplicate username accepted: {response.status_code} {response.text}"
    assert response.json()["error"] == "username is already taken"
    listed = server.request("GET", f"{server.href('user')}?username=alice", token=root_token)
    assert [item["email"] for item in listed.json()["items"]] == ["alice@example.com"]


@allure.epic("User")
@allure.feature("REQ-USER-004")
@allure.story("SCN-USER-004-A")
def test_omitted_email_is_stored_as_no_email(server: Sut, root_token: str) -> None:
    """A creation request without an email value creates the account with no email address."""
    response = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={"username": "alice", "password": SECRET_PASSWORD, "role": "STANDARD"},
    )

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    assert response.json()["email"] is None


@allure.epic("User")
@allure.feature("REQ-USER-004")
@allure.story("SCN-USER-004-B")
def test_null_email_is_stored_as_no_email(server: Sut, root_token: str) -> None:
    """A creation request with an explicit null email creates the account with no email address."""
    response = register(server, root_token, "alice", None, SECRET_PASSWORD)

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    assert response.json()["email"] is None


@allure.epic("User")
@allure.feature("REQ-USER-004")
@allure.story("SCN-USER-004-C")
def test_padded_email_is_rejected(server: Sut, root_token: str) -> None:
    """A valid address surrounded by whitespace answers 422 and stores nothing."""
    for email in (" alice@example.com", "alice@example.com ", " alice@example.com "):
        response = server.request(
            "POST",
            server.href("register"),
            token=root_token,
            json={"username": "alice", "email": email, "password": SECRET_PASSWORD, "role": "STANDARD"},
        )
        assert response.status_code == 422, f"{email!r} was accepted: {response.status_code} {response.text}"

    listed = server.request("GET", server.href("user"), token=root_token)
    assert {item["username"] for item in listed.json()["items"]} == {ROOT_ADMIN_USERNAME}


@allure.epic("User")
@allure.feature("REQ-USER-004")
@allure.story("SCN-USER-004-D")
def test_invalid_email_is_rejected(server: Sut, root_token: str) -> None:
    """A value that is not a valid email address answers 422 and stores nothing."""
    for email in ("not-an-email", "alice@", "@example.com", "alice example@example.com"):
        response = server.request(
            "POST",
            server.href("register"),
            token=root_token,
            json={"username": "alice", "email": email, "password": SECRET_PASSWORD, "role": "STANDARD"},
        )
        assert response.status_code == 422, f"{email!r} was accepted: {response.status_code} {response.text}"

    listed = server.request("GET", server.href("user"), token=root_token)
    assert {item["username"] for item in listed.json()["items"]} == {ROOT_ADMIN_USERNAME}


@allure.epic("User")
@allure.feature("REQ-USER-004")
@allure.story("SCN-USER-004-E")
def test_display_name_email_is_rejected(server: Sut, root_token: str) -> None:
    """A mailbox carrying a display name answers 422 and stores nothing."""
    response = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={
            "username": "alice",
            "email": "Alice <alice@example.com>",
            "password": SECRET_PASSWORD,
            "role": "STANDARD",
        },
    )

    assert response.status_code == 422, f"display name accepted: {response.status_code} {response.text}"
    listed = server.request("GET", server.href("user"), token=root_token)
    assert {item["username"] for item in listed.json()["items"]} == {ROOT_ADMIN_USERNAME}


@allure.epic("User")
@allure.feature("REQ-USER-004")
@allure.story("SCN-USER-004-F")
def test_taken_email_conflicts(server: Sut, root_token: str) -> None:
    """An email another account already holds answers 409 and stores nothing."""
    first = register(server, root_token, "alice", "shared@example.com", SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.text}"

    response = register(server, root_token, "bobby", "shared@example.com", SECRET_PASSWORD)

    assert response.status_code == 409, f"duplicate email accepted: {response.status_code} {response.text}"
    assert response.json()["error"] == "email is already taken"
    listed = server.request("GET", f"{server.href('user')}?username=bobby", token=root_token)
    assert listed.json()["items"] == []


@allure.epic("User")
@allure.feature("REQ-USER-007")
@allure.story("SCN-USER-007-A")
def test_a_created_account_has_one_credential(server: Sut, root_token: str) -> None:
    """A valid creation stores the account and one credential that authenticates it."""
    response = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    assert server.login("alice", SECRET_PASSWORD)

    listed = server.request("GET", f"{server.href('user')}?username=alice", token=root_token)
    items = listed.json()["items"]
    assert len(items) == 1
    assert items[0]["email"] == "alice@example.com"


@allure.epic("User")
@allure.feature("REQ-USER-007")
@allure.story("SCN-USER-007-B")
def test_a_rejected_creation_leaves_nothing(server: Sut, root_token: str) -> None:
    """A rejection for a duplicate or an invalid value leaves no account and no credential."""
    first = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.text}"

    duplicate = register(server, root_token, "alice", "second@example.com", SECRET_PASSWORD)
    invalid = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={"username": "bobby", "email": "not-an-email", "password": SECRET_PASSWORD, "role": "STANDARD"},
    )

    assert duplicate.status_code == 409, f"duplicate answered {duplicate.status_code}: {duplicate.text}"
    assert invalid.status_code == 422, f"invalid email answered {invalid.status_code}: {invalid.text}"
    assert server.attempt_login("alice", SECRET_PASSWORD).status_code == 200
    assert server.attempt_login("bobby", SECRET_PASSWORD).status_code == 401
    listed = server.request("GET", server.href("user"), token=root_token)
    assert {item["username"] for item in listed.json()["items"]} == {ROOT_ADMIN_USERNAME, "alice"}


@allure.epic("User")
@allure.feature("REQ-USER-008")
@allure.story("SCN-USER-008-A")
@allure.story("SCN-USER-008-B")
def test_the_created_profile_and_location(server: Sut, root_token: str) -> None:
    """A successful creation answers 201 with the profile and addresses the account in Location."""
    response = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["username"] == "alice"
    assert body["email"] == "alice@example.com"
    assert body["role"] == "STANDARD"
    assert body["id"] > 0
    assert response.headers.get("Location") == link_of(body, "self")


@allure.epic("User")
@allure.feature("REQ-USER-012")
@allure.story("SCN-USER-012-A")
def test_update_changes_the_username(server: Sut, root_token: str) -> None:
    """An update carrying a new valid username changes the stored username."""
    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    user_id = created.json()["id"]

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{user_id}",
        token=root_token,
        json={"username": "alicia"},
    )

    assert response.status_code == 200, f"update failed: {response.status_code} {response.text}"
    assert response.json()["username"] == "alicia"
    stored = server.request("GET", f"{server.href('user')}/{user_id}", token=root_token)
    assert stored.json()["username"] == "alicia"


@allure.epic("User")
@allure.feature("REQ-USER-012")
@allure.story("SCN-USER-012-B")
def test_update_changes_the_email(server: Sut, root_token: str) -> None:
    """An update carrying a new valid email changes the stored email."""
    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    user_id = created.json()["id"]

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{user_id}",
        token=root_token,
        json={"email": "alicia@example.org"},
    )

    assert response.status_code == 200, f"update failed: {response.status_code} {response.text}"
    assert response.json()["email"] == "alicia@example.org"
    stored = server.request("GET", f"{server.href('user')}/{user_id}", token=root_token)
    assert stored.json()["email"] == "alicia@example.org"


@allure.epic("User")
@allure.feature("REQ-USER-012")
@allure.story("SCN-USER-012-C")
def test_update_changes_the_role(server: Sut, root_token: str) -> None:
    """An update carrying a declared role changes the stored role."""
    created = register(server, root_token, "alice", None, SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    user_id = created.json()["id"]

    response = server.request("PATCH", f"{server.href('user')}/{user_id}", token=root_token, json={"role": "ADMIN"})

    assert response.status_code == 200, f"update failed: {response.status_code} {response.text}"
    assert response.json()["role"] == "ADMIN"
    stored = server.request("GET", f"{server.href('user')}/{user_id}", token=root_token)
    assert stored.json()["role"] == "ADMIN"


@allure.epic("User")
@allure.feature("REQ-USER-012")
@allure.story("SCN-USER-012-D")
def test_update_leaves_omitted_fields_unchanged(server: Sut, root_token: str) -> None:
    """An update carrying one field leaves every other stored value unchanged."""
    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    profile = created.json()

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{profile['id']}",
        token=root_token,
        json={"email": "alicia@example.org"},
    )

    assert response.status_code == 200, f"update failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["email"] == "alicia@example.org"
    assert body["username"] == profile["username"]
    assert body["role"] == profile["role"]


@allure.epic("User")
@allure.feature("REQ-USER-012")
@allure.story("SCN-USER-012-E")
def test_an_empty_body_changes_nothing(server: Sut, root_token: str) -> None:
    """An update carrying no field answers 200 and leaves every stored value unchanged."""
    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    profile = created.json()

    response = server.request("PATCH", f"{server.href('user')}/{profile['id']}", token=root_token, json={})

    assert response.status_code == 200, f"update failed: {response.status_code} {response.text}"
    stored = server.request("GET", f"{server.href('user')}/{profile['id']}", token=root_token)
    assert stored.json()["username"] == profile["username"]
    assert stored.json()["email"] == profile["email"]
    assert stored.json()["role"] == profile["role"]


@allure.epic("User")
@allure.feature("REQ-USER-012")
@allure.story("SCN-USER-012-F")
def test_an_update_of_an_unknown_account_is_not_found(server: Sut, root_token: str) -> None:
    """An update addressing an identifier that names no account answers 404 and stores nothing."""
    created = register(server, root_token, "alice", None, SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"

    response = server.request("PATCH", f"{server.href('user')}/999999", token=root_token, json={"username": "ghost"})

    assert response.status_code == 404, f"unknown account answered {response.status_code}: {response.text}"
    listed = server.request("GET", f"{server.href('user')}?username=ghost", token=root_token)
    assert listed.json()["items"] == []


@allure.epic("User")
@allure.feature("REQ-USER-013")
@allure.story("SCN-USER-013-A")
def test_a_rename_onto_a_taken_username_conflicts(server: Sut, root_token: str) -> None:
    """A rename onto another account's username answers 409 and changes nothing."""
    alice = register(server, root_token, "alice", None, SECRET_PASSWORD)
    assert alice.status_code == 201, f"creation failed: {alice.status_code} {alice.text}"
    bobby = register(server, root_token, "bobby", None, SECRET_PASSWORD)
    assert bobby.status_code == 201, f"creation failed: {bobby.status_code} {bobby.text}"
    alice_id = alice.json()["id"]

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{alice_id}",
        token=root_token,
        json={"username": "bobby"},
    )

    assert response.status_code == 409, f"duplicate rename answered {response.status_code}: {response.text}"
    stored = server.request("GET", f"{server.href('user')}/{alice_id}", token=root_token)
    assert stored.json()["username"] == "alice"


@allure.epic("User")
@allure.feature("REQ-USER-013")
@allure.story("SCN-USER-013-B")
def test_an_email_collision_on_update_conflicts(server: Sut, root_token: str) -> None:
    """An update carrying another account's email answers 409 and changes nothing."""
    alice = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert alice.status_code == 201, f"creation failed: {alice.status_code} {alice.text}"
    bobby = register(server, root_token, "bobby", "bobby@example.com", SECRET_PASSWORD)
    assert bobby.status_code == 201, f"creation failed: {bobby.status_code} {bobby.text}"
    alice_id = alice.json()["id"]

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{alice_id}",
        token=root_token,
        json={"email": "bobby@example.com"},
    )

    assert response.status_code == 409, f"duplicate email answered {response.status_code}: {response.text}"
    stored = server.request("GET", f"{server.href('user')}/{alice_id}", token=root_token)
    assert stored.json()["email"] == "alice@example.com"


@allure.epic("User")
@allure.feature("REQ-USER-013")
@allure.story("SCN-USER-013-C")
def test_keeping_the_accounts_own_value_is_not_a_collision(server: Sut, root_token: str) -> None:
    """An update carrying the account's current username and email answers 200 and stores them."""
    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    profile = created.json()

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{profile['id']}",
        token=root_token,
        json={"username": profile["username"], "email": profile["email"]},
    )

    assert response.status_code == 200, f"own values answered {response.status_code}: {response.text}"
    body = response.json()
    assert body["username"] == profile["username"]
    assert body["email"] == profile["email"]


@allure.epic("User")
@allure.feature("REQ-USER-014")
@allure.story("SCN-USER-014-A")
def test_an_update_with_an_invalid_username_is_rejected(server: Sut, root_token: str) -> None:
    """A username that breaks a creation rule answers 422 and leaves the stored values unchanged."""
    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    profile = created.json()

    for username in (" Alice", "Alice", "alice "):
        response = server.request(
            "PATCH",
            f"{server.href('user')}/{profile['id']}",
            token=root_token,
            json={"username": username},
        )

        assert response.status_code == 422, f"{username!r} was accepted: {response.status_code} {response.text}"

    stored = server.request("GET", f"{server.href('user')}/{profile['id']}", token=root_token)
    assert stored.json()["username"] == profile["username"]
    assert stored.json()["email"] == profile["email"]


@allure.epic("User")
@allure.feature("REQ-USER-014")
@allure.story("SCN-USER-014-B")
def test_an_update_with_an_invalid_email_is_rejected(server: Sut, root_token: str) -> None:
    """An invalid email address answers 422 and leaves the stored values unchanged."""
    created = register(server, root_token, "alice", "alice@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    profile = created.json()

    for email in ("not-an-email", "Alice <alice@example.com>", " alice@example.com"):
        response = server.request(
            "PATCH",
            f"{server.href('user')}/{profile['id']}",
            token=root_token,
            json={"email": email},
        )

        assert response.status_code == 422, f"{email!r} was accepted: {response.status_code} {response.text}"

    stored = server.request("GET", f"{server.href('user')}/{profile['id']}", token=root_token)
    assert stored.json()["email"] == profile["email"]
    assert stored.json()["username"] == profile["username"]


@allure.epic("User")
@allure.feature("REQ-USER-015")
@allure.story("SCN-USER-015-A")
def test_deletion_removes_the_identity(server: Sut, root_token: str) -> None:
    """A deletion answers 204 and removes the account, its credential and its permission overrides."""
    target_id, target_token = provision(server, root_token, "alice")

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    assert response.content == b""
    assert server.request("GET", f"{server.href('user')}/{target_id}", token=root_token).status_code == 404
    assert server.request("GET", f"{server.href('user')}/{target_id}/permission", token=root_token).status_code == 404
    assert server.attempt_login("alice", SECRET_PASSWORD).status_code == 401
    assert server.request("GET", server.href("current_user"), token=target_token).status_code == 401


@allure.epic("User")
@allure.feature("REQ-USER-015")
@allure.story("SCN-USER-015-B")
def test_deletion_of_an_unknown_account_is_not_found(server: Sut, root_token: str) -> None:
    """A deletion addressing an identifier that names no account answers 404."""
    response = server.request("DELETE", f"{server.href('user')}/999999", token=root_token)

    assert response.status_code == 404, f"unknown account answered {response.status_code}: {response.text}"


@allure.epic("User")
@allure.feature("REQ-USER-016")
@allure.story("SCN-USER-016-A")
@allure.story("SCN-USER-016-B")
@allure.story("SCN-USER-016-C")
def test_deletion_removes_owned_data(server: Sut, root_token: str) -> None:
    """A deleted account's gallery with its item, playlist and unfinished upload session are gone."""
    target_id, target_token = provision(server, root_token, "alice")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    file = upload(server, target_token, "owned.png", image)
    gallery = create_gallery(server, target_token, "Owned Gallery")
    item = add_to_gallery(server, target_token, gallery, file["id"])
    playlist = server.request(
        "POST",
        server.href("playlist"),
        token=target_token,
        json={"name": "Owned Playlist", "is_public": False},
    )
    assert playlist.status_code == 201, f"playlist failed: {playlist.status_code} {playlist.text}"
    upload_session = begin_upload(server, target_token, "unfinished.png", image)

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    assert server.request("GET", link_of(gallery, "self"), token=root_token).status_code == 404
    assert server.request("GET", link_of(item, "self"), token=root_token).status_code == 404
    assert server.request("GET", link_of(playlist.json(), "self"), token=root_token).status_code == 404
    assert server.request("GET", upload_session.self_href, token=root_token).status_code == 404


@allure.epic("User")
@allure.feature("REQ-USER-017")
@allure.story("SCN-USER-017-A")
def test_private_gallery_media_is_removed_with_its_bytes(server: Sut, root_token: str) -> None:
    """An image referenced only by the account's own gallery is deleted with its record and bytes."""
    target_id, target_token = provision(server, root_token, "alice")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    file = upload(server, target_token, "private.png", image)
    image_href = link_of(file, "image")
    gallery = create_gallery(server, target_token, "Private Gallery")
    add_to_gallery(server, target_token, gallery, file["id"])
    assert server.exists(f"media/{file['path']}")

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    assert server.request("GET", link_of(file, "self"), token=root_token).status_code == 404
    assert server.request("GET", image_href, token=root_token).status_code == 404
    assert not server.exists(f"media/{file['path']}")


@allure.epic("User")
@allure.feature("REQ-USER-017")
@allure.story("SCN-USER-017-C")
def test_the_accounts_own_public_gallery_is_not_a_shared_reference(server: Sut, root_token: str) -> None:
    """An image referenced only by the account's own public gallery is deleted with the account."""
    target_id, target_token = provision(server, root_token, "alice")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    file = upload(server, target_token, "public.png", image)
    image_href = link_of(file, "image")
    gallery = create_gallery(server, target_token, "Public Gallery", is_public=True)
    add_to_gallery(server, target_token, gallery, file["id"])

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    assert server.request("GET", link_of(file, "self"), token=root_token).status_code == 404
    assert server.request("GET", image_href, token=root_token).status_code == 404
    assert not server.exists(f"media/{file['path']}")


@allure.epic("User")
@allure.feature("REQ-USER-018")
@allure.story("SCN-USER-018-A")
def test_an_image_in_another_gallery_survives_under_account_zero(server: Sut, root_token: str) -> None:
    """An image referenced by another account's gallery survives, owned by account 0."""
    target_id, target_token = provision(server, root_token, "alice")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    file = upload(server, target_token, "shared.png", image)
    gallery = create_gallery(server, root_token, "Shared Gallery")
    item = add_to_gallery(server, root_token, gallery, file["id"])

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    gallery_read = server.request("GET", link_of(gallery, "self"), token=root_token)
    assert gallery_read.status_code == 200, f"gallery failed: {gallery_read.status_code} {gallery_read.text}"
    assert item["id"] in [entry["id"] for entry in gallery_read.json()["items"]]
    profile = file_profile(server, root_token, file)
    assert profile["_links"]["owner"]["href"] == "/api/v1/user/0"
    content = server.request("GET", profile["_links"]["content"]["href"], token=root_token)
    assert content.status_code == 200
    assert content.content.startswith(b"\x89PNG")


@allure.epic("User")
@allure.feature("REQ-USER-018")
@allure.story("SCN-USER-018-B")
def test_a_video_backing_a_film_survives(server: Sut, root_token: str) -> None:
    """A video backing a film survives the deletion and the film stays readable."""
    target_id, target_token = provision(server, root_token, "alice")
    video = (FIXTURE_DIRECTORY / "clip.mp4").read_bytes()
    file = upload(server, target_token, "film.mp4", video, "video/mp4")
    video_id = linked_id(link_of(file, "video"))
    created = server.request(
        "POST",
        server.href("movie"),
        token=root_token,
        json={"name": "Surviving Film", "video_id": video_id},
    )
    assert created.status_code == 201, f"movie failed: {created.status_code} {created.text}"
    movie_href = link_of(created.json(), "self")

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    assert server.request("GET", movie_href, token=root_token).status_code == 200
    profile = file_profile(server, root_token, file)
    assert profile["_links"]["owner"]["href"] == "/api/v1/user/0"
    content = server.request("GET", profile["_links"]["content"]["href"], token=root_token)
    assert content.status_code == 200
    assert content.content


@allure.epic("User")
@allure.feature("REQ-USER-018")
@allure.story("SCN-USER-018-C")
def test_catalogue_artwork_survives(server: Sut, root_token: str) -> None:
    """An image referenced as catalogue artwork stays readable through the catalogue resource."""
    target_id, target_token = provision(server, root_token, "alice")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    image_file = upload(server, target_token, "artwork.png", image)
    image_href = link_of(image_file, "image")
    image_id = linked_id(image_href)
    video = (FIXTURE_DIRECTORY / "clip.mp4").read_bytes()
    video_file = upload(server, target_token, "artwork.mp4", video, "video/mp4")
    video_id = linked_id(link_of(video_file, "video"))
    created = server.request(
        "POST",
        server.href("movie"),
        token=root_token,
        json={"name": "Illustrated Film", "video_id": video_id, "cover_image_id": image_id},
    )
    assert created.status_code == 201, f"movie failed: {created.status_code} {created.text}"

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.text}"
    artwork = server.request("GET", image_href, token=root_token)
    assert artwork.status_code == 200, f"artwork failed: {artwork.status_code} {artwork.text}"
    assert file_profile(server, root_token, image_file)["_links"]["owner"]["href"] == "/api/v1/user/0"


@allure.epic("User")
@allure.feature("REQ-USER-019")
@allure.story("SCN-USER-019-A")
def test_a_colliding_digest_is_cleared_on_transfer(server: Sut, root_token: str) -> None:
    """Two accounts' identical-content files both survive, so the second transfer cleared its digest."""
    alice_id, alice_token = provision(server, root_token, "alice")
    bobby_id, bobby_token = provision(server, root_token, "bobby")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    alice_file = upload(server, alice_token, "alice.png", image)
    bobby_file = upload(server, bobby_token, "bobby.png", image)
    assert alice_file["id"] != bobby_file["id"]
    gallery = create_gallery(server, root_token, "Colliding Digests")
    add_to_gallery(server, root_token, gallery, alice_file["id"])
    add_to_gallery(server, root_token, gallery, bobby_file["id"])

    first = server.request("DELETE", f"{server.href('user')}/{alice_id}", token=root_token)
    second = server.request("DELETE", f"{server.href('user')}/{bobby_id}", token=root_token)

    assert first.status_code == 204, f"first delete failed: {first.status_code} {first.text}"
    assert second.status_code == 204, f"second delete failed: {second.status_code} {second.text}"
    for file in (alice_file, bobby_file):
        profile = file_profile(server, root_token, file)
        assert profile["_links"]["owner"]["href"] == "/api/v1/user/0"
        assert server.request("GET", profile["_links"]["content"]["href"], token=root_token).status_code == 200

    # The digest is not exposed by the API, so the second transfer's row is read
    # from the working directory's datastore.
    connection = sqlite3.connect(server.dir / "mnemorium.db")
    try:
        digests = dict(connection.execute("SELECT file_id, integrity_hash FROM file").fetchall())
    finally:
        connection.close()
    assert digests[alice_file["id"]] is not None
    assert digests[bobby_file["id"]] is None
