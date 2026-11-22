"""Black-box proof of the `user` capability."""

import json
from itertools import count

import pytest
from conftest import (
    SEED_MEDIA_DIRECTORY,
    add_to_gallery,
    create_gallery,
    login,
    provision,
    upload,
)
from harness import (
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    Sut,
    begin_upload,
    register,
)
from labels import scenario
from mnemorium_client.api.identity import post_login
from mnemorium_client.models import LoginRequest

IMAGE_BYTES = (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes()
VIDEO_BYTES = (SEED_MEDIA_DIRECTORY / "file-video.mp4").read_bytes()
IMAGE_CONTENT_TYPE = "image/png"
VIDEO_CONTENT_TYPE = "video/mp4"

_ACCOUNTS = count()


@scenario("SCN-USER-001-A")
@scenario("SCN-USER-005-A")
@scenario("SCN-USER-009-A")
def test_a_created_account_reads_back_its_profile(server: Sut, root_token: str) -> None:
    """An account reads back through its username with its identifier, email or no email, and role."""
    with_email = f"with{next(_ACCOUNTS)}"
    without_email = f"bare{next(_ACCOUNTS)}"
    first = register(server, root_token, with_email, f"{with_email}@example.com", SECRET_PASSWORD)
    second = register(server, root_token, without_email, None, SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.content!r}"
    assert second.status_code == 201, f"creation failed: {second.status_code} {second.content!r}"

    for created, expected_email in ((first, f"{with_email}@example.com"), (second, None)):
        profile = json.loads(created.content)
        listed = server.request("GET", f"{server.href('user')}?username={profile['username']}", token=root_token)
        assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
        items = json.loads(listed.content)["items"]
        assert [item["id"] for item in items] == [profile["id"]]
        item = items[0]
        assert isinstance(item["id"], int) and item["id"] > 0
        assert item["username"] == profile["username"]
        assert item["email"] == expected_email
        assert item["role"] == "STANDARD"


@scenario("SCN-USER-011-A")
def test_the_caller_reads_its_own_account(server: Sut, root_token: str) -> None:
    """A request for the caller's own account answers 200 with that account's profile."""
    response = server.request("GET", server.href("current_user"), token=root_token)

    assert response.status_code == 200, f"read failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)
    assert body["id"] == 0
    assert body["username"] == ROOT_ADMIN_USERNAME
    assert body["role"] == "ADMIN"


@scenario("SCN-USER-002-A")
def test_a_declared_role_is_stored(server: Sut, root_token: str) -> None:
    """A creation carrying ADMIN or STANDARD stores and returns that role."""
    for role in ("ADMIN", "STANDARD"):
        name = f"{role.lower()}{next(_ACCOUNTS)}"
        created = register(server, root_token, name, None, SECRET_PASSWORD, role=role)
        assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"

        listed = server.request("GET", f"{server.href('user')}?username={name}", token=root_token)
        items = json.loads(listed.content)["items"]
        assert [item["role"] for item in items] == [role]
        assert json.loads(created.content)["role"] == role


@scenario("SCN-USER-002-B")
def test_an_undeclared_role_is_rejected(server: Sut, root_token: str) -> None:
    """A role outside the declared values answers 422 and stores nothing."""
    name = f"role{next(_ACCOUNTS)}"
    response = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={"username": name, "email": None, "password": SECRET_PASSWORD, "role": "SUPERUSER"},
    )

    assert response.status_code == 422, f"undeclared role accepted: {response.status_code} {response.content!r}"
    listed = server.request("GET", f"{server.href('user')}?username={name}", token=root_token)
    assert json.loads(listed.content)["items"] == []


@scenario("SCN-USER-020-A")
def test_a_username_with_whitespace_is_rejected(server: Sut, root_token: str) -> None:
    """A username containing whitespace at any position answers 422 and stores nothing."""
    base = f"name{next(_ACCOUNTS)}"
    for username in (f" {base}", f"{base} ", f"al ice{next(_ACCOUNTS)}", f"\t{base}"):
        response = server.request(
            "POST",
            server.href("register"),
            token=root_token,
            json={"username": username, "email": None, "password": SECRET_PASSWORD, "role": "STANDARD"},
        )
        assert response.status_code == 422, f"{username!r} was accepted: {response.status_code} {response.content!r}"

    listed = server.request("GET", f"{server.href('user')}?username={base}", token=root_token)
    assert json.loads(listed.content)["items"] == [], "a refused username was stored"


@scenario("SCN-USER-020-B")
def test_an_uppercase_username_is_accepted_and_read_back(server: Sut, root_token: str) -> None:
    """`Alice` and `aliceSmith` are created with 201 and read back unchanged."""
    for username in ("Alice", "aliceSmith"):
        response = register(server, root_token, username, None, SECRET_PASSWORD)

        assert response.status_code == 201, f"{username!r} was rejected: {response.content!r}"
        assert json.loads(response.content)["username"] == username
        listed = server.request("GET", f"{server.href('user')}?username={username}", token=root_token)
        items = json.loads(listed.content)["items"]
        assert [item["username"] for item in items] == [username]


@scenario("SCN-USER-020-C")
def test_username_boundary_lengths_are_accepted(server: Sut, root_token: str) -> None:
    """A username of exactly 4 or exactly 100 characters is created with that username."""
    short = f"a{next(_ACCOUNTS):03d}"
    long = f"u{next(_ACCOUNTS):03d}" + "x" * 96
    for username in (short, long):
        response = register(server, root_token, username, None, SECRET_PASSWORD)

        assert response.status_code == 201, f"{len(username)}-character name rejected: {response.content!r}"
        assert json.loads(response.content)["username"] == username


@scenario("SCN-USER-020-D")
@scenario("SCN-USER-020-E")
def test_username_boundary_lengths_are_rejected(server: Sut, root_token: str) -> None:
    """A username of 3 or 101 characters answers 422 and stores nothing."""
    too_short = "abc"
    too_long = "x" * 101
    assert len(too_short) == 3
    assert len(too_long) == 101
    for username in (too_short, too_long):
        response = server.request(
            "POST",
            server.href("register"),
            token=root_token,
            json={"username": username, "email": None, "password": SECRET_PASSWORD, "role": "STANDARD"},
        )

        assert response.status_code == 422, f"{len(username)}-character name accepted: {response.content!r}"

    listed = server.request("GET", server.href("user"), token=root_token)
    assert listed.status_code == 200, f"list failed: {listed.status_code} {listed.content!r}"
    stored = {item["username"] for item in json.loads(listed.content)["items"]}
    assert too_short not in stored and too_long not in stored, "a refused username was stored"


@scenario("SCN-USER-020-F")
@scenario("SCN-USER-004-F")
def test_a_taken_username_or_email_is_refused(server: Sut, root_token: str) -> None:
    """A username or email another account holds answers 409 and stores nothing."""
    holder = f"holder{next(_ACCOUNTS)}"
    challenger = f"challenger{next(_ACCOUNTS)}"
    email = f"{holder}@example.com"
    first = register(server, root_token, holder, email, SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.content!r}"

    duplicate_name = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={
            "username": holder,
            "email": f"{challenger}@example.com",
            "password": SECRET_PASSWORD,
            "role": "STANDARD",
        },
    )
    duplicate_email = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={"username": challenger, "email": email, "password": SECRET_PASSWORD, "role": "STANDARD"},
    )

    assert duplicate_name.status_code == 409, f"duplicate username accepted: {duplicate_name.status_code}"
    assert json.loads(duplicate_name.content)["error"] == "username is already taken"
    assert duplicate_email.status_code == 409, f"duplicate email accepted: {duplicate_email.status_code}"
    assert json.loads(duplicate_email.content)["error"] == "email is already taken"
    stored = server.request("GET", f"{server.href('user')}?username={holder}", token=root_token)
    items = json.loads(stored.content)["items"]
    assert [item["email"] for item in items] == [email], "the refused creation changed the holder"
    listed = server.request("GET", f"{server.href('user')}?username={challenger}", token=root_token)
    assert json.loads(listed.content)["items"] == [], "a refused creation stored an account"


@scenario("SCN-USER-004-A")
@scenario("SCN-USER-004-B")
def test_an_omitted_or_null_email_is_stored_as_none(server: Sut, root_token: str) -> None:
    """A creation with no email value or an explicit null stores no email address."""
    omitted_name = f"omit{next(_ACCOUNTS)}"
    null_name = f"null{next(_ACCOUNTS)}"
    omitted = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={"username": omitted_name, "password": SECRET_PASSWORD, "role": "STANDARD"},
    )
    explicit_null = register(server, root_token, null_name, None, SECRET_PASSWORD)

    assert omitted.status_code == 201, f"creation failed: {omitted.status_code} {omitted.content!r}"
    assert explicit_null.status_code == 201, f"creation failed: {explicit_null.status_code} {explicit_null.content!r}"
    for name in (omitted_name, null_name):
        listed = server.request("GET", f"{server.href('user')}?username={name}", token=root_token)
        items = json.loads(listed.content)["items"]
        assert [item["email"] for item in items] == [None]


@scenario("SCN-USER-004-C")
@scenario("SCN-USER-004-D")
@scenario("SCN-USER-004-E")
def test_an_invalid_mailbox_is_rejected(server: Sut, root_token: str) -> None:
    """A padded, malformed or display-name mailbox answers 422 and stores nothing."""
    name = f"mail{next(_ACCOUNTS)}"
    for email in (" invalid@example.com", "invalid@example.com ", "not-an-email", "Alice <alice@example.com>"):
        response = server.request(
            "POST",
            server.href("register"),
            token=root_token,
            json={"username": name, "email": email, "password": SECRET_PASSWORD, "role": "STANDARD"},
        )
        assert response.status_code == 422, f"{email!r} was accepted: {response.status_code} {response.content!r}"

    listed = server.request("GET", f"{server.href('user')}?username={name}", token=root_token)
    assert json.loads(listed.content)["items"] == [], "a refused creation stored an account"


@scenario("SCN-USER-004-G")
def test_an_email_case_variant_is_a_conflict(server: Sut, root_token: str) -> None:
    """A case variant of a stored address answers 409 and creates no second account."""
    holder = f"case{next(_ACCOUNTS)}"
    challenger = f"caseb{next(_ACCOUNTS)}"
    email = f"{holder}@example.com"
    first = register(server, root_token, holder, email, SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.content!r}"

    response = register(server, root_token, challenger, email.upper(), SECRET_PASSWORD)

    assert response.status_code == 409, f"case variant accepted: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["error"] == "email is already taken"
    listed = server.request("GET", f"{server.href('user')}?username={challenger}", token=root_token)
    assert json.loads(listed.content)["items"] == []


@scenario("SCN-USER-007-A")
def test_a_created_account_has_one_credential(server: Sut, root_token: str) -> None:
    """A valid creation stores the account and one credential that authenticates it."""
    name = f"cred{next(_ACCOUNTS)}"
    response = register(server, root_token, name, f"{name}@example.com", SECRET_PASSWORD)

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    assert login(server, name, SECRET_PASSWORD)
    listed = server.request("GET", f"{server.href('user')}?username={name}", token=root_token)
    items = json.loads(listed.content)["items"]
    assert len(items) == 1
    assert items[0]["email"] == f"{name}@example.com"


@scenario("SCN-USER-007-B")
def test_a_rejected_creation_leaves_nothing(server: Sut, root_token: str) -> None:
    """A rejection for a duplicate or an invalid value leaves no account and no credential."""
    holder = f"keep{next(_ACCOUNTS)}"
    ghost = f"ghost{next(_ACCOUNTS)}"
    first = register(server, root_token, holder, f"{holder}@example.com", SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.content!r}"

    duplicate = register(server, root_token, holder, f"{ghost}@example.com", SECRET_PASSWORD)
    invalid = server.request(
        "POST",
        server.href("register"),
        token=root_token,
        json={"username": ghost, "email": "not-an-email", "password": SECRET_PASSWORD, "role": "STANDARD"},
    )

    assert duplicate.status_code == 409, f"duplicate answered {duplicate.status_code}: {duplicate.content!r}"
    assert invalid.status_code == 422, f"invalid email answered {invalid.status_code}: {invalid.content!r}"
    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=SECRET_PASSWORD, identifier=holder)
        ).status_code
        == 200
    )
    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=SECRET_PASSWORD, identifier=ghost)
        ).status_code
        == 401
    )
    listed = server.request("GET", f"{server.href('user')}?username={ghost}", token=root_token)
    assert json.loads(listed.content)["items"] == []


@scenario("SCN-USER-008-A")
@scenario("SCN-USER-008-B")
def test_a_creation_returns_the_profile_and_location(server: Sut, root_token: str) -> None:
    """A successful creation answers 201 with the profile and addresses the account in Location."""
    name = f"made{next(_ACCOUNTS)}"
    response = register(server, root_token, name, f"{name}@example.com", SECRET_PASSWORD)

    assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"
    body = json.loads(response.content)
    assert body["username"] == name
    assert body["email"] == f"{name}@example.com"
    assert body["role"] == "STANDARD"
    assert body["id"] > 0
    assert response.headers.get("Location") == body["_links"]["self"]["href"]


@scenario("SCN-USER-010-A")
@pytest.mark.own_instance(reason="exact_collection")
def test_no_filter_returns_every_account(sut: Sut) -> None:
    """The collection requested without a filter carries every account."""
    server = sut.start()
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    names = [f"listed{next(_ACCOUNTS)}" for _ in range(3)]
    for name in names:
        response = register(server, root_token, name, None, SECRET_PASSWORD)
        assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"

    response = server.request("GET", server.href("user"), token=root_token)

    assert response.status_code == 200, f"list failed: {response.status_code} {response.content!r}"
    assert {item["username"] for item in json.loads(response.content)["items"]} == {
        ROOT_ADMIN_USERNAME,
        *names,
    }


@scenario("SCN-USER-010-B")
def test_filters_narrow_the_result(server: Sut, root_token: str) -> None:
    """Only the accounts matching every supplied filter appear."""
    standard = f"std{next(_ACCOUNTS)}"
    admin = f"adm{next(_ACCOUNTS)}"
    combined = f"both{next(_ACCOUNTS)}"
    registrations = (
        (standard, f"{standard}@example.com", "STANDARD"),
        (admin, f"{admin}@example.com", "ADMIN"),
        (combined, f"{combined}@example.com", "STANDARD"),
    )
    for username, email, role in registrations:
        response = register(server, root_token, username, email, SECRET_PASSWORD, role=role)
        assert response.status_code == 201, f"creation failed: {response.status_code} {response.content!r}"

    queries = (
        (f"?username={standard}", {standard}),
        (f"?email={admin}@example.com", {admin}),
        (f"?role=ADMIN&username={admin}", {admin}),
        (f"?role=STANDARD&email={combined}@example.com", {combined}),
    )

    for query, expected in queries:
        response = server.request("GET", f"{server.href('user')}{query}", token=root_token)

        assert response.status_code == 200, f"{query} failed: {response.status_code} {response.content!r}"
        assert {item["username"] for item in json.loads(response.content)["items"]} == expected


@scenario("SCN-USER-010-C")
def test_an_empty_result_carries_no_items(server: Sut, root_token: str) -> None:
    """A filter matching no account answers 200 with no items."""
    response = server.request(
        "GET",
        f"{server.href('user')}?username=nobody{next(_ACCOUNTS)}",
        token=root_token,
    )

    assert response.status_code == 200, f"list failed: {response.status_code} {response.content!r}"
    assert json.loads(response.content)["items"] == []


@scenario("SCN-USER-010-E")
def test_the_email_filter_ignores_case(server: Sut, root_token: str) -> None:
    """An email filter that differs from the stored address only by case finds the account."""
    name = f"casef{next(_ACCOUNTS)}"
    email = f"{name}@example.com"
    created = register(server, root_token, name, email, SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"

    response = server.request("GET", f"{server.href('user')}?email={email.upper()}", token=root_token)

    assert response.status_code == 200, f"filter failed: {response.status_code} {response.content!r}"
    assert [item["username"] for item in json.loads(response.content)["items"]] == [name]


@scenario("SCN-USER-012-A")
@scenario("SCN-USER-012-B")
@scenario("SCN-USER-012-C")
def test_an_update_changes_each_submitted_field(server: Sut, root_token: str) -> None:
    """Each submitted username, email and role is stored and reported."""
    name = f"edit{next(_ACCOUNTS)}"
    created = register(server, root_token, name, f"{name}@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"
    profile = json.loads(created.content)
    user_id = profile["id"]

    renamed = server.request(
        "PATCH",
        f"{server.href('user')}/{user_id}",
        token=root_token,
        json={"username": f"{name}x"},
    )
    assert renamed.status_code == 200, f"update failed: {renamed.status_code} {renamed.content!r}"
    assert json.loads(renamed.content)["username"] == f"{name}x"
    emailed = server.request(
        "PATCH",
        f"{server.href('user')}/{user_id}",
        token=root_token,
        json={"email": f"{name}@example.org"},
    )
    assert emailed.status_code == 200, f"update failed: {emailed.status_code} {emailed.content!r}"
    assert json.loads(emailed.content)["email"] == f"{name}@example.org"
    promoted = server.request(
        "PATCH",
        f"{server.href('user')}/{user_id}",
        token=root_token,
        json={"role": "ADMIN"},
    )
    assert promoted.status_code == 200, f"update failed: {promoted.status_code} {promoted.content!r}"
    assert json.loads(promoted.content)["role"] == "ADMIN"

    stored = server.request("GET", f"{server.href('user')}/{user_id}", token=root_token)
    body = json.loads(stored.content)
    assert body["username"] == f"{name}x"
    assert body["email"] == f"{name}@example.org"
    assert body["role"] == "ADMIN"


@scenario("SCN-USER-012-D")
@scenario("SCN-USER-012-E")
def test_an_update_leaves_omitted_fields_and_an_empty_body_unchanged(server: Sut, root_token: str) -> None:
    """An update carrying one field, then no field, leaves every other stored value unchanged."""
    name = f"keepf{next(_ACCOUNTS)}"
    created = register(server, root_token, name, f"{name}@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"
    profile = json.loads(created.content)

    emailed = server.request(
        "PATCH",
        f"{server.href('user')}/{profile['id']}",
        token=root_token,
        json={"email": f"{name}@example.org"},
    )
    assert emailed.status_code == 200, f"update failed: {emailed.status_code} {emailed.content!r}"
    body = json.loads(emailed.content)
    assert body["email"] == f"{name}@example.org"
    assert body["username"] == profile["username"]
    assert body["role"] == profile["role"]

    empty = server.request("PATCH", f"{server.href('user')}/{profile['id']}", token=root_token, json={})
    assert empty.status_code == 200, f"update failed: {empty.status_code} {empty.content!r}"
    stored = json.loads(empty.content)
    assert stored["username"] == profile["username"]
    assert stored["email"] == f"{name}@example.org"
    assert stored["role"] == profile["role"]


@scenario("SCN-USER-013-A")
@scenario("SCN-USER-013-B")
def test_an_update_onto_a_taken_username_or_email_conflicts(server: Sut, root_token: str) -> None:
    """A rename onto another account's username or email answers 409 and changes nothing."""
    alice = f"useralice{next(_ACCOUNTS)}"
    bobby = f"userbobby{next(_ACCOUNTS)}"
    first = register(server, root_token, alice, f"{alice}@example.com", SECRET_PASSWORD)
    second = register(server, root_token, bobby, f"{bobby}@example.com", SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.content!r}"
    assert second.status_code == 201, f"creation failed: {second.status_code} {second.content!r}"
    alice_id = json.loads(first.content)["id"]

    rename = server.request(
        "PATCH",
        f"{server.href('user')}/{alice_id}",
        token=root_token,
        json={"username": bobby},
    )
    email = server.request(
        "PATCH",
        f"{server.href('user')}/{alice_id}",
        token=root_token,
        json={"email": f"{bobby}@example.com"},
    )

    assert rename.status_code == 409, f"duplicate rename answered {rename.status_code}: {rename.content!r}"
    assert email.status_code == 409, f"duplicate email answered {email.status_code}: {email.content!r}"
    stored = json.loads(server.request("GET", f"{server.href('user')}/{alice_id}", token=root_token).content)
    assert stored["username"] == alice
    assert stored["email"] == f"{alice}@example.com"


@scenario("SCN-USER-013-C")
def test_an_update_keeping_the_own_value_is_not_a_collision(server: Sut, root_token: str) -> None:
    """An update carrying the account's current username and email answers 200 and stores them."""
    name = f"self{next(_ACCOUNTS)}"
    created = register(server, root_token, name, f"{name}@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"
    profile = json.loads(created.content)

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{profile['id']}",
        token=root_token,
        json={"username": profile["username"], "email": profile["email"]},
    )

    assert response.status_code == 200, f"own values answered {response.status_code}: {response.content!r}"
    body = json.loads(response.content)
    assert body["username"] == profile["username"]
    assert body["email"] == profile["email"]


@scenario("SCN-USER-013-D")
def test_a_case_only_email_change_conflicts(server: Sut, root_token: str) -> None:
    """An update carrying another account's address in a different case answers 409 and changes nothing."""
    david = f"userdavid{next(_ACCOUNTS)}"
    erin = f"usererin{next(_ACCOUNTS)}"
    first = register(server, root_token, david, f"{david}@example.com", SECRET_PASSWORD)
    second = register(server, root_token, erin, f"{erin}@example.com", SECRET_PASSWORD)
    assert first.status_code == 201, f"creation failed: {first.status_code} {first.content!r}"
    assert second.status_code == 201, f"creation failed: {second.status_code} {second.content!r}"
    erin_id = json.loads(second.content)["id"]

    response = server.request(
        "PATCH",
        f"{server.href('user')}/{erin_id}",
        token=root_token,
        json={"email": f"{david}@example.com".upper()},
    )

    assert response.status_code == 409, f"case variant accepted: {response.status_code} {response.content!r}"
    stored = json.loads(server.request("GET", f"{server.href('user')}/{erin_id}", token=root_token).content)
    assert stored["email"] == f"{erin}@example.com"


@scenario("SCN-USER-014-A")
@scenario("SCN-USER-014-B")
@scenario("SCN-USER-014-C")
def test_an_update_with_an_invalid_value_is_rejected(server: Sut, root_token: str) -> None:
    """An invalid username, email or undeclared role answers 422 and leaves the stored values unchanged."""
    name = f"refuse{next(_ACCOUNTS)}"
    created = register(server, root_token, name, f"{name}@example.com", SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"
    profile = json.loads(created.content)

    attempts = (
        {"username": f" {name}x"},
        {"username": f"{name} X"},
        {"email": "not-an-email"},
        {"email": f"{name} <{name}@example.com>"},
        {"role": "SUPERUSER"},
    )
    for body in attempts:
        response = server.request(
            "PATCH",
            f"{server.href('user')}/{profile['id']}",
            token=root_token,
            json=body,
        )
        assert response.status_code == 422, f"{body!r} was accepted: {response.status_code} {response.content!r}"

    stored = json.loads(server.request("GET", f"{server.href('user')}/{profile['id']}", token=root_token).content)
    assert stored["username"] == profile["username"]
    assert stored["email"] == profile["email"]
    assert stored["role"] == profile["role"]


@scenario("SCN-USER-015-A")
def test_a_deletion_removes_the_identity(server: Sut, root_token: str) -> None:
    """A deletion answers 204 and removes the account, its credential and its permission summary."""
    name = f"gone{next(_ACCOUNTS)}"
    target_id, target_token = provision(server, root_token, name)

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.content!r}"
    assert response.content == b""
    assert server.request("GET", f"{server.href('user')}/{target_id}", token=root_token).status_code == 404
    summary = server.request(
        "GET",
        f"{server.href('permission')}?user_id={target_id}",
        token=root_token,
    )
    assert summary.status_code == 404, f"permission summary survived: {summary.status_code}"
    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=SECRET_PASSWORD, identifier=name)
        ).status_code
        == 401
    )
    assert server.request("GET", server.href("current_user"), token=target_token).status_code == 401


@scenario("SCN-USER-016-A")
@scenario("SCN-USER-016-B")
@scenario("SCN-USER-016-C")
def test_a_deletion_removes_owned_data(server: Sut, root_token: str) -> None:
    """A deleted account's gallery with its item, playlist and unfinished upload session are gone."""
    name = f"owned{next(_ACCOUNTS)}"
    target_id, target_token = provision(server, root_token, name)
    file = upload(server, target_token, "owned.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    gallery = create_gallery(server, target_token, "Owned Gallery")
    item = add_to_gallery(server, target_token, gallery, file["id"])
    playlist = server.request(
        "POST",
        server.href("playlist"),
        token=target_token,
        json={"name": "Owned Playlist", "is_public": False},
    )
    assert playlist.status_code == 201, f"playlist failed: {playlist.status_code} {playlist.content!r}"
    playlist_body = json.loads(playlist.content)
    upload_session = begin_upload(server, target_token, "unfinished.png", IMAGE_BYTES)

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.content!r}"
    assert server.request("GET", gallery["_links"]["self"]["href"], token=root_token).status_code == 404
    assert server.request("GET", item["_links"]["self"]["href"], token=root_token).status_code == 404
    assert server.request("GET", playlist_body["_links"]["self"]["href"], token=root_token).status_code == 404
    assert server.request("GET", upload_session.self_href, token=root_token).status_code == 404


@scenario("SCN-USER-017-A")
def test_private_gallery_media_is_removed_with_its_bytes(server: Sut, root_token: str) -> None:
    """An image referenced only by the account's own gallery is deleted with its record and bytes."""
    name = f"private{next(_ACCOUNTS)}"
    target_id, target_token = provision(server, root_token, name)
    file = upload(server, target_token, "private.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    typed = json.loads(server.follow(file, "image", token=target_token).content)
    gallery = create_gallery(server, target_token, f"Private {name}")
    add_to_gallery(server, target_token, gallery, file["id"])
    assert server.exists(f"media/{file['path']}")

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.content!r}"
    assert server.request("GET", file["_links"]["self"]["href"], token=root_token).status_code == 404
    assert server.request("GET", typed["_links"]["self"]["href"], token=root_token).status_code == 404
    assert not server.exists(f"media/{file['path']}")


@scenario("SCN-USER-017-C")
def test_the_accounts_own_public_gallery_is_not_a_shared_reference(server: Sut, root_token: str) -> None:
    """An image referenced only by the account's own public gallery is deleted with the account."""
    name = f"pub{next(_ACCOUNTS)}"
    target_id, target_token = provision(server, root_token, name)
    file = upload(server, target_token, "public.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    typed = json.loads(server.follow(file, "image", token=target_token).content)
    gallery = create_gallery(server, target_token, f"Public {name}", is_public=True)
    add_to_gallery(server, target_token, gallery, file["id"])

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.content!r}"
    assert server.request("GET", file["_links"]["self"]["href"], token=root_token).status_code == 404
    assert server.request("GET", typed["_links"]["self"]["href"], token=root_token).status_code == 404
    assert not server.exists(f"media/{file['path']}")


@scenario("SCN-USER-018-A")
def test_an_image_in_another_gallery_survives_under_account_zero(server: Sut, root_token: str) -> None:
    """An image referenced by another account's gallery survives, owned by account 0."""
    name = f"shared{next(_ACCOUNTS)}"
    target_id, target_token = provision(server, root_token, name)
    file = upload(server, target_token, "shared.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    gallery = create_gallery(server, root_token, f"Shared {name}")
    item = add_to_gallery(server, root_token, gallery, file["id"])

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.content!r}"
    gallery_read = server.request("GET", gallery["_links"]["self"]["href"], token=root_token)
    assert gallery_read.status_code == 200, f"gallery failed: {gallery_read.status_code} {gallery_read.content!r}"
    assert item["id"] in [entry["id"] for entry in json.loads(gallery_read.content)["items"]]
    profile = json.loads(server.request("GET", file["_links"]["self"]["href"], token=root_token).content)
    assert profile["_links"]["owner"]["href"] == "/api/v1/user/0"
    content = server.request("GET", profile["_links"]["content"]["href"], token=root_token)
    assert content.status_code == 200
    assert content.content.startswith(b"\x89PNG")


@scenario("SCN-USER-018-B")
def test_a_video_backing_a_film_survives(server: Sut, root_token: str) -> None:
    """A video backing a film survives the deletion and the film stays readable."""
    name = f"film{next(_ACCOUNTS)}"
    target_id, target_token = provision(server, root_token, name)
    file = upload(server, target_token, "film.mp4", VIDEO_BYTES, VIDEO_CONTENT_TYPE)
    typed = json.loads(server.follow(file, "video", token=target_token).content)
    created = server.request(
        "POST",
        server.href("movie"),
        token=root_token,
        json={"name": f"Surviving {name}", "video_id": typed["id"]},
    )
    assert created.status_code == 201, f"movie failed: {created.status_code} {created.content!r}"
    movie_href = json.loads(created.content)["_links"]["self"]["href"]

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.content!r}"
    assert server.request("GET", movie_href, token=root_token).status_code == 200
    profile = json.loads(server.request("GET", file["_links"]["self"]["href"], token=root_token).content)
    assert profile["_links"]["owner"]["href"] == "/api/v1/user/0"
    content = server.request("GET", profile["_links"]["content"]["href"], token=root_token)
    assert content.status_code == 200
    assert content.content


@scenario("SCN-USER-018-C")
def test_catalogue_artwork_survives(server: Sut, root_token: str) -> None:
    """An image referenced as catalogue artwork stays readable through the catalogue resource."""
    name = f"art{next(_ACCOUNTS)}"
    target_id, target_token = provision(server, root_token, name)
    image_file = upload(server, target_token, "artwork.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    image = json.loads(server.follow(image_file, "image", token=target_token).content)
    video_file = upload(server, target_token, "artwork.mp4", VIDEO_BYTES, VIDEO_CONTENT_TYPE)
    video = json.loads(server.follow(video_file, "video", token=target_token).content)
    created = server.request(
        "POST",
        server.href("movie"),
        token=root_token,
        json={
            "name": f"Illustrated {name}",
            "video_id": video["id"],
            "cover_image_id": image["id"],
        },
    )
    assert created.status_code == 201, f"movie failed: {created.status_code} {created.content!r}"

    response = server.request("DELETE", f"{server.href('user')}/{target_id}", token=root_token)

    assert response.status_code == 204, f"delete failed: {response.status_code} {response.content!r}"
    artwork = server.request("GET", image["_links"]["self"]["href"], token=root_token)
    assert artwork.status_code == 200, f"artwork failed: {artwork.status_code} {artwork.content!r}"
    profile = json.loads(server.request("GET", image_file["_links"]["self"]["href"], token=root_token).content)
    assert profile["_links"]["owner"]["href"] == "/api/v1/user/0"


@scenario("SCN-USER-019-A")
def test_a_transferred_file_with_a_colliding_digest_stays_readable(server: Sut, root_token: str) -> None:
    """Two accounts' identical-content files both survive, so the second transfer cleared its digest."""
    alice = f"digest{next(_ACCOUNTS)}"
    bobby = f"digestb{next(_ACCOUNTS)}"
    alice_id, alice_token = provision(server, root_token, alice)
    bobby_id, bobby_token = provision(server, root_token, bobby)
    alice_file = upload(server, alice_token, "alice.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    bobby_file = upload(server, bobby_token, "bobby.png", IMAGE_BYTES, IMAGE_CONTENT_TYPE)
    assert alice_file["id"] != bobby_file["id"]
    gallery = create_gallery(server, root_token, f"Digests {alice}")
    add_to_gallery(server, root_token, gallery, alice_file["id"])
    add_to_gallery(server, root_token, gallery, bobby_file["id"])

    first = server.request("DELETE", f"{server.href('user')}/{alice_id}", token=root_token)
    second = server.request("DELETE", f"{server.href('user')}/{bobby_id}", token=root_token)

    assert first.status_code == 204, f"first delete failed: {first.status_code} {first.content!r}"
    assert second.status_code == 204, f"second delete failed: {second.status_code} {second.content!r}"
    for file in (alice_file, bobby_file):
        profile = json.loads(server.request("GET", file["_links"]["self"]["href"], token=root_token).content)
        assert profile["_links"]["owner"]["href"] == "/api/v1/user/0"
        content = server.request("GET", profile["_links"]["content"]["href"], token=root_token)
        assert content.status_code == 200
