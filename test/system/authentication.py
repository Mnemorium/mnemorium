"""Black-box proof of the `authentication` capability."""

import json
import time
from itertools import count

import allure
import pytest
from conftest import (
    INSTANCE_ENV,
    JWT_SECRET,
    attempt,
    limiter_config,
    login,
    mint_token,
    provision,
    start_instance,
)
from harness import (
    MSG_PASSWORD_HIDDEN,
    MSG_PASSWORD_LOGGED,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    WRONG_PASSWORD,
    Sut,
    register,
)
from mnemorium_client.api.identity import patch_credential, post_login, post_register, post_token
from mnemorium_client.api.user import get_me
from mnemorium_client.models import LoginRequest, PatchCredentialRequest, RegisterRequest, Role

PASSWORD_SYMBOLS = frozenset("!@#$%^&*()-_=+")

_ACCOUNTS = count()


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-001")
@allure.story("SCN-AUTHENTICATION-001-A")
def test_first_start_creates_the_root_admin(server: Sut, root_token: str) -> None:
    """The instance's Root Admin is user 0, `root`, Admin and no email."""
    response = get_me.sync_detailed(client=server.client(root_token))

    assert response.status_code == 200
    body = json.loads(response.content)
    assert body["id"] == 0
    assert body["username"] == ROOT_ADMIN_USERNAME
    assert body["role"] == "ADMIN"
    assert body["email"] is None


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-001")
@allure.story("SCN-AUTHENTICATION-001-B")
@pytest.mark.own_instance(reason="restart")
def test_restart_does_not_recreate_or_reveal_the_root_admin(sut: Sut) -> None:
    """An initialized directory restarts healthy, creates no second account, and reveals nothing."""
    server = start_instance(sut)
    password = server.root_password()

    server.restart(env=INSTANCE_ENV)
    token = login(server, ROOT_ADMIN_USERNAME, password)

    assert server.request("GET", server.href("health"), token=token).status_code == 200
    assert MSG_PASSWORD_LOGGED.search(server.logs()) is None


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-002")
@allure.story("SCN-AUTHENTICATION-002-A")
def test_generated_password_follows_the_declared_shape(server: Sut) -> None:
    """The revealed default password is 20 characters: 19 alphanumeric and one declared symbol."""
    password = server.root_password()

    assert len(password) == 20
    assert sum(character.isascii() and character.isalnum() for character in password) == 19
    assert sum(character in PASSWORD_SYMBOLS for character in password) == 1


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-003")
@allure.story("SCN-AUTHENTICATION-003-A")
@pytest.mark.own_instance(reason="log_sink")
def test_the_password_is_printed_once_and_never_logged(sut: Sut) -> None:
    """The reveal prints the password once to standard output and no log file holds it."""
    server = start_instance(sut)
    password = server.root_password()
    stdout = server.logs()

    assert login(server, ROOT_ADMIN_USERNAME, password)

    server.stop()
    assert stdout.count(password) == 1
    assert password not in server.log_text()


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-003")
@allure.story("SCN-AUTHENTICATION-003-B")
@pytest.mark.own_instance(reason="first_boot")
def test_a_first_start_without_the_reveal_is_refused(sut: Sut) -> None:
    """A first start with the reveal disabled fails, names the setting, and creates no account."""
    failure = sut.start_expecting_failure(
        env=INSTANCE_ENV,
        config={"security": {"log_root_admin_password": False}},
    )

    assert "security.log_root_admin_password" in failure.output
    assert MSG_PASSWORD_LOGGED.search(failure.output) is None

    sut.stop()
    server = start_instance(sut, config={"security": {"log_root_admin_password": True}})

    assert login(server, ROOT_ADMIN_USERNAME, server.root_password())


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-003")
@allure.story("SCN-AUTHENTICATION-003-C")
@pytest.mark.own_instance(reason="restart")
def test_disabling_the_reveal_after_initialization_still_starts(sut: Sut) -> None:
    """An initialized directory starts with the reveal disabled, prints the notice, and reveals nothing."""
    server = start_instance(sut)
    password = server.root_password()

    server.restart(
        env=INSTANCE_ENV,
        config={"security": {"log_root_admin_password": False}},
    )
    token = login(server, ROOT_ADMIN_USERNAME, password)

    assert server.request("GET", server.href("health"), token=token).status_code == 200
    stdout = server.logs()
    assert MSG_PASSWORD_HIDDEN in stdout
    assert MSG_PASSWORD_LOGGED.search(stdout) is None


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-004")
@allure.story("SCN-AUTHENTICATION-004-A")
@allure.story("SCN-AUTHENTICATION-004-B")
@allure.story("SCN-AUTHENTICATION-004-C")
def test_a_login_issues_a_usable_bearer_token(server: Sut) -> None:
    """A valid login carries the token fields, resolves its user, and never carrying the password."""
    password = server.root_password()

    response = post_login.sync_detailed(
        client=server.client(), body=LoginRequest(password=password, identifier=ROOT_ADMIN_USERNAME)
    )

    assert response.status_code == 200
    body = json.loads(response.content)
    assert {"access_token", "expires_in", "token_type"} <= set(body)
    assert body["token_type"] == "bearer"
    assert password not in response.content.decode()
    assert not any("password" in field or "hash" in field for field in body)

    used = get_me.sync_detailed(client=server.client(body["access_token"]))

    assert used.status_code == 200
    profile = json.loads(used.content)
    assert profile["id"] == 0
    assert profile["username"] == ROOT_ADMIN_USERNAME
    assert profile["role"] == "ADMIN"


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-005")
@allure.story("SCN-AUTHENTICATION-005-A")
def test_bad_credentials_are_rejected_uniformly(server: Sut) -> None:
    """A wrong password and an unknown username answer the same 401, body and challenge."""
    wrong_password = post_login.sync_detailed(
        client=server.client(), body=LoginRequest(password=WRONG_PASSWORD, identifier=ROOT_ADMIN_USERNAME)
    )
    unknown_username = post_login.sync_detailed(
        client=server.client(),
        body=LoginRequest(
            password=WRONG_PASSWORD,
            identifier=f"no-such-account{next(_ACCOUNTS)}",
        ),
    )

    assert wrong_password.status_code == unknown_username.status_code == 401
    assert json.loads(wrong_password.content) == json.loads(unknown_username.content)
    assert (
        wrong_password.headers.get("WWW-Authenticate") == unknown_username.headers.get("WWW-Authenticate") == "Bearer"
    )


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-004")
@allure.story("SCN-AUTHENTICATION-004-D")
def test_a_login_by_email_address_succeeds(server: Sut, root_token: str) -> None:
    """The email address stored on an account authenticates it exactly as its username does."""
    name = f"mail{next(_ACCOUNTS)}"
    email = f"{name}@example.com"
    created = register(server, root_token, name, email, SECRET_PASSWORD)
    assert created.status_code == 201, f"creation failed: {created.status_code} {created.content!r}"

    by_email = post_login.sync_detailed(
        client=server.client(), body=LoginRequest(identifier=email, password=SECRET_PASSWORD)
    )
    by_username = post_login.sync_detailed(
        client=server.client(), body=LoginRequest(identifier=name, password=SECRET_PASSWORD)
    )

    assert by_email.status_code == by_username.status_code == 200
    assert set(json.loads(by_email.content)) == set(json.loads(by_username.content))


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-004")
@allure.story("SCN-AUTHENTICATION-004-A")
def test_a_username_wins_over_another_accounts_email(server: Sut, root_token: str) -> None:
    """A value that is one account's username resolves as that username, never as another's email."""
    suffix = next(_ACCOUNTS)
    shared = f"collision{suffix}@example.com"
    username_password = "Alice!Pass"
    email_password = "Bobby!Pass"
    holder = register(server, root_token, shared, None, username_password)
    other = register(server, root_token, f"holder{suffix}", shared, email_password)
    assert holder.status_code == 201, f"creation failed: {holder.status_code} {holder.content!r}"
    assert other.status_code == 201, f"creation failed: {other.status_code} {other.content!r}"

    as_username = post_login.sync_detailed(
        client=server.client(), body=LoginRequest(identifier=shared, password=username_password)
    )
    as_email = post_login.sync_detailed(
        client=server.client(), body=LoginRequest(identifier=shared, password=email_password)
    )

    assert as_username.status_code == 200, f"username password refused: {as_username.status_code}"
    assert as_email.status_code == 401, "another account's email must not resolve the value"
    profile = get_me.sync_detailed(client=server.client(json.loads(as_username.content)["access_token"]))
    assert json.loads(profile.content)["id"] == json.loads(holder.content)["id"]


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-A")
@pytest.mark.own_instance(reason="process_state")
def test_a_login_burst_is_throttled(sut: Sut) -> None:
    """Exceeding one address's burst answers 429 with Retry-After."""
    server = start_instance(sut, config=limiter_config())

    assert attempt(server, f"ghost-a{next(_ACCOUNTS)}").status_code == 401
    throttled = attempt(server, f"ghost-b{next(_ACCOUNTS)}")

    assert throttled.status_code == 429
    assert "Retry-After" in throttled.headers


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-B")
@pytest.mark.own_instance(reason="process_state")
def test_the_login_quota_refills(sut: Sut) -> None:
    """After the configured period a throttled address is admitted again."""
    config = limiter_config()
    config["security"]["rate_limit"]["period_seconds"] = 1
    server = start_instance(sut, config=config)
    assert attempt(server, f"ghost-a{next(_ACCOUNTS)}").status_code == 401
    assert attempt(server, f"ghost-b{next(_ACCOUNTS)}").status_code == 429

    time.sleep(2)

    assert attempt(server, f"ghost-c{next(_ACCOUNTS)}").status_code == 401


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-C")
@pytest.mark.own_instance(reason="process_state")
@pytest.mark.slow
def test_a_client_address_header_cannot_mint_a_new_quota(sut: Sut) -> None:
    """A trusted proxy's header keys the bucket; an untrusted peer's header is ignored."""
    server = start_instance(sut)
    peer = server.peer_address()

    server.restart(env=INSTANCE_ENV, config=limiter_config(trusted=peer))

    assert attempt(server, f"ghost-a{next(_ACCOUNTS)}", "203.0.113.1").status_code == 401
    assert attempt(server, f"ghost-b{next(_ACCOUNTS)}", "203.0.113.1").status_code == 429
    assert attempt(server, f"ghost-c{next(_ACCOUNTS)}", "203.0.113.2").status_code == 401

    server.restart(env=INSTANCE_ENV, config=limiter_config())

    assert attempt(server, f"ghost-d{next(_ACCOUNTS)}", "203.0.113.3").status_code == 401
    assert attempt(server, f"ghost-e{next(_ACCOUNTS)}", "203.0.113.4").status_code == 429


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-D")
@pytest.mark.own_instance(reason="process_state")
@pytest.mark.slow
def test_one_account_is_throttled_across_addresses(sut: Sut) -> None:
    """One account's quota is spent across addresses while each address keeps its own."""
    server = start_instance(sut)
    peer = server.peer_address()
    server.restart(env=INSTANCE_ENV, config=limiter_config(trusted=peer))

    assert attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.1").status_code == 401
    assert attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.2").status_code == 429
    assert attempt(server, f"ghost-d{next(_ACCOUNTS)}", "203.0.113.3").status_code == 401


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-E")
@pytest.mark.own_instance(reason="process_state")
@pytest.mark.slow
def test_an_unknown_username_is_throttled_identically(sut: Sut) -> None:
    """An unknown username is throttled exactly like an account that exists."""
    server = start_instance(sut)
    peer = server.peer_address()
    server.restart(env=INSTANCE_ENV, config=limiter_config(trusted=peer))
    ghost = f"ghost-a{next(_ACCOUNTS)}"

    assert attempt(server, ghost, "203.0.113.1").status_code == 401
    ghost_throttled = attempt(server, ghost, "203.0.113.2")
    assert attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.3").status_code == 401
    root_throttled = attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.4")

    assert ghost_throttled.status_code == root_throttled.status_code == 429
    assert json.loads(ghost_throttled.content) == json.loads(root_throttled.content)
    assert "Retry-After" in ghost_throttled.headers
    assert "Retry-After" in root_throttled.headers


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-F")
@pytest.mark.own_instance(reason="process_state")
@pytest.mark.slow
def test_an_unknown_email_is_throttled_identically(sut: Sut) -> None:
    """An unknown email address is throttled exactly like an identifier that exists."""
    server = start_instance(sut)
    peer = server.peer_address()
    server.restart(env=INSTANCE_ENV, config=limiter_config(trusted=peer))
    ghost = f"ghost{next(_ACCOUNTS)}@example.com"

    assert attempt(server, ghost, "203.0.113.1").status_code == 401
    ghost_throttled = attempt(server, ghost, "203.0.113.2")
    assert attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.3").status_code == 401
    root_throttled = attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.4")

    assert ghost_throttled.status_code == root_throttled.status_code == 429
    assert json.loads(ghost_throttled.content) == json.loads(root_throttled.content)
    assert "Retry-After" in ghost_throttled.headers


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-010")
@allure.story("SCN-AUTHENTICATION-010-B")
@allure.story("SCN-AUTHENTICATION-010-C")
@allure.story("SCN-AUTHENTICATION-010-D")
def test_invalid_tokens_are_rejected(server: Sut) -> None:
    """A tampered signature, an expired token and a non-numeric subject each answer 401."""
    rejected = {
        "a tampered signature": mint_token(JWT_SECRET, "0", tampered=True),
        "an expired token": mint_token(JWT_SECRET, "0", expires_in=-1),
        "a non-numeric subject": mint_token(JWT_SECRET, ROOT_ADMIN_USERNAME),
    }

    for reason, token in rejected.items():
        response = get_me.sync_detailed(client=server.client(token))
        assert response.status_code == 401, f"{reason} was accepted: {response.status_code} {response.content!r}"


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-012")
@allure.feature("REQ-AUTHENTICATION-013")
@allure.story("SCN-AUTHENTICATION-012-A")
@allure.story("SCN-AUTHENTICATION-013-C")
@pytest.mark.own_instance(reason="process_state")
def test_rotation_and_regeneration_leave_issued_tokens_valid(sut: Sut) -> None:
    """A token issued before a credential rotation or a regeneration still authenticates afterwards."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    rotated = patch_credential.sync_detailed(
        1,
        client=server.client(token),
        body=PatchCredentialRequest(password="N3w!Secret"),
    )
    assert rotated.status_code == 204

    regenerated = post_token.sync_detailed(client=server.client(token))
    assert regenerated.status_code == 200

    response = get_me.sync_detailed(client=server.client(token))

    assert response.status_code == 200
    assert json.loads(response.content)["id"] == 0


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-013")
@allure.story("SCN-AUTHENTICATION-013-A")
@allure.story("SCN-AUTHENTICATION-013-B")
def test_regenerating_a_token_keeps_the_caller(server: Sut, root_token: str) -> None:
    """Regeneration returns the login response shape and a token for the same user."""
    owner_id, owner = provision(server, root_token, f"tokenregen{next(_ACCOUNTS)}")

    response = post_token.sync_detailed(client=server.client(owner))

    assert response.status_code == 200
    body = json.loads(response.content)
    assert {"access_token", "expires_in", "token_type"} <= set(body)
    assert body["token_type"] == "bearer"

    regenerated = get_me.sync_detailed(client=server.client(body["access_token"]))

    assert regenerated.status_code == 200
    assert json.loads(regenerated.content)["id"] == owner_id


# A credential is identified by its own sequence: the Root Admin's is `1`, and on a fresh instance a created account's
# is `user_id + 1`. Only that first inference holds on the shared instance, which is why the rotation cases keep their
# own containers.
@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-014")
@allure.story("SCN-AUTHENTICATION-014-A")
@pytest.mark.own_instance(reason="process_state")
def test_the_root_admin_rotates_its_own_credential(sut: Sut) -> None:
    """Rotating the Root Admin's own credential replaces the password it authenticates with."""
    server = start_instance(sut)
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    new_password = "R00t!Rotated"

    rotated = patch_credential.sync_detailed(
        1,
        client=server.client(token),
        body=PatchCredentialRequest(password=new_password),
    )

    assert rotated.status_code == 204
    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=password, identifier=ROOT_ADMIN_USERNAME)
        ).status_code
        == 401
    )
    assert login(server, ROOT_ADMIN_USERNAME, new_password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-014")
@allure.story("SCN-AUTHENTICATION-014-B")
@pytest.mark.own_instance(reason="process_state")
def test_the_root_admin_rotates_another_users_credential(sut: Sut) -> None:
    """Rotating another user's credential makes that user's new password authenticate."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    username = f"alice{next(_ACCOUNTS)}"
    registered = post_register.sync_detailed(
        client=server.client(token),
        body=RegisterRequest(password="Alice!Pass", role=Role.STANDARD, username=username),
    )
    user_id = json.loads(registered.content)["id"]
    new_password = "NewAlice!Pass"

    rotated = patch_credential.sync_detailed(
        user_id + 1,
        client=server.client(token),
        body=PatchCredentialRequest(password=new_password),
    )

    assert rotated.status_code == 204
    assert login(server, username, new_password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-014")
@allure.story("SCN-AUTHENTICATION-014-D")
@pytest.mark.own_instance(reason="process_state")
def test_a_non_root_caller_cannot_rotate_a_credential(sut: Sut) -> None:
    """An Admin and a Standard User are refused with 403 and their passwords stay unchanged."""
    server = start_instance(sut)
    root_token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    standard_name = f"alice{next(_ACCOUNTS)}"
    admin_name = f"bobby{next(_ACCOUNTS)}"
    standard = post_register.sync_detailed(
        client=server.client(root_token),
        body=RegisterRequest(password="Alice!Pass", role=Role.STANDARD, username=standard_name),
    )
    admin = post_register.sync_detailed(
        client=server.client(root_token),
        body=RegisterRequest(password="Bobby!Pass", role=Role.ADMIN, username=admin_name),
    )
    standard_id = json.loads(standard.content)["id"]
    admin_id = json.loads(admin.content)["id"]

    for username, password, user_id in (
        (standard_name, "Alice!Pass", standard_id),
        (admin_name, "Bobby!Pass", admin_id),
    ):
        token = login(server, username, password)
        refused = patch_credential.sync_detailed(
            user_id + 1,
            client=server.client(token),
            body=PatchCredentialRequest(password="N0!Allowed"),
        )

        assert refused.status_code == 403
        assert login(server, username, password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-015")
@allure.story("SCN-AUTHENTICATION-015-A")
def test_a_rotation_that_fails_the_policy_changes_nothing(server: Sut, root_token: str) -> None:
    """A short or symbol-free password answers 422 and the old password still authenticates."""
    password = server.root_password()

    for rejected in ("xY3!z9w", "password123"):
        response = patch_credential.sync_detailed(
            1,
            client=server.client(root_token),
            body=PatchCredentialRequest(password=rejected),
        )
        assert response.status_code == 422, f"{rejected!r} was accepted: {response.content!r}"

    assert login(server, ROOT_ADMIN_USERNAME, password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-018")
@allure.story("SCN-AUTHENTICATION-018-A")
@pytest.mark.own_instance(reason="process_state")
def test_a_compliant_password_is_accepted_at_rotation(sut: Sut) -> None:
    """A compliant password replaces the Root Admin's credential and authenticates afterwards."""
    server = start_instance(sut)
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    compliant = "C0mpliant!Pass"

    rotated = patch_credential.sync_detailed(
        1,
        client=server.client(token),
        body=PatchCredentialRequest(password=compliant),
    )

    assert rotated.status_code == 204
    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=password, identifier=ROOT_ADMIN_USERNAME)
        ).status_code
        == 401
    )
    assert login(server, ROOT_ADMIN_USERNAME, compliant)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-018")
@allure.story("SCN-AUTHENTICATION-018-A")
def test_a_compliant_password_is_accepted_at_creation(server: Sut, root_token: str) -> None:
    """A compliant password creates an account that authenticates with it."""
    username = f"policy{next(_ACCOUNTS)}"

    created = post_register.sync_detailed(
        client=server.client(root_token),
        body=RegisterRequest(password=SECRET_PASSWORD, role=Role.STANDARD, username=username),
    )

    assert created.status_code == 201
    assert login(server, username, SECRET_PASSWORD)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-018")
@allure.story("SCN-AUTHENTICATION-018-B")
@allure.story("SCN-AUTHENTICATION-018-C")
def test_a_short_or_symbol_free_password_is_rejected_at_creation(server: Sut, root_token: str) -> None:
    """A short and a symbol-free password each answer 422 and store no account."""
    for rejected in ("short1!", "longpassword"):
        username = f"weak{next(_ACCOUNTS)}"

        refused = post_register.sync_detailed(
            client=server.client(root_token),
            body=RegisterRequest(password=rejected, role=Role.STANDARD, username=username),
        )
        assert refused.status_code == 422, f"{username} was created with {rejected!r}"

        created = post_register.sync_detailed(
            client=server.client(root_token),
            body=RegisterRequest(password=SECRET_PASSWORD, role=Role.STANDARD, username=username),
        )
        assert created.status_code == 201, "the rejected creation must not have stored the user"
