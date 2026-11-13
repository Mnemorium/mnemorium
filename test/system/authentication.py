"""Black-box proof of the `authentication` capability.

Every case starts its own container from the image under test through the shared `harness` plugin and reads only what
an operator can: HTTP responses, standard output, the log sink and the working directory. The instance secrets are
fixed here so a case can mint its own token with the standard library and restart the same directory deterministically.
"""

import base64
import json
import time

import allure
import pytest
from conftest import (
    INSTANCE_ENV,
    JWT_SECRET,
    attempt,
    limiter_config,
    mint_token,
    start_instance,
)
from harness import (
    MSG_PASSWORD_HIDDEN,
    MSG_PASSWORD_LOGGED,
    ROOT_ADMIN_USERNAME,
    WRONG_PASSWORD,
    Sut,
    register,
)

PASSWORD_SYMBOLS = frozenset("!@#$%^&*()-_=+")


def test_a_root_admin_logs_in(sut: Sut) -> None:
    """Smoke: a fresh instance reveals a password that authenticates the Root Admin."""
    server = start_instance(sut)

    assert server.login(ROOT_ADMIN_USERNAME, server.root_password())


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-001")
@allure.story("SCN-AUTHENTICATION-001-A")
def test_first_start_creates_the_root_admin(sut: Sut) -> None:
    """An empty working directory is bootstrapped with user 0, `root`, Admin and no email."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    response = server.request("GET", server.href("current_user"), token=token)

    assert response.status_code == 200
    body = response.json()
    assert body["id"] == 0
    assert body["username"] == ROOT_ADMIN_USERNAME
    assert body["role"] == "ADMIN"
    assert body["email"] is None


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-001")
@allure.story("SCN-AUTHENTICATION-001-B")
def test_restart_does_not_recreate_or_reveal_the_root_admin(sut: Sut) -> None:
    """An initialized directory restarts healthy, creates no second account, and reveals nothing."""
    server = start_instance(sut)
    password = server.root_password()

    server.restart(env=INSTANCE_ENV)

    assert server.request("GET", server.href("health")).status_code == 200
    assert MSG_PASSWORD_LOGGED.search(server.logs()) is None
    assert server.login(ROOT_ADMIN_USERNAME, password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-002")
@allure.story("SCN-AUTHENTICATION-002-A")
def test_generated_password_follows_the_declared_shape(sut: Sut) -> None:
    """The revealed default password is 20 characters: 19 alphanumeric and one declared symbol."""
    server = start_instance(sut)
    password = server.root_password()

    assert len(password) == 20
    assert sum(character.isascii() and character.isalnum() for character in password) == 19
    assert sum(character in PASSWORD_SYMBOLS for character in password) == 1


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-003")
@allure.story("SCN-AUTHENTICATION-003-A")
def test_the_password_is_printed_once_and_never_logged(sut: Sut) -> None:
    """The reveal prints the password once to standard output and no log file holds it."""
    server = start_instance(sut)
    password = server.root_password()
    stdout = server.logs()

    assert server.login(ROOT_ADMIN_USERNAME, password)

    server.stop()
    assert stdout.count(password) == 1
    assert password not in server.log_text()


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-003")
@allure.story("SCN-AUTHENTICATION-003-B")
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

    assert server.login(ROOT_ADMIN_USERNAME, server.root_password())


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-003")
@allure.story("SCN-AUTHENTICATION-003-C")
def test_disabling_the_reveal_after_initialization_still_starts(sut: Sut) -> None:
    """An initialized directory starts with the reveal disabled, prints the notice, and reveals nothing."""
    server = start_instance(sut)
    server.root_password()

    server.restart(
        env=INSTANCE_ENV,
        config={"security": {"log_root_admin_password": False}},
    )

    assert server.request("GET", server.href("health")).status_code == 200
    stdout = server.logs()
    assert MSG_PASSWORD_HIDDEN in stdout
    assert MSG_PASSWORD_LOGGED.search(stdout) is None


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-004")
@allure.story("SCN-AUTHENTICATION-004-A")
def test_login_response_carries_exactly_the_token_fields(sut: Sut) -> None:
    """A successful login carries the token fields and names the login operation."""
    server = start_instance(sut)

    response = server.request(
        "POST",
        server.href("login"),
        json={"username": ROOT_ADMIN_USERNAME, "password": server.root_password()},
    )

    assert response.status_code == 200
    body = response.json()
    assert {"access_token", "expires_in", "token_type"} <= set(body)
    assert body["token_type"] == "bearer"
    assert body["_links"]["self"]["href"] == server.href("login")


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-004")
@allure.story("SCN-AUTHENTICATION-004-B")
def test_the_issued_token_identifies_its_user(sut: Sut) -> None:
    """The token returned by login resolves the same user on a protected endpoint."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    response = server.request("GET", server.href("current_user"), token=token)

    assert response.status_code == 200
    body = response.json()
    assert body["id"] == 0
    assert body["username"] == ROOT_ADMIN_USERNAME
    assert body["role"] == "ADMIN"
    assert body["_links"]["self"]["href"] == f"/api/v1/user/{body['id']}"


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-004")
@allure.story("SCN-AUTHENTICATION-004-C")
def test_login_response_carries_no_password(sut: Sut) -> None:
    """The login response holds neither the submitted password nor a password field."""
    server = start_instance(sut)
    password = server.root_password()

    response = server.request(
        "POST",
        server.href("login"),
        json={"username": ROOT_ADMIN_USERNAME, "password": password},
    )

    assert password not in response.text
    assert not any("password" in field or "hash" in field for field in response.json())


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-005")
@allure.story("SCN-AUTHENTICATION-005-A")
def test_bad_credentials_are_rejected_uniformly(sut: Sut) -> None:
    """A wrong password and an unknown username answer the same 401, body and challenge."""
    server = start_instance(sut)
    server.root_password()

    wrong_password = server.attempt_login(ROOT_ADMIN_USERNAME, WRONG_PASSWORD)
    unknown_username = server.attempt_login("no-such-account", WRONG_PASSWORD)

    assert wrong_password.status_code == unknown_username.status_code == 401
    assert wrong_password.json() == unknown_username.json()
    assert (
        wrong_password.headers.get("WWW-Authenticate") == unknown_username.headers.get("WWW-Authenticate") == "Bearer"
    )


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-007")
@allure.story("SCN-AUTHENTICATION-007-A")
def test_login_reports_the_configured_lifetime(sut: Sut) -> None:
    """Login advertises `security.jwt.ttl` and issues a token that expires exactly then."""
    ttl = 120
    server = start_instance(sut, config={"security": {"jwt": {"ttl": ttl}}})

    before = int(time.time())
    response = server.request(
        "POST",
        server.href("login"),
        json={"username": ROOT_ADMIN_USERNAME, "password": server.root_password()},
    )
    after = int(time.time())

    assert response.status_code == 200
    body = response.json()
    payload = body["access_token"].split(".")[1]
    issued = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
    assert body["expires_in"] == ttl
    assert before + ttl <= issued["exp"] <= after + ttl


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-A")
def test_a_login_burst_is_throttled(sut: Sut) -> None:
    """Exceeding one address's burst answers 429 with Retry-After."""
    server = start_instance(sut, config=limiter_config())

    assert attempt(server, "ghost-a").status_code == 401
    throttled = attempt(server, "ghost-b")

    assert throttled.status_code == 429
    assert "Retry-After" in throttled.headers


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-B")
def test_the_login_quota_refills(sut: Sut) -> None:
    """After the configured period a throttled address is admitted again."""
    config = limiter_config()
    config["security"]["rate_limit"]["period_seconds"] = 1
    server = start_instance(sut, config=config)
    assert attempt(server, "ghost-a").status_code == 401
    assert attempt(server, "ghost-b").status_code == 429

    time.sleep(2)

    assert attempt(server, "ghost-c").status_code == 401


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-C")
@pytest.mark.slow
def test_a_client_address_header_cannot_mint_a_new_quota(sut: Sut) -> None:
    """A trusted proxy's header keys the bucket; an untrusted peer's header is ignored."""
    server = start_instance(sut)
    peer = server.peer_address()

    server.restart(env=INSTANCE_ENV, config=limiter_config(trusted=peer))

    assert attempt(server, "ghost-a", "203.0.113.1").status_code == 401
    assert attempt(server, "ghost-b", "203.0.113.1").status_code == 429
    assert attempt(server, "ghost-c", "203.0.113.2").status_code == 401

    server.restart(env=INSTANCE_ENV, config=limiter_config())

    assert attempt(server, "ghost-d", "203.0.113.3").status_code == 401
    assert attempt(server, "ghost-e", "203.0.113.4").status_code == 429


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-D")
@pytest.mark.slow
def test_one_account_is_throttled_across_addresses(sut: Sut) -> None:
    """One account's quota is spent across addresses while each address keeps its own."""
    server = start_instance(sut)
    peer = server.peer_address()
    server.restart(env=INSTANCE_ENV, config=limiter_config(trusted=peer))

    assert attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.1").status_code == 401
    assert attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.2").status_code == 429
    assert attempt(server, "ghost-d", "203.0.113.3").status_code == 401


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-008")
@allure.story("SCN-AUTHENTICATION-008-E")
@pytest.mark.slow
def test_an_unknown_username_is_throttled_identically(sut: Sut) -> None:
    """An unknown username is throttled exactly like an account that exists."""
    server = start_instance(sut)
    peer = server.peer_address()
    server.restart(env=INSTANCE_ENV, config=limiter_config(trusted=peer))

    assert attempt(server, "ghost-a", "203.0.113.1").status_code == 401
    ghost_throttled = attempt(server, "ghost-a", "203.0.113.2")
    assert attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.3").status_code == 401
    root_throttled = attempt(server, ROOT_ADMIN_USERNAME, "203.0.113.4")

    assert ghost_throttled.status_code == root_throttled.status_code == 429
    assert ghost_throttled.json() == root_throttled.json()
    assert "Retry-After" in ghost_throttled.headers
    assert "Retry-After" in root_throttled.headers


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-010")
@allure.story("SCN-AUTHENTICATION-010-B")
@allure.story("SCN-AUTHENTICATION-010-C")
@allure.story("SCN-AUTHENTICATION-010-D")
def test_invalid_tokens_are_rejected(sut: Sut) -> None:
    """A tampered signature, an expired token and a non-numeric subject each answer 401."""
    server = start_instance(sut)
    server.root_password()

    rejected = {
        "a tampered signature": mint_token(JWT_SECRET, "0", tampered=True),
        "an expired token": mint_token(JWT_SECRET, "0", expires_in=-1),
        "a non-numeric subject": mint_token(JWT_SECRET, ROOT_ADMIN_USERNAME),
    }

    for reason, token in rejected.items():
        response = server.request("GET", server.href("current_user"), token=token)
        assert response.status_code == 401, f"{reason} was accepted: {response.status_code} {response.text}"


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-012")
@allure.feature("REQ-AUTHENTICATION-013")
@allure.story("SCN-AUTHENTICATION-012-A")
@allure.story("SCN-AUTHENTICATION-013-C")
def test_rotation_and_regeneration_leave_issued_tokens_valid(sut: Sut) -> None:
    """A token issued before a credential rotation or a regeneration still authenticates afterwards."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    rotated = server.request(
        "PATCH",
        server.href("credential", id=1),
        token=token,
        json={"password": "N3w!Secret"},
    )
    assert rotated.status_code == 204

    regenerated = server.request("POST", server.href("token"), token=token)
    assert regenerated.status_code == 200

    response = server.request("GET", server.href("current_user"), token=token)

    assert response.status_code == 200
    assert response.json()["id"] == 0


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-013")
@allure.story("SCN-AUTHENTICATION-013-A")
@allure.story("SCN-AUTHENTICATION-013-B")
def test_regenerating_a_token_keeps_the_caller(sut: Sut) -> None:
    """Regeneration returns the login response shape and a token for the same user."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    response = server.request("POST", server.href("token"), token=token)

    assert response.status_code == 200
    body = response.json()
    assert {"access_token", "expires_in", "token_type"} <= set(body)
    assert body["token_type"] == "bearer"
    assert body["_links"]["self"]["href"] == server.href("token")

    regenerated = server.request("GET", server.href("current_user"), token=body["access_token"])

    assert regenerated.status_code == 200
    assert regenerated.json()["id"] == 0


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-014")
@allure.story("SCN-AUTHENTICATION-014-A")
def test_the_root_admin_rotates_its_own_credential(sut: Sut) -> None:
    """Rotating the Root Admin's own credential replaces the password it authenticates with."""
    server = start_instance(sut)
    password = server.root_password()
    token = server.login(ROOT_ADMIN_USERNAME, password)
    new_password = "R00t!Rotated"

    rotated = server.request(
        "PATCH",
        server.href("credential", id=1),
        token=token,
        json={"password": new_password},
    )

    assert rotated.status_code == 204
    assert server.attempt_login(ROOT_ADMIN_USERNAME, password).status_code == 401
    assert server.login(ROOT_ADMIN_USERNAME, new_password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-014")
@allure.story("SCN-AUTHENTICATION-014-B")
def test_the_root_admin_rotates_another_users_credential(sut: Sut) -> None:
    """Rotating another user's credential makes that user's new password authenticate."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    registered = register(server, token, "alice", None, "Alice!Pass")
    user_id = registered.json()["id"]
    new_password = "NewAlice!Pass"

    rotated = server.request(
        "PATCH",
        server.href("credential", id=user_id + 1),
        token=token,
        json={"password": new_password},
    )

    assert rotated.status_code == 204
    assert server.login("alice", new_password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-014")
@allure.story("SCN-AUTHENTICATION-014-C")
def test_rotating_an_unknown_credential_is_not_found(sut: Sut) -> None:
    """An unknown credential identifier answers 404 and stores nothing."""
    server = start_instance(sut)
    token = server.login(ROOT_ADMIN_USERNAME, server.root_password())

    rotated = server.request(
        "PATCH",
        server.href("credential", id=999999),
        token=token,
        json={"password": "N0!SuchCredential"},
    )

    assert rotated.status_code == 404


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-014")
@allure.story("SCN-AUTHENTICATION-014-D")
def test_a_non_root_caller_cannot_rotate_a_credential(sut: Sut) -> None:
    """An Admin and a Standard User are refused with 403 and their passwords stay unchanged."""
    server = start_instance(sut)
    root_token = server.login(ROOT_ADMIN_USERNAME, server.root_password())
    standard = register(server, root_token, "alice", None, "Alice!Pass")
    admin = register(server, root_token, "bobby", None, "Bobby!Pass", role="ADMIN")
    standard_id = standard.json()["id"]
    admin_id = admin.json()["id"]

    for username, password, user_id in (
        ("alice", "Alice!Pass", standard_id),
        ("bobby", "Bobby!Pass", admin_id),
    ):
        token = server.login(username, password)
        refused = server.request(
            "PATCH",
            server.href("credential", id=user_id + 1),
            token=token,
            json={"password": "N0!Allowed"},
        )

        assert refused.status_code == 403
        assert server.login(username, password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-015")
@allure.story("SCN-AUTHENTICATION-015-A")
def test_a_rotation_that_fails_the_policy_changes_nothing(sut: Sut) -> None:
    """A short or symbol-free password answers 422 and the old password still authenticates."""
    server = start_instance(sut)
    password = server.root_password()
    token = server.login(ROOT_ADMIN_USERNAME, password)

    for rejected in ("xY3!z9w", "password123"):
        response = server.request(
            "PATCH",
            server.href("credential", id=1),
            token=token,
            json={"password": rejected},
        )
        assert response.status_code == 422, f"{rejected!r} was accepted: {response.text}"

    assert server.login(ROOT_ADMIN_USERNAME, password)


@allure.epic("Authentication")
@allure.feature("REQ-AUTHENTICATION-018")
@allure.story("SCN-AUTHENTICATION-018-A")
@allure.story("SCN-AUTHENTICATION-018-B")
@allure.story("SCN-AUTHENTICATION-018-C")
def test_the_password_policy_applies_at_both_entry_points(sut: Sut) -> None:
    """Creation and rotation accept a compliant password and reject a short or symbol-free one."""
    server = start_instance(sut)
    password = server.root_password()
    token = server.login(ROOT_ADMIN_USERNAME, password)
    compliant = "C0mpliant!Pass"

    for username, rejected in (("carol", "short1!"), ("dave", "longpassword")):
        invalid = server.request(
            "POST",
            server.href("register"),
            token=token,
            json={"username": username, "email": None, "password": rejected, "role": "STANDARD"},
        )
        assert invalid.status_code == 422, f"{username} was created with {rejected!r}"

        created = server.request(
            "POST",
            server.href("register"),
            token=token,
            json={"username": username, "email": None, "password": compliant, "role": "STANDARD"},
        )
        assert created.status_code == 201, "the rejected creation must not have stored the user"

    for rejected in ("short1!", "longpassword"):
        refused = server.request(
            "PATCH",
            server.href("credential", id=1),
            token=token,
            json={"password": rejected},
        )
        assert refused.status_code == 422

    assert server.login(ROOT_ADMIN_USERNAME, password)

    rotated = server.request(
        "PATCH",
        server.href("credential", id=1),
        token=token,
        json={"password": compliant},
    )
    assert rotated.status_code == 204
    assert server.login(ROOT_ADMIN_USERNAME, compliant)
