"""Black-box proof of the `logging` capability."""

import shutil
import time
from itertools import count
from urllib.parse import urlsplit

import allure
import pytest
import yaml
from conftest import (
    INSTANCE_ENV,
    JWT_SECRET,
    PEPPER,
    SEED_MEDIA_DIRECTORY,
    attempt,
    begin,
    create_gallery,
    limiter_config,
    login,
    mint_token,
    provision,
    start_instance,
    store_chunk,
)
from harness import (
    LOG_DIRECTORY,
    MEDIA_DIRECTORY,
    MSG_CONFIG_GENERATED,
    ROOT_ADMIN_USERNAME,
    SECRET_PASSWORD,
    STATE_PATH,
    WRONG_PASSWORD,
    Sut,
)
from mnemorium_client.api.asset import delete_upload, post_upload_complete
from mnemorium_client.api.identity import patch_credential, post_login, post_register, post_token
from mnemorium_client.api.library import get_gallery, identify_video_movie_metadata, list_galleries
from mnemorium_client.api.user import get_me
from mnemorium_client.models import LoginRequest, PatchCredentialRequest, RegisterRequest, Role

_LEVEL_ENV = "MNEMORIUM_LOGGING_LEVEL"
_APPLIED = "the environment overrode these configuration settings"
_ROTATION_NEVER_FILE = "mnemorium.log"
_SEEDED_LOGS = {f"logs/mnemorium.log.2020-01-0{day}": "old\n" for day in range(1, 6)}
_ACCOUNTS = count()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-001")
@allure.story("SCN-LOGGING-001-B")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-A")
def test_the_standard_output_sink_holds_the_event(server: Sut, root_token: str) -> None:
    """An accepted credential appears on standard output, naming its actor."""
    name = f"sink{next(_ACCOUNTS)}"
    before = len(server.events("authn_succeeded"))
    owner_id, _token = provision(server, root_token, name)
    server.wait_until(
        lambda: len(server.events("authn_succeeded")) > before,
        "the sink recorded an authn_succeeded event",
    )

    emitted = [record for record in server.events("authn_succeeded")[before:] if record["actor"] == str(owner_id)]
    assert len(emitted) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-001")
@allure.story("SCN-LOGGING-001-D")
def test_standard_output_is_uncoloured_by_default(server: Sut, root_token: str) -> None:
    """The installed configuration colours neither sink."""
    provision(server, root_token, f"plain{next(_ACCOUNTS)}")

    assert "\x1b[" not in server.logs()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-006")
@allure.story("SCN-LOGGING-006-C")
@allure.feature("REQ-LOGGING-009")
@allure.story("SCN-LOGGING-009-A")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-B")
def test_a_rejected_login_is_recorded_as_a_warning(server: Sut) -> None:
    """A wrong password and an unknown identifier each emit one warning with the same reason."""
    first = f"wrong{next(_ACCOUNTS)}"
    second = f"unknown{next(_ACCOUNTS)}@example.com"
    before = len(server.events("authn_failed"))
    post_login.sync_detailed(client=server.client(), body=LoginRequest(password=WRONG_PASSWORD, identifier=first))
    post_login.sync_detailed(client=server.client(), body=LoginRequest(password=WRONG_PASSWORD, identifier=second))
    server.wait_until(
        lambda: len(server.events("authn_failed")) >= before + 2,
        "the sink recorded two authn_failed events",
    )

    emitted = server.events("authn_failed")[before:]
    mine = emitted[:2]
    assert len(mine) == 2
    assert all(record["reason"] == "invalid_credentials" for record in mine)
    logs = server.logs()
    assert first not in logs, "a rejected identifier must never reach the sink"
    assert second not in logs, "a rejected email address must never reach the sink"
    assert all("claimed_identity" not in record for record in mine)
    lines = [line for line in logs.splitlines() if 'event="authn_failed"' in line]
    assert all(" WARN " in line for line in lines)


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-B")
def test_a_rejected_login_carries_no_client_value(server: Sut) -> None:
    """A control character in the submitted identifier cannot forge a record, because it is never logged."""
    nonce = next(_ACCOUNTS)
    claimed = f"evil\nforged{nonce}@example.com"
    before = len(server.events("authn_failed"))
    post_login.sync_detailed(client=server.client(), body=LoginRequest(password=WRONG_PASSWORD, identifier=claimed))
    server.wait_until(
        lambda: len(server.events("authn_failed")) > before,
        "the sink recorded an authn_failed event",
    )

    emitted = server.events("authn_failed")[before:]
    mine = emitted[:1]
    assert len(mine) == 1
    assert "claimed_identity" not in mine[0]
    assert not any(f"forged{nonce}" in line for line in server.logs().splitlines())


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-006")
@allure.story("SCN-LOGGING-006-A")
@allure.feature("REQ-LOGGING-013")
@allure.story("SCN-LOGGING-013-A")
def test_a_denial_is_recorded_once(server: Sut, root_token: str) -> None:
    """A denied view records one `authz_failed` naming the actor, the action and the target."""
    name = f"denied{next(_ACCOUNTS)}"
    owner_id, owner = provision(server, root_token, name)
    gallery = create_gallery(server, root_token, f"private-gallery{next(_ACCOUNTS)}", is_public=False)
    gallery_id = gallery["id"]
    before = len(server.events("authz_failed"))

    response = get_gallery.sync_detailed(gallery_id, client=server.client(owner))

    assert response.status_code == 404
    server.wait_until(
        lambda: len(server.events("authz_failed")) > before,
        "the sink recorded an authz_failed event",
    )
    emitted = server.events("authz_failed")[before:]
    mine = [
        record for record in emitted if record["actor"] == str(owner_id) and record["target"] == f"GALLERY:{gallery_id}"
    ]
    assert len(mine) == 1
    assert mine[0]["action"] == "view"


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-013")
@allure.story("SCN-LOGGING-013-B")
def test_filtering_a_listing_records_no_denial(server: Sut, root_token: str) -> None:
    """A listing that drops an undiscoverable item is not a denial."""
    name = f"listing{next(_ACCOUNTS)}"
    owner_id, owner = provision(server, root_token, name)
    create_gallery(server, root_token, f"hidden-gallery{next(_ACCOUNTS)}", is_public=False)
    before = len(server.events("authz_failed"))

    response = list_galleries.sync_detailed(client=server.client(owner))

    assert response.status_code == 200
    time.sleep(0.2)
    emitted = server.events("authz_failed")[before:]
    assert [record for record in emitted if record["actor"] == str(owner_id)] == []


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-013")
@allure.story("SCN-LOGGING-013-C")
def test_an_out_of_order_action_is_suspicious(server: Sut, root_token: str) -> None:
    """Completing an upload before its chunks arrive records one `suspicious_business_logic`."""
    data = (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes()
    before = len(server.events("suspicious_business_logic"))
    session = begin(server, root_token, f"out-of-order{next(_ACCOUNTS)}.png", data)

    response = post_upload_complete.sync_detailed(session.id, client=server.client(root_token))

    assert response.status_code == 422
    server.wait_until(
        lambda: len(server.events("suspicious_business_logic")) > before,
        "the sink recorded a suspicious_business_logic event",
    )
    emitted = server.events("suspicious_business_logic")[before:]
    mine = [record for record in emitted if record["action"] == "complete" and record["reason"] == "not_complete"]
    assert len(mine) == 1
    assert mine[0]["actor"] == "0"


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-007")
@allure.story("SCN-LOGGING-007-A")
def test_a_query_string_is_withheld(server: Sut, root_token: str) -> None:
    """A traced request carries the path and not the query string."""
    sentinel = f"LEAK-SENTINEL{next(_ACCOUNTS)}"
    path = urlsplit(server.href("gallery")).path

    response = list_galleries.sync_detailed(client=server.client(root_token), name=sentinel)

    assert response.status_code == 200
    logs = server.logs()
    assert f"path={path}" in logs
    assert sentinel not in logs


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-008")
@allure.story("SCN-LOGGING-008-D")
@allure.feature("REQ-LOGGING-018")
@allure.story("SCN-LOGGING-018-A")
@allure.story("SCN-LOGGING-018-B")
def test_a_personal_data_read_is_recorded(server: Sut, root_token: str) -> None:
    """Reading personal data records the actor and the object and no personal value."""
    name = f"personal{next(_ACCOUNTS)}"
    before = len(server.events("sensitive_data_access"))
    owner_id, owner = provision(server, root_token, name)

    response = get_me.sync_detailed(client=server.client(owner))

    assert response.status_code == 200
    server.wait_until(
        lambda: len(server.events("sensitive_data_access")) > before,
        "the sink recorded a sensitive_data_access event",
    )
    emitted = server.events("sensitive_data_access")[before:]
    mine = [record for record in emitted if record["actor"] == str(owner_id)]
    assert len(mine) == 1
    assert mine[0]["object"] == "current_user"
    assert name not in mine[0].values()
    assert not any("@" in value for value in mine[0].values())


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-E")
def test_a_token_regeneration_names_the_actor(server: Sut, root_token: str) -> None:
    """A token regeneration records one event naming the actor."""
    name = f"regen{next(_ACCOUNTS)}"
    before = len(server.events("token_regenerated"))
    owner_id, owner = provision(server, root_token, name)

    response = post_token.sync_detailed(client=server.client(owner))

    assert response.status_code == 200
    server.wait_until(
        lambda: len(server.events("token_regenerated")) > before,
        "the sink recorded a token_regenerated event",
    )
    emitted = server.events("token_regenerated")[before:]
    mine = [record for record in emitted if record["actor"] == str(owner_id)]
    assert len(mine) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-015")
@allure.story("SCN-LOGGING-015-D")
def test_user_administration_names_actor_and_target(server: Sut, root_token: str) -> None:
    """Creating a user records one `user_admin` naming the actor, the action and the target."""
    before = len(server.events("user_admin"))
    created = post_register.sync_detailed(
        client=server.client(root_token),
        body=RegisterRequest(
            password=SECRET_PASSWORD,
            role=Role.STANDARD,
            username=f"admin-target{next(_ACCOUNTS)}",
        ),
    )

    assert created.status_code == 201
    server.wait_until(
        lambda: len(server.events("user_admin")) > before,
        "the sink recorded a user_admin event",
    )
    emitted = server.events("user_admin")[before:]
    mine = [record for record in emitted if record["action"] == "create" and record["target"] == "user"]
    assert mine
    assert mine[-1]["actor"] == "0"


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-017")
@allure.story("SCN-LOGGING-017-A")
def test_an_upload_lifecycle_is_recorded(server: Sut, root_token: str) -> None:
    """An accepted, written, completed and cancelled upload each record one event with its outcome."""
    data = (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes()
    name = f"lifecycle{next(_ACCOUNTS)}"
    before = len(server.events("upload"))
    session = begin(server, root_token, f"{name}.png", data)
    assert store_chunk(server, root_token, session, 0, data).status_code == 200
    assert post_upload_complete.sync_detailed(session.id, client=server.client(root_token)).status_code == 200
    cancelled = begin(server, root_token, f"{name}-cancel.png", data)
    assert delete_upload.sync_detailed(cancelled.id, client=server.client(root_token)).status_code == 204
    server.wait_until(
        lambda: len(server.events("upload")) >= before + 5,
        "the sink recorded five upload events",
    )

    emitted = server.events("upload")[before:]
    first = [record["outcome"] for record in emitted if record["upload"] == str(session.id)]
    second = [record["outcome"] for record in emitted if record["upload"] == str(cancelled.id)]
    assert first == ["accepted", "chunk_written", "completed"]
    assert second == ["accepted", "cancelled"]
    assert all(record["actor"] == "0" for record in emitted if record["upload"] in {str(session.id), str(cancelled.id)})


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-001")
@allure.story("SCN-LOGGING-001-A")
@pytest.mark.own_instance(reason="log_sink")
def test_the_file_sink_holds_the_event(sut: Sut) -> None:
    """An accepted credential appears in `logs/mnemorium.log`."""
    server = start_instance(sut)
    login(server, ROOT_ADMIN_USERNAME, server.root_password())
    server.stop()

    assert len(server.written_events("authn_succeeded")) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-001")
@allure.story("SCN-LOGGING-001-C")
@pytest.mark.own_instance(reason="log_sink")
def test_the_file_sink_is_never_coloured(sut: Sut) -> None:
    """Colour enabled for standard output never reaches the file sink."""
    server = start_instance(sut, config={"logging": {"ansi": True}})
    login(server, ROOT_ADMIN_USERNAME, server.root_password())
    stdout = server.logs()
    server.stop()

    assert "\x1b[" in stdout
    assert "\x1b[" not in server.log_text()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-003")
@allure.story("SCN-LOGGING-003-D")
@pytest.mark.own_instance(reason="log_sink")
def test_the_error_rung_hides_the_warning_and_info(sut: Sut) -> None:
    """At `error`, an operational warning and info reach no sink."""
    server = start_instance(sut, env={_LEVEL_ENV: "error"})
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    get_me.sync_detailed(client=server.client(token))

    assert MSG_CONFIG_GENERATED not in server.logs()
    assert _APPLIED not in server.logs()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-003")
@allure.story("SCN-LOGGING-003-D")
@pytest.mark.own_instance(reason="log_sink")
def test_the_warn_rung_keeps_the_warning_and_hides_the_info(sut: Sut) -> None:
    """At `warn`, an operational warning reaches a sink and the info below it does not."""
    server = start_instance(sut, env={_LEVEL_ENV: "warn"})
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    get_me.sync_detailed(client=server.client(token))

    assert MSG_CONFIG_GENERATED in server.logs()
    assert _APPLIED not in server.logs()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-003")
@allure.story("SCN-LOGGING-003-D")
@pytest.mark.own_instance(reason="log_sink")
def test_the_info_rung_keeps_the_warning_and_info(sut: Sut) -> None:
    """At `info`, the applied-setting record reaches a sink."""
    server = start_instance(sut, env={_LEVEL_ENV: "info"})
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    get_me.sync_detailed(client=server.client(token))

    assert MSG_CONFIG_GENERATED in server.logs()
    assert _APPLIED in server.logs()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-004")
@allure.story("SCN-LOGGING-004-A")
@pytest.mark.own_instance(reason="log_sink")
def test_a_fault_only_verbosity_cannot_silence_a_success(sut: Sut) -> None:
    """With a verbosity admitting only `error`, a login succeeds and its catalogued event reaches the sinks."""
    server = start_instance(sut, env={_LEVEL_ENV: "error"})
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    assert token
    assert len(server.events("authn_succeeded")) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-004")
@allure.story("SCN-LOGGING-004-B")
@pytest.mark.own_instance(reason="log_sink")
def test_the_lowest_declared_severity_survives(sut: Sut) -> None:
    """With a verbosity admitting only `error`, a catalogued `debug` event still reaches the sinks."""
    server = start_instance(sut, env={_LEVEL_ENV: "error"})
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    response = get_me.sync_detailed(client=server.client(token))

    assert response.status_code == 200
    assert len(server.events("sensitive_data_access")) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-002")
@allure.story("SCN-LOGGING-002-A")
@pytest.mark.own_instance(reason="restart")
def test_rotation_replaces_the_active_file(sut: Sut) -> None:
    """A period change starts a dated active file holding the further event and keeps the previous file."""
    server = start_instance(sut, config={"logging": {"rotation": "NEVER"}})
    server.stop()

    assert server.listing(LOG_DIRECTORY) == [_ROTATION_NEVER_FILE]

    server.start(env=INSTANCE_ENV, config={"logging": {"rotation": "DAILY"}})
    names = server.listing(LOG_DIRECTORY)
    dated = [name for name in names if name.startswith(f"{_ROTATION_NEVER_FILE}.")]
    assert _ROTATION_NEVER_FILE in names, "the previous active file must be retained"
    assert len(dated) == 1, f"expected one dated active file, found {names}"
    server.stop()
    assert 'event="lifecycle"' in server.read(f"{LOG_DIRECTORY}/{dated[0]}")


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-002")
@allure.story("SCN-LOGGING-002-B")
@pytest.mark.own_instance(reason="log_sink")
def test_pruning_bounds_the_retained_files(sut: Sut) -> None:
    """A maximum of two keeps one seeded file plus today's."""
    sut.start(config={"logging": {"max_files": 2}}, files=_SEEDED_LOGS)
    sut.stop()

    assert len(sut.listing(LOG_DIRECTORY)) == 2


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-002")
@allure.story("SCN-LOGGING-002-C")
@pytest.mark.own_instance(reason="log_sink")
def test_a_zero_maximum_keeps_every_file(sut: Sut) -> None:
    """A maximum of zero retains the five seeded files and today's."""
    sut.start(config={"logging": {"max_files": 0}}, files=_SEEDED_LOGS)
    sut.stop()

    assert len(sut.listing(LOG_DIRECTORY)) == 6


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-015")
@allure.story("SCN-LOGGING-015-A")
@pytest.mark.own_instance(reason="log_sink")
def test_startup_records_a_lifecycle_event(sut: Sut) -> None:
    """Installing logging records the component and the outcome, even at a fault-only verbosity."""
    server = start_instance(sut, env={_LEVEL_ENV: "error"})
    server.stop()

    installed = [
        record
        for record in server.written_events("lifecycle")
        if record.get("component") == "logging" and record.get("outcome") == "initialized"
    ]
    assert len(installed) == 1


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-007")
@allure.story("SCN-LOGGING-007-B")
@allure.feature("REQ-LOGGING-009")
@allure.story("SCN-LOGGING-009-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_file_name_is_withheld_from_a_storage_fault(sut: Sut) -> None:
    """A failed promotion records its classification at `error` and never the user-supplied name."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    data = (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes()
    sentinel = "leak-sentinel.png"
    session = begin(server, token, sentinel, data)
    assert store_chunk(server, token, session, 0, data).status_code == 200
    shutil.rmtree(server.dir / MEDIA_DIRECTORY)

    response = post_upload_complete.sync_detailed(session.id, client=server.client(token))

    assert response.status_code == 503
    emitted = server.wait_for_events("port_fault")
    assert len(emitted) == 1
    assert emitted[0]["kind"] == "destination_missing"
    assert emitted[0]["operation"] == "promote"
    lines = [line for line in server.logs().splitlines() if 'event="port_fault"' in line]
    assert lines and "ERROR" in lines[0]
    assert sentinel not in server.logs()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-008")
@allure.story("SCN-LOGGING-008-A")
@pytest.mark.own_instance(reason="log_sink")
def test_login_secrets_stay_out_of_every_sink(sut: Sut) -> None:
    """Neither the submitted password nor the issued token reaches standard output or a log file."""
    server = start_instance(sut)
    password = server.root_password()
    token = login(server, ROOT_ADMIN_USERNAME, password)
    new_password = "N3w!Secret"

    response = patch_credential.sync_detailed(
        1,
        client=server.client(token),
        body=PatchCredentialRequest(password=new_password),
    )

    assert response.status_code == 204
    stdout = server.logs()
    server.stop()
    written = server.log_text()
    assert token not in stdout
    assert token not in written
    assert new_password not in stdout
    assert new_password not in written
    assert password not in written


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-008")
@allure.story("SCN-LOGGING-008-B")
@pytest.mark.own_instance(reason="log_sink")
def test_the_configured_instance_secrets_stay_out_of_every_sink(sut: Sut) -> None:
    """Neither the configured pepper nor the signing secret reaches a sink."""
    server = start_instance(sut)
    server.root_password()
    stdout = server.logs()
    server.stop()
    written = server.log_text()

    for secret in (PEPPER, JWT_SECRET):
        assert secret not in stdout
        assert secret not in written


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-008")
@allure.story("SCN-LOGGING-008-B")
@pytest.mark.own_instance(reason="first_boot")
def test_the_generated_instance_secrets_stay_out_of_every_sink(sut: Sut) -> None:
    """Neither the generated pepper nor the generated signing secret reaches a sink."""
    server = sut.start()
    state = yaml.safe_load(server.read(STATE_PATH))
    server.root_password()
    stdout = server.logs()
    server.stop()
    written = server.log_text()

    for secret in (state["pepper"], state["jwt_secret"]):
        assert secret not in stdout
        assert secret not in written


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-C")
@pytest.mark.own_instance(reason="log_sink")
def test_token_rejections_name_their_reason(sut: Sut) -> None:
    """A missing, invalid and expired token each emit one `session_validation_failed`."""
    server = start_instance(sut)
    server.root_password()

    rejected = (
        get_me.sync_detailed(client=server.client()),
        get_me.sync_detailed(client=server.client(mint_token(JWT_SECRET, "0", tampered=True))),
        get_me.sync_detailed(client=server.client(mint_token(JWT_SECRET, "0", expires_in=-1))),
    )
    assert [response.status_code for response in rejected] == [401, 401, 401]

    emitted = server.wait_for_events("session_validation_failed")
    assert len(emitted) == 3
    assert sum(record["reason"] == "missing_authorization_header" for record in emitted) == 1
    assert sum(record["reason"] == "invalid_or_expired_token" for record in emitted) == 2


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-D")
@pytest.mark.own_instance(reason="log_sink")
def test_a_throttled_request_names_its_route(sut: Sut) -> None:
    """A rate-limited login records one `rate_limit_exceeded` with its route and reason."""
    server = start_instance(sut, config=limiter_config())
    server.root_password()

    assert attempt(server, "ghost-a").status_code == 401
    assert attempt(server, "ghost-b").status_code == 429

    emitted = server.wait_for_events("rate_limit_exceeded")
    assert len(emitted) == 1
    assert emitted[0]["route"] == "identity.login"
    assert emitted[0]["reason"] == "quota_exceeded"


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-012")
@allure.story("SCN-LOGGING-012-E")
@pytest.mark.own_instance(reason="log_sink")
def test_a_credential_rotation_names_the_actor(sut: Sut) -> None:
    """A credential rotation records one event naming the actor."""
    server = start_instance(sut)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())

    rotated = patch_credential.sync_detailed(
        1,
        client=server.client(token),
        body=PatchCredentialRequest(password="R0tated!Pass"),
    )

    assert rotated.status_code == 204
    emitted = server.wait_for_events("credential_rotated")
    assert len(emitted) == 1
    assert emitted[0]["actor"] == "0"


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-015")
@allure.story("SCN-LOGGING-015-B")
@allure.story("SCN-LOGGING-015-C")
@pytest.mark.own_instance(reason="first_boot")
def test_identity_creation_is_reported(sut: Sut) -> None:
    """A first start creates the identity and reports it once per generated secret; reuse reports nothing."""
    sut.start()
    sut.stop()
    first = sut.log_text()

    objects = [record for record in sut.written_events("system_object")]
    identity = [record for record in objects if record.get("object") == "instance_state"]
    assert len(identity) == 1
    assert identity[0].get("action") == "create"
    assert len([record for record in objects if record.get("object") == "root_admin"]) == 1

    secrets = sut.written_events("secret_initialized")
    assert len(secrets) == 2
    assert sum(record.get("component") == "pepper" for record in secrets) == 1
    assert sum(record.get("component") == "jwt_secret" for record in secrets) == 1

    sut.start()
    sut.stop()
    reused = sut.log_text()[len(first) :]

    assert 'event="system_object"' not in reused
    assert 'event="secret_initialized"' not in reused


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-016")
@allure.story("SCN-LOGGING-016-A")
@pytest.mark.own_instance(reason="global_filesystem")
def test_an_ingest_reports_counts_only(sut: Sut) -> None:
    """A startup scan records the counts and never a file name."""
    sentinel = "incoming-sentinel.png"
    sut.write(f"{MEDIA_DIRECTORY}/{sentinel}", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())

    server = sut.start()
    emitted = server.wait_for_events("ingest")
    line = emitted[0]

    assert line["registered"] == "1"
    for counter in ("completed", "untouched", "changed", "missing", "removed", "skipped"):
        assert line.get(counter) == "0", f"missing the {counter} counter"
    assert sentinel not in server.logs()


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-016")
@allure.story("SCN-LOGGING-016-B")
@pytest.mark.own_instance(reason="global_filesystem")
def test_a_degraded_observer_is_reported(sut: Sut) -> None:
    """An unreadable library root degrades the observer and records `ingest_fault` with its kind."""
    media = sut.dir / MEDIA_DIRECTORY
    media.mkdir()
    media.chmod(0o000)
    try:
        server = sut.start()

        emitted = server.wait_for_events("ingest_fault")
        assert emitted[0]["kind"] == "watch_registration_failed"
        assert emitted[0]["operation"] == "media_change_source"
    finally:
        media.chmod(0o700)


@allure.epic("Logging")
@allure.feature("REQ-LOGGING-016")
@allure.story("SCN-LOGGING-016-C")
@allure.story("SCN-LOGGING-016-D")
@pytest.mark.own_instance(reason="log_sink")
def test_a_skipped_provider_reports_its_classification(sut: Sut) -> None:
    """A lookup naming an unconfigured provider records only the provider and the classification."""
    config = {"library": {"metadata": {"video": {"providers": [{"name": "tmdb", "index": 1, "api_key": "test-key"}]}}}}
    server = start_instance(sut, config=config)
    token = login(server, ROOT_ADMIN_USERNAME, server.root_password())
    reference = "tt0111161"

    response = identify_video_movie_metadata.sync_detailed("omdb", reference, client=server.client(token))

    assert response.status_code in {200, 404}
    emitted = server.wait_for_events("metadata_provider_unconfigured")
    assert len(emitted) == 1
    assert emitted[0]["provider"] == "omdb"
    assert reference not in emitted[0].values()
    lines = [line for line in server.logs().splitlines() if 'event="metadata_provider_unconfigured"' in line]
    assert lines and reference not in lines[0]
