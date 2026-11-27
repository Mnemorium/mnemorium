"""Black-box proof of the `authentication` capability."""

import json
import re
import time
from itertools import count

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
    API_PREFIX,
    MSG_SETUP_TOKEN,
    QR_BLOCK,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    SEED_PROFILE_ENV,
    WRONG_PASSWORD,
    Sut,
    register,
)
from labels import TestType, alternate_flow, boundary, business_rule, negative, positive, scenario, security
from mnemorium_client.api.identity import patch_credential, post_login, post_register, post_token
from mnemorium_client.api.user import get_me
from mnemorium_client.models import LoginRequest, PatchCredentialRequest, RegisterRequest, Role

# The Crockford base32 alphabet the setup token declares.
SETUP_TOKEN_ALPHABET = frozenset("0123456789ABCDEFGHJKMNPQRSTVWXYZ")
SETUP_TOKEN_PATTERN = re.compile(r"[0-9A-Z]{4}-[0-9A-Z]{4}")
# The reveal-disabled configuration every reveal case shares.
SETUP_REVEAL_DISABLED = {"security": {"root_admin_setup": {"reveal": False}}}

# A credential identifier no suite run can mint.
UNKNOWN_CREDENTIAL = 999_999_999

_ACCOUNTS = count()


def setup_attempt(
    server: Sut,
    token: str,
    *,
    password: str = SECRET_PASSWORD,
    email: str | None = None,
    address: str | None = None,
):
    """Submit one setup exchange, optionally from a declared client address."""
    body = {"token": token, "password": password}
    if email is not None:
        body["email"] = email
    headers = {"x-real-ip": address} if address is not None else None
    return server.request(
        "POST",
        f"{API_PREFIX}/identity/setup",
        json=body,
        headers=headers,
    )


@positive
@scenario("SCN-AUTHENTICATION-001-A")
@pytest.mark.own_instance(reason="first_boot")
def test_first_start_creates_the_root_admin(sut: Sut) -> None:
    """A fresh start prints a setup token, refuses login until setup, then reports user 0 as `root`."""
    server = start_instance(sut)
    token = server.root_setup_token()

    assert token
    assert (
        post_login.sync_detailed(
            client=server.client(),
            body=LoginRequest(password=SECRET_PASSWORD, identifier=ROOT_ADMIN_USERNAME),
        ).status_code
        == 401
    )

    server.root_setup()
    access_token = login(server, ROOT_ADMIN_USERNAME, SECRET_PASSWORD)
    response = get_me.sync_detailed(client=server.client(access_token))

    assert response.status_code == 200
    body = json.loads(response.content)
    assert body["id"] == 0
    assert body["username"] == ROOT_ADMIN_USERNAME
    assert body["role"] == "ADMIN"
    assert body["email"] is None


@business_rule
@scenario("SCN-AUTHENTICATION-001-B")
@pytest.mark.own_instance(reason="restart")
def test_restart_does_not_recreate_or_reveal_the_root_admin(sut: Sut) -> None:
    """An initialized directory restarts healthy, creates no second account, and reveals nothing."""
    server = start_instance(sut)
    server.root_setup()
    password = server.root_password()

    server.restart(env=INSTANCE_ENV)
    token = login(server, ROOT_ADMIN_USERNAME, password)

    assert server.request("GET", server.href("health"), token=token).status_code == 200
    assert MSG_SETUP_TOKEN.search(server.logs()) is None
    listed = server.request("GET", server.href("user"), token=token)
    assert listed.status_code == 200
    assert [item["id"] for item in json.loads(listed.content)["items"]] == [0]


@business_rule
@scenario("SCN-AUTHENTICATION-002-A")
@pytest.mark.own_instance(reason="first_boot")
def test_setup_token_follows_the_declared_shape(sut: Sut) -> None:
    """The issued token is eight Crockford base32 characters displayed as XXXX-XXXX."""
    server = start_instance(sut)
    token = server.root_setup_token()

    assert SETUP_TOKEN_PATTERN.fullmatch(token), f"{token!r} is not displayed as XXXX-XXXX"
    compact = token.replace("-", "")
    assert len(compact) == 8
    assert SETUP_TOKEN_ALPHABET.issuperset(compact), f"{token!r} uses a character outside the alphabet"
    server.root_setup()


@security
@scenario("SCN-AUTHENTICATION-003-A")
@scenario("SCN-LOGGING-008-E", TestType.SECURITY)
@pytest.mark.own_instance(reason="log_sink")
def test_the_setup_token_and_qr_are_printed_once_and_never_logged(sut: Sut) -> None:
    """The reveal prints the token and a QR block once to standard output and no log file holds them."""
    server = start_instance(sut)
    token = server.root_setup_token()
    stdout = server.logs()

    assert stdout.count(token) == 1
    assert QR_BLOCK in stdout, "the reveal printed no QR block"
    server.root_setup()
    assert login(server, ROOT_ADMIN_USERNAME, server.root_password())

    server.stop()
    assert token not in server.log_text()


@negative
@scenario("SCN-AUTHENTICATION-003-B")
@pytest.mark.own_instance(reason="first_boot")
def test_a_first_start_without_the_reveal_is_refused(sut: Sut) -> None:
    """A first start with the reveal disabled fails, names the setting, and creates no account."""
    failure = sut.start_expecting_failure(env=INSTANCE_ENV, config=SETUP_REVEAL_DISABLED)

    assert "security.root_admin_setup.reveal" in failure.output
    assert MSG_SETUP_TOKEN.search(failure.output) is None

    sut.stop()
    server = start_instance(sut, config={"security": {"root_admin_setup": {"reveal": True}}})

    assert server.root_setup_token()
    server.root_setup()
    assert login(server, ROOT_ADMIN_USERNAME, server.root_password())


@negative
@scenario("SCN-AUTHENTICATION-003-D")
@pytest.mark.own_instance(reason="restart")
def test_a_pending_instance_restarts_without_the_reveal_is_refused(sut: Sut) -> None:
    """A pending instance restarted with the reveal disabled fails, leaves the grant usable, and names the setting."""
    server = start_instance(sut)
    server.root_setup_token()
    server.stop()

    failure = sut.start_expecting_failure(env=INSTANCE_ENV, config=SETUP_REVEAL_DISABLED)

    assert "security.root_admin_setup.reveal" in failure.output
    assert MSG_SETUP_TOKEN.search(failure.output) is None

    sut.stop()
    resumed = start_instance(sut, config={"security": {"root_admin_setup": {"reveal": True}}})

    assert resumed.root_setup_token(), "the refused start must have left the grant pending"
    resumed.root_setup()
    assert login(resumed, ROOT_ADMIN_USERNAME, resumed.root_password())


@positive
@scenario("SCN-AUTHENTICATION-003-C")
@pytest.mark.own_instance(reason="restart")
def test_disabling_the_reveal_after_initialization_still_starts(sut: Sut) -> None:
    """An initialized directory starts with the reveal disabled and reveals nothing."""
    server = start_instance(sut)
    server.root_setup()
    password = server.root_password()

    server.restart(env=INSTANCE_ENV, config=SETUP_REVEAL_DISABLED)
    token = login(server, ROOT_ADMIN_USERNAME, password)

    assert server.request("GET", server.href("health"), token=token).status_code == 200
    assert MSG_SETUP_TOKEN.search(server.logs()) is None


@alternate_flow
@scenario("SCN-AUTHENTICATION-002-B")
@pytest.mark.own_instance(reason="first_boot")
def test_a_submitted_token_is_normalized(sut: Sut) -> None:
    """A token submitted lowercased, without its dash and with O/I confusables names the same grant."""
    server = start_instance(sut)
    token = server.root_setup_token().replace("-", "")
    folded = token.lower().replace("0", "o").replace("1", "i")

    response = setup_attempt(server, folded)

    assert response.status_code == 204, f"normalized token refused: {response.status_code} {response.text}"
    assert login(server, ROOT_ADMIN_USERNAME, SECRET_PASSWORD)


@security
@scenario("SCN-AUTHENTICATION-020-A")
@scenario("SCN-AUTHENTICATION-020-C")
@pytest.mark.own_instance(reason="first_boot")
def test_a_used_or_unknown_token_is_refused_identically(sut: Sut) -> None:
    """A consumed token and a token that was never issued answer the same 401 and apply nothing."""
    server = start_instance(sut)
    token = server.root_setup_token()
    assert setup_attempt(server, token).status_code == 204

    reused = setup_attempt(server, token)
    unknown = setup_attempt(server, "0000-0000")

    assert reused.status_code == unknown.status_code == 401
    assert json.loads(reused.content) == json.loads(unknown.content)
    assert login(server, ROOT_ADMIN_USERNAME, SECRET_PASSWORD)


@negative
@scenario("SCN-AUTHENTICATION-020-B")
@pytest.mark.own_instance(reason="first_boot")
def test_an_expired_token_is_refused(sut: Sut) -> None:
    """A token submitted after the configured lifetime answers 401 and applies nothing."""
    server = start_instance(
        sut,
        config={"security": {"root_admin_setup": {"ttl_seconds": 1}}},
    )
    token = server.root_setup_token()

    time.sleep(2)

    response = setup_attempt(server, token)

    assert response.status_code == 401
    assert (
        post_login.sync_detailed(
            client=server.client(),
            body=LoginRequest(password=SECRET_PASSWORD, identifier=ROOT_ADMIN_USERNAME),
        ).status_code
        == 401
    )


@business_rule
@scenario("SCN-AUTHENTICATION-020-D")
@pytest.mark.own_instance(reason="restart")
def test_a_restart_replaces_the_pending_token(sut: Sut) -> None:
    """Restarting while setup is pending invalidates the old token and issues a working one."""
    server = start_instance(sut)
    old = server.root_setup_token()

    server.restart(env=INSTANCE_ENV)
    new = server.root_setup_token()

    assert new != old
    assert setup_attempt(server, old).status_code == 401
    assert setup_attempt(server, new).status_code == 204


@negative
@scenario("SCN-AUTHENTICATION-020-E")
@pytest.mark.own_instance(reason="process_state")
def test_setup_attempts_are_rate_limited(sut: Sut) -> None:
    """One address's burst and then the process-wide budget each answer 429 with Retry-After."""
    server = start_instance(sut)
    peer = server.peer_address()
    server.restart(env=INSTANCE_ENV, config=limiter_config(trusted=peer))

    first = setup_attempt(server, "0000-0000", address="203.0.113.1")
    same_address = setup_attempt(server, "0000-0000", address="203.0.113.1")
    other_address = setup_attempt(server, "0000-0000", address="203.0.113.2")

    assert first.status_code == 401
    assert same_address.status_code == 429
    assert other_address.status_code == 429
    assert "Retry-After" in same_address.headers
    assert "Retry-After" in other_address.headers


@security
@scenario("SCN-AUTHENTICATION-020-F")
@pytest.mark.own_instance(reason="first_boot")
def test_the_setup_token_is_not_a_session(sut: Sut) -> None:
    """Presenting the setup token as a bearer token answers 401."""
    server = start_instance(sut)
    token = server.root_setup_token()

    response = get_me.sync_detailed(client=server.client(token))

    assert response.status_code == 401


@positive
@scenario("SCN-AUTHENTICATION-021-A")
@pytest.mark.own_instance(reason="first_boot")
def test_a_complete_setup_applies(sut: Sut) -> None:
    """A setup answers 204 with no body, the chosen password authenticates, and the email is stored."""
    server = start_instance(sut)
    token = server.root_setup_token()

    response = setup_attempt(server, token, email="root@example.com")

    assert response.status_code == 204
    assert response.content == b""
    access_token = login(server, ROOT_ADMIN_USERNAME, SECRET_PASSWORD)
    profile = get_me.sync_detailed(client=server.client(access_token))
    assert profile.status_code == 200
    assert json.loads(profile.content)["email"] == "root@example.com"
    assert setup_attempt(server, token).status_code == 401


@alternate_flow
@scenario("SCN-AUTHENTICATION-021-B")
@pytest.mark.own_instance(reason="first_boot")
def test_the_setup_email_is_optional(sut: Sut) -> None:
    """A setup with no email answers 204 and leaves the Root Admin without one."""
    server = start_instance(sut)
    token = server.root_setup_token()

    response = setup_attempt(server, token)

    assert response.status_code == 204
    access_token = login(server, ROOT_ADMIN_USERNAME, SECRET_PASSWORD)
    profile = get_me.sync_detailed(client=server.client(access_token))
    assert profile.status_code == 200
    assert json.loads(profile.content)["email"] is None


@negative
@scenario("SCN-AUTHENTICATION-021-C")
@scenario("SCN-AUTHENTICATION-021-D")
@pytest.mark.own_instance(reason="first_boot")
def test_a_refused_setup_leaves_the_grant_usable(sut: Sut) -> None:
    """A weak password answers 422 and a taken email 409; the same token then completes setup."""
    server = start_instance(sut, env={SEED_PROFILE_ENV: "system"})
    token = server.root_setup_token()

    weak = setup_attempt(server, token, password="short1!")
    taken = setup_attempt(server, token, email="coverart@example.com")
    accepted = setup_attempt(server, token)

    assert weak.status_code == 422, f"weak password answered {weak.status_code}: {weak.text}"
    assert taken.status_code == 409, f"taken email answered {taken.status_code}: {taken.text}"
    assert accepted.status_code == 204
    assert login(server, ROOT_ADMIN_USERNAME, SECRET_PASSWORD)


@scenario("SCN-AUTHENTICATION-004-A", TestType.POSITIVE)
@scenario("SCN-AUTHENTICATION-004-B", TestType.POSITIVE)
@scenario("SCN-AUTHENTICATION-004-C", TestType.SECURITY)
@scenario("SCN-API-004-D", TestType.POSITIVE)
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
    assert body["_links"]["self"]["href"] == f"{API_PREFIX}/identity/login"
    assert password not in response.content.decode()
    assert not any("password" in field or "hash" in field for field in body)

    used = get_me.sync_detailed(client=server.client(body["access_token"]))

    assert used.status_code == 200
    profile = json.loads(used.content)
    assert profile["id"] == 0
    assert profile["username"] == ROOT_ADMIN_USERNAME
    assert profile["role"] == "ADMIN"


@security
@scenario("SCN-AUTHENTICATION-005-A")
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


@alternate_flow
@scenario("SCN-AUTHENTICATION-004-D")
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


@positive
@scenario("SCN-AUTHENTICATION-004-A")
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


@security
@scenario("SCN-AUTHENTICATION-006-B")
@pytest.mark.own_instance(reason="log_sink")
def test_login_secrets_are_not_logged(sut: Sut) -> None:
    """A valid login's password and the token it issues never reach a log file."""
    server = start_instance(sut)
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    server.stop()

    logs = server.log_text()
    assert password not in logs, "the submitted password reached a log file"
    assert token not in logs, "the issued token reached a log file"


@negative
@scenario("SCN-AUTHENTICATION-008-A")
@pytest.mark.own_instance(reason="process_state")
def test_a_login_burst_is_throttled(sut: Sut) -> None:
    """Exceeding one address's burst answers 429 with Retry-After."""
    server = start_instance(sut, config=limiter_config())

    assert attempt(server, f"ghost-a{next(_ACCOUNTS)}").status_code == 401
    throttled = attempt(server, f"ghost-b{next(_ACCOUNTS)}")

    assert throttled.status_code == 429
    assert "Retry-After" in throttled.headers


@boundary
@scenario("SCN-AUTHENTICATION-008-B")
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


@security
@scenario("SCN-AUTHENTICATION-008-C")
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


@business_rule
@scenario("SCN-AUTHENTICATION-008-D")
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


@security
@scenario("SCN-AUTHENTICATION-008-E")
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


@security
@scenario("SCN-AUTHENTICATION-008-F")
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


@scenario("SCN-AUTHENTICATION-010-B", TestType.SECURITY)
@scenario("SCN-AUTHENTICATION-010-C", TestType.NEGATIVE)
@scenario("SCN-AUTHENTICATION-010-D", TestType.NEGATIVE)
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


@business_rule
@scenario("SCN-AUTHENTICATION-012-A")
@scenario("SCN-AUTHENTICATION-013-C")
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


@positive
@scenario("SCN-AUTHENTICATION-013-A")
@scenario("SCN-AUTHENTICATION-013-B")
@scenario("SCN-API-004-D", TestType.POSITIVE)
def test_regenerating_a_token_keeps_the_caller(server: Sut, root_token: str) -> None:
    """Regeneration returns the login response shape and a token for the same user."""
    owner_id, owner = provision(server, root_token, f"tokenregen{next(_ACCOUNTS)}")

    response = post_token.sync_detailed(client=server.client(owner))

    assert response.status_code == 200
    body = json.loads(response.content)
    assert {"access_token", "expires_in", "token_type"} <= set(body)
    assert body["token_type"] == "bearer"
    assert body["_links"]["self"]["href"] == f"{API_PREFIX}/identity/token"

    regenerated = get_me.sync_detailed(client=server.client(body["access_token"]))

    assert regenerated.status_code == 200
    assert json.loads(regenerated.content)["id"] == owner_id


# A credential is identified by its own sequence: the Root Admin's is `1`, and on a fresh instance a created account's
# is `user_id + 1`. Only that first inference holds on the shared instance, which is why the rotation cases keep their
# own containers.
@positive
@scenario("SCN-AUTHENTICATION-014-A")
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


@positive
@scenario("SCN-AUTHENTICATION-014-B")
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


@negative
@scenario("SCN-AUTHENTICATION-014-C")
def test_an_unknown_credential_is_not_rotated(server: Sut, root_token: str) -> None:
    """A rotation naming a credential that does not exist answers 404 and stores nothing."""
    rotated = patch_credential.sync_detailed(
        UNKNOWN_CREDENTIAL,
        client=server.client(root_token),
        body=PatchCredentialRequest(password="N3w!Secret"),
    )

    assert rotated.status_code == 404, f"rotation answered {rotated.status_code}: {rotated.content!r}"


@security
@scenario("SCN-AUTHENTICATION-014-D")
@pytest.mark.own_instance(reason="process_state")
def test_a_non_root_caller_cannot_rotate_a_credential(sut: Sut) -> None:
    """An Admin and a Standard User are refused with 403 and their passwords stay unchanged."""
    # The setup exchange and the loop's logins share one bucket, so the burst is raised.
    server = start_instance(
        sut,
        config={"security": {"rate_limit": {"burst_size": 20, "period_seconds": 1}}},
    )
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


@negative
@scenario("SCN-AUTHENTICATION-015-A")
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


@positive
@scenario("SCN-AUTHENTICATION-018-A")
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


@positive
@scenario("SCN-AUTHENTICATION-018-A")
def test_a_compliant_password_is_accepted_at_creation(server: Sut, root_token: str) -> None:
    """A compliant password creates an account that authenticates with it."""
    username = f"policy{next(_ACCOUNTS)}"

    created = post_register.sync_detailed(
        client=server.client(root_token),
        body=RegisterRequest(password=SECRET_PASSWORD, role=Role.STANDARD, username=username),
    )

    assert created.status_code == 201
    assert login(server, username, SECRET_PASSWORD)


@negative
@scenario("SCN-AUTHENTICATION-018-B")
@scenario("SCN-AUTHENTICATION-018-C")
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
