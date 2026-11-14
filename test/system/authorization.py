"""Black-box proof of the `authorization` capability.

Every case starts its own container from the image under test through the shared `harness` plugin and reads only what an
operator can: HTTP responses and the working directory. The suite drives the resource-and-grant engine through the
public API: it creates galleries, reads them as several callers, and changes their permission with the three
`/identity/permission` operations.

The API index publishes the permission summary operation; the resource policy and the share operation extend it, so no
path is built from a base literal here.

The `REQ-GALLERY-006` case lives here because it is the same shape of proof as the rest of the module — a resource the
caller can read but may not change answers `403` — but its labels stay the gallery capability's, because the scenario is
owned by `openspec/specs/gallery/spec.md`. The split is deliberate, not drift.
"""

import allure
from conftest import FIXTURE_DIRECTORY, add_to_gallery, create_gallery, linked_id, provision, upload
from harness import ROOT_ADMIN_USERNAME, SECRET_PASSWORD, Sut, begin_upload, link_of, register

GALLERY = "GALLERY"
VIEWER = "VIEWER"
EDITOR = "EDITOR"
OWNER = "OWNER"
ADMINISTRATION = "MANAGE_PERMISSIONS"

PRIVATE = "PRIVATE"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-031")
@allure.story("SCN-AUTHORIZATION-031-A")
def test_a_caller_reads_its_own_summary(server: Sut, root_token: str) -> None:
    """A Standard user reaches its own summary and receives no grant internals."""
    alice_id, alice = provision(server, root_token, "alice")

    response = server.request("GET", f"{server.href('permission')}?user_id={alice_id}", token=alice)

    assert response.status_code == 200, f"summary failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["user_id"] == alice_id
    assert body["role"] == "STANDARD"
    assert all("grant_id" not in item for item in body["grants"])


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-031")
@allure.story("SCN-AUTHORIZATION-031-B")
def test_an_admin_reads_a_standard_users_summary(server: Sut, root_token: str) -> None:
    """An Admin reaches a Standard user's summary."""
    bob_id, _bob = provision(server, root_token, "bobby")
    admin_id, admin = provision(server, root_token, "admina", role="ADMIN")
    assert admin_id > 0

    response = server.request("GET", f"{server.href('permission')}?user_id={bob_id}", token=admin)

    assert response.status_code == 200, f"summary failed: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-031")
@allure.story("SCN-AUTHORIZATION-031-C")
def test_another_users_summary_is_forbidden(server: Sut, root_token: str) -> None:
    """A Standard user may not read another user's summary."""
    _bob_id, bob = provision(server, root_token, "bobby")
    carol_id, _carol = provision(server, root_token, "carol")

    response = server.request("GET", f"{server.href('permission')}?user_id={carol_id}", token=bob)

    assert response.status_code == 403, f"summary leaked: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-031")
@allure.story("SCN-AUTHORIZATION-031-D")
def test_the_explanation_is_returned_to_a_manager(server: Sut, root_token: str) -> None:
    """The Root Admin receives the decision and its explanation."""
    alice_id, _alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, root_token, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    response = server.request(
        "GET",
        f"{server.href('permission')}?user_id={alice_id}&resource_kind={GALLERY}&resource_id={gallery_id}&action=view",
        token=root_token,
    )

    assert response.status_code == 200, f"explanation failed: {response.status_code} {response.text}"
    decision = response.json()["decision"]
    assert decision["allowed"] is False
    assert decision["reason"] == "default"
    assert decision["auth_path"][0]["kind"] == GALLERY


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-032")
@allure.story("SCN-AUTHORIZATION-032-A")
def test_an_owner_reads_the_resource_permission(server: Sut, root_token: str) -> None:
    """The owner reads its gallery's visibility and grants."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    response = server.request(
        "GET",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}",
        token=alice,
    )

    assert response.status_code == 200, f"policy failed: {response.status_code} {response.text}"
    body = response.json()
    assert body["visibility"] == PRIVATE
    assert body["address"]["kind"] == GALLERY


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-032")
@allure.story("SCN-AUTHORIZATION-032-B")
def test_an_owner_replaces_the_set(server: Sut, root_token: str) -> None:
    """A replacement stores exactly the submitted grant set."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])
    policy = f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}"

    replaced = server.request(
        "PUT",
        policy,
        token=alice,
        json={
            "visibility": "PUBLIC",
            "grants": [{"principal": {"kind": "USER", "id": bob_id}, "role": VIEWER, "effect": "ALLOW"}],
        },
    )
    followed = server.request("GET", policy, token=alice)

    assert replaced.status_code == 200, f"replacement failed: {replaced.status_code} {replaced.text}"
    assert followed.status_code == 200, f"policy failed: {followed.status_code} {followed.text}"
    body = followed.json()
    assert body["visibility"] == "PUBLIC"
    assert [item["role"] for item in body["grants"] if item["principal"]["id"] == bob_id] == [VIEWER]
    assert bob_id > 0
    assert bob


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-032")
@allure.story("SCN-AUTHORIZATION-032-C")
def test_a_manager_cannot_replace(server: Sut, root_token: str) -> None:
    """A caller holding Manager but not ManagePermissions may read but not replace."""
    alice_id, alice = provision(server, root_token, "alice")
    bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])
    policy = f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}"

    shared = server.request(
        "PUT",
        policy,
        token=alice,
        json={
            "visibility": PRIVATE,
            "grants": [{"principal": {"kind": "USER", "id": bob_id}, "role": "MANAGER", "effect": "ALLOW"}],
        },
    )
    assert shared.status_code == 200, f"replacement failed: {shared.status_code} {shared.text}"

    read = server.request("GET", policy, token=bob)
    replaced = server.request("PUT", policy, token=bob, json={"visibility": "PUBLIC", "grants": []})

    assert read.status_code == 200, f"policy failed: {read.status_code} {read.text}"
    assert replaced.status_code == 403, f"replacement allowed: {replaced.status_code} {replaced.text}"
    assert alice_id > 0


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-032")
@allure.story("SCN-AUTHORIZATION-032-D")
def test_a_rejected_replacement_leaves_the_previous_set(server: Sut, root_token: str) -> None:
    """A replacement naming the Owner role answers 422 and stores nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, _bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])
    policy = f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}"

    refused = server.request(
        "PUT",
        policy,
        token=alice,
        json={
            "visibility": "PUBLIC",
            "grants": [{"principal": {"kind": "USER", "id": bob_id}, "role": OWNER, "effect": "ALLOW"}],
        },
    )
    followed = server.request("GET", policy, token=alice)

    assert refused.status_code == 422, f"replacement accepted: {refused.status_code} {refused.text}"
    assert followed.json()["visibility"] == PRIVATE


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-033")
@allure.story("SCN-AUTHORIZATION-033-A")
def test_a_manager_shares_a_viewer(server: Sut, root_token: str) -> None:
    """An owner shares Viewer and the recipient can read the gallery."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    shared = server.request(
        "POST",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}/share",
        token=alice,
        json={"principal": {"kind": "USER", "id": bob_id}, "role": VIEWER},
    )
    read = server.request("GET", f"{server.href('gallery')}/{gallery_id}", token=bob)

    assert shared.status_code == 200, f"share failed: {shared.status_code} {shared.text}"
    assert read.status_code == 200, f"shared read failed: {read.status_code} {read.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-033")
@allure.story("SCN-AUTHORIZATION-033-B")
def test_sharing_cannot_escalate(server: Sut, root_token: str) -> None:
    """A share naming Editor answers 403 and stores nothing."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    shared = server.request(
        "POST",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}/share",
        token=alice,
        json={"principal": {"kind": "USER", "id": bob_id}, "role": EDITOR},
    )
    read = server.request("GET", f"{server.href('gallery')}/{gallery_id}", token=bob)

    assert shared.status_code == 403, f"escalation allowed: {shared.status_code} {shared.text}"
    assert read.status_code == 404, f"escalated read allowed: {read.status_code} {read.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-033")
@allure.story("SCN-AUTHORIZATION-033-C")
def test_a_share_is_revoked(server: Sut, root_token: str) -> None:
    """A revoked share stops reaching the resource."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])
    shared = server.request(
        "POST",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}/share",
        token=alice,
        json={"principal": {"kind": "USER", "id": bob_id}, "role": VIEWER},
    )
    assert shared.status_code == 200, f"share failed: {shared.status_code} {shared.text}"

    revoked = server.request("DELETE", shared.json()["_links"]["self"]["href"], token=alice)
    read = server.request("GET", f"{server.href('gallery')}/{gallery_id}", token=bob)

    assert revoked.status_code == 204, f"revocation failed: {revoked.status_code} {revoked.text}"
    assert read.status_code == 404, f"revoked read allowed: {read.status_code} {read.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-034")
@allure.story("SCN-AUTHORIZATION-034-A")
def test_the_owner_role_is_refused(server: Sut, root_token: str) -> None:
    """A grant naming Owner answers 422."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, _bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}",
        token=alice,
        json={
            "visibility": PRIVATE,
            "grants": [{"principal": {"kind": "USER", "id": bob_id}, "role": OWNER, "effect": "ALLOW"}],
        },
    )

    assert response.status_code == 422, f"owner grant accepted: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-034")
@allure.story("SCN-AUTHORIZATION-034-B")
def test_permission_administration_is_refused(server: Sut, root_token: str) -> None:
    """A grant naming ManagePermissions answers 422."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, _bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}",
        token=alice,
        json={
            "visibility": PRIVATE,
            "grants": [
                {
                    "principal": {"kind": "USER", "id": bob_id},
                    "role": ADMINISTRATION,
                    "effect": "ALLOW",
                }
            ],
        },
    )

    assert response.status_code == 422, f"administration grant accepted: {response.status_code}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-034")
@allure.story("SCN-AUTHORIZATION-034-C")
def test_a_duplicate_is_refused(server: Sut, root_token: str) -> None:
    """A set naming the same grant twice answers 422."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, _bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])
    duplicate = {"principal": {"kind": "USER", "id": bob_id}, "role": VIEWER, "effect": "ALLOW"}

    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}",
        token=alice,
        json={"visibility": PRIVATE, "grants": [duplicate, duplicate]},
    )

    assert response.status_code == 422, f"duplicate accepted: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-034")
@allure.story("SCN-AUTHORIZATION-034-D")
def test_an_unknown_principal_is_refused(server: Sut, root_token: str) -> None:
    """A grant naming a user that does not exist answers 422."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}",
        token=alice,
        json={
            "visibility": PRIVATE,
            "grants": [{"principal": {"kind": "USER", "id": 999_999}, "role": VIEWER, "effect": "ALLOW"}],
        },
    )

    assert response.status_code == 422, f"unknown principal accepted: {response.status_code}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-034")
@allure.story("SCN-AUTHORIZATION-034-E")
def test_admin_creation_is_refused(server: Sut, root_token: str) -> None:
    """A CreateChild grant naming the Admin child kind answers 422."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, _bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}",
        token=alice,
        json={
            "visibility": PRIVATE,
            "grants": [
                {
                    "principal": {"kind": "USER", "id": bob_id},
                    "role": "CONTRIBUTOR",
                    "child_kind": "ADMIN",
                    "effect": "ALLOW",
                }
            ],
        },
    )

    assert response.status_code == 422, f"admin creation accepted: {response.status_code}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-035")
@allure.story("SCN-AUTHORIZATION-035-A")
def test_a_role_grant_cannot_be_written(server: Sut, root_token: str) -> None:
    """A replacement naming a role principal answers 422."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    response = server.request(
        "PUT",
        f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}",
        token=alice,
        json={
            "visibility": PRIVATE,
            "grants": [
                {
                    "principal": {"kind": "ROLE", "role": "STANDARD"},
                    "role": VIEWER,
                    "effect": "ALLOW",
                }
            ],
        },
    )

    assert response.status_code == 422, f"role grant accepted: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-035")
@allure.story("SCN-AUTHORIZATION-035-B")
def test_the_seeded_role_applies(server: Sut, root_token: str) -> None:
    """A Standard user with no per-resource grant creates a gallery through its seeded role."""
    _alice_id, alice = provision(server, root_token, "alice")

    response = server.request(
        "POST",
        server.href("gallery"),
        token=alice,
        json={"name": "owned", "is_public": False},
    )

    assert response.status_code == 201, f"catalogue write failed: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-016")
@allure.story("SCN-AUTHORIZATION-016-A")
def test_a_public_resource_is_readable_by_every_caller(server: Sut, root_token: str) -> None:
    """A public gallery is readable by a user with no grant."""
    _alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday", is_public=True)
    gallery_id = linked_id(gallery["_links"]["self"]["href"])

    response = server.request("GET", f"{server.href('gallery')}/{gallery_id}", token=bob)

    assert response.status_code == 200, f"public read failed: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-014")
@allure.story("SCN-AUTHORIZATION-014-A")
def test_a_deny_wins_over_a_grant(server: Sut, root_token: str) -> None:
    """A deny stored on the resource defeats a viewer grant."""
    _alice_id, alice = provision(server, root_token, "alice")
    bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])
    policy = f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}"

    permitted = server.request(
        "PUT",
        policy,
        token=alice,
        json={
            "visibility": PRIVATE,
            "grants": [{"principal": {"kind": "USER", "id": bob_id}, "role": VIEWER, "effect": "ALLOW"}],
        },
    )
    assert permitted.status_code == 200, f"grant failed: {permitted.status_code} {permitted.text}"
    readable = server.request("GET", f"{server.href('gallery')}/{gallery_id}", token=bob)
    assert readable.status_code == 200, f"granted read failed: {readable.status_code}"

    denied = server.request(
        "PUT",
        policy,
        token=alice,
        json={
            "visibility": PRIVATE,
            "grants": [
                {"principal": {"kind": "USER", "id": bob_id}, "role": VIEWER, "effect": "ALLOW"},
                {"principal": {"kind": "USER", "id": bob_id}, "role": VIEWER, "effect": "DENY"},
            ],
        },
    )
    unreadable = server.request("GET", f"{server.href('gallery')}/{gallery_id}", token=bob)

    assert denied.status_code == 200, f"deny failed: {denied.status_code} {denied.text}"
    assert unreadable.status_code == 404, f"deny defeated: {unreadable.status_code} {unreadable.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-021")
@allure.story("SCN-AUTHORIZATION-021-A")
def test_a_caller_reads_its_own_account(server: Sut, root_token: str) -> None:
    """A caller reads its own account without a grant."""
    alice_id, alice = provision(server, root_token, "alice")

    response = server.request("GET", f"{server.href('user')}/{alice_id}", token=alice)

    assert response.status_code == 200, f"own account failed: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-018")
@allure.story("SCN-AUTHORIZATION-018-A")
def test_the_root_cannot_read_another_users_upload(server: Sut, root_token: str) -> None:
    """An upload is owner-only, even for the Root Admin."""
    _alice_id, alice = provision(server, root_token, "alice")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    session = begin_upload(server, alice, "cover.png", image)

    response = server.request("GET", session.self_href, token=root_token)

    assert response.status_code == 404, f"root read the upload: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-022")
@allure.story("SCN-AUTHORIZATION-022-B")
def test_a_user_cannot_create_an_admin(server: Sut, root_token: str) -> None:
    """A Standard user may not register an Admin account."""
    _alice_id, alice = provision(server, root_token, "alice")

    response = server.request(
        "POST",
        server.href("register"),
        token=alice,
        json={
            "username": "mallory",
            "email": None,
            "password": SECRET_PASSWORD,
            "role": "ADMIN",
        },
    )

    assert response.status_code == 403, f"admin creation allowed: {response.status_code} {response.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-025")
@allure.story("SCN-AUTHORIZATION-025-A")
def test_an_asset_is_reachable_through_its_gallery(server: Sut, root_token: str) -> None:
    """A gallery item's image is readable by the gallery owner and concealed from others."""
    _alice_id, alice = provision(server, root_token, "alice")
    _bob_id, bob = provision(server, root_token, "bobby")
    gallery = create_gallery(server, alice, "holiday")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    file = upload(server, alice, "cover.png", image)
    item = add_to_gallery(server, alice, gallery, linked_id(link_of(file, "image")))

    owner_read = server.request("GET", link_of(item, "self"), token=alice)
    stranger_read = server.request("GET", link_of(item, "self"), token=bob)

    assert owner_read.status_code == 200, f"owner read failed: {owner_read.status_code}"
    assert stranger_read.status_code == 404, f"stranger read allowed: {stranger_read.status_code}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-019")
@allure.story("SCN-AUTHORIZATION-019-A")
def test_an_admin_creates_a_standard_account(server: Sut, root_token: str) -> None:
    """An Admin creates a Standard account, which then authenticates."""
    _admin_id, admin = provision(server, root_token, "admina", role="ADMIN")

    created = register(server, admin, "alice", None, SECRET_PASSWORD)

    assert created.status_code == 201, f"creation failed: {created.status_code} {created.text}"
    assert created.json()["role"] == "STANDARD"
    assert server.attempt_login("alice", SECRET_PASSWORD).status_code == 200


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-019")
@allure.story("SCN-AUTHORIZATION-019-B")
def test_an_admin_cannot_create_an_admin(server: Sut, root_token: str) -> None:
    """An Admin creating an Admin answers 403 and stores nothing."""
    _admin_id, admin = provision(server, root_token, "admina", role="ADMIN")

    response = server.request(
        "POST",
        server.href("register"),
        token=admin,
        json={
            "username": "mallory",
            "email": None,
            "password": SECRET_PASSWORD,
            "role": "ADMIN",
        },
    )

    assert response.status_code == 403, f"admin creation allowed: {response.status_code} {response.text}"
    listed = server.request("GET", f"{server.href('user')}?username=mallory", token=root_token)
    assert listed.json()["items"] == []
    assert server.attempt_login("mallory", SECRET_PASSWORD).status_code == 401


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-019")
@allure.story("SCN-AUTHORIZATION-019-C")
def test_an_admin_cannot_touch_an_admin(server: Sut, root_token: str) -> None:
    """An Admin deleting an Admin peer or itself answers 403 and applies nothing."""
    admin_id, admin = provision(server, root_token, "admina", role="ADMIN")
    peer_id, peer = provision(server, root_token, "adminb", role="ADMIN")

    peer_deleted = server.request("DELETE", f"{server.href('user')}/{peer_id}", token=admin)
    self_deleted = server.request("DELETE", f"{server.href('user')}/{admin_id}", token=admin)

    assert peer_deleted.status_code == 403, f"peer delete allowed: {peer_deleted.status_code}"
    assert self_deleted.status_code == 403, f"self delete allowed: {self_deleted.status_code}"
    assert server.request("GET", server.href("current_user"), token=peer).status_code == 200
    assert server.request("GET", server.href("current_user"), token=admin).status_code == 200


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-019")
@allure.story("SCN-AUTHORIZATION-019-D")
def test_the_root_admin_deletes_an_admin(server: Sut, root_token: str) -> None:
    """The Root Admin deletes an Admin account and the account is gone."""
    admin_id, admin = provision(server, root_token, "admina", role="ADMIN")

    deleted = server.request("DELETE", f"{server.href('user')}/{admin_id}", token=root_token)

    assert deleted.status_code == 204, f"delete failed: {deleted.status_code} {deleted.text}"
    assert server.request("GET", f"{server.href('user')}/{admin_id}", token=root_token).status_code == 404
    assert server.request("GET", server.href("current_user"), token=admin).status_code == 401


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-019")
@allure.story("SCN-AUTHORIZATION-019-E")
def test_the_root_account_is_untouchable(server: Sut, root_token: str) -> None:
    """Deleting or updating account 0 answers 403 and applies nothing."""
    deleted = server.request("DELETE", f"{server.href('user')}/0", token=root_token)
    updated = server.request(
        "PATCH",
        f"{server.href('user')}/0",
        token=root_token,
        json={"username": "notroot"},
    )

    assert deleted.status_code == 403, f"root deleted: {deleted.status_code} {deleted.text}"
    assert updated.status_code == 403, f"root updated: {updated.status_code} {updated.text}"
    stored = server.request("GET", f"{server.href('user')}/0", token=root_token)
    assert stored.status_code == 200, f"the root account must survive: {stored.text}"
    assert stored.json()["username"] == ROOT_ADMIN_USERNAME


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-020")
@allure.story("SCN-AUTHORIZATION-020-A")
def test_an_admin_replaces_a_standard_users_grants(server: Sut, root_token: str) -> None:
    """An Admin replaces the grant set naming a Standard user and reads the replacement back."""
    _admin_id, admin = provision(server, root_token, "admina", role="ADMIN")
    alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, admin, "holiday")
    gallery_id = linked_id(gallery["_links"]["self"]["href"])
    policy = f"{server.href('permission')}/resource/{GALLERY}/{gallery_id}"

    replaced = server.request(
        "PUT",
        policy,
        token=admin,
        json={
            "visibility": PRIVATE,
            "grants": [{"principal": {"kind": "USER", "id": alice_id}, "role": VIEWER, "effect": "ALLOW"}],
        },
    )
    followed = server.request("GET", policy, token=admin)
    readable = server.request("GET", f"{server.href('gallery')}/{gallery_id}", token=alice)

    assert replaced.status_code == 200, f"replacement failed: {replaced.status_code} {replaced.text}"
    assert followed.status_code == 200, f"policy failed: {followed.status_code} {followed.text}"
    body = followed.json()
    assert body["visibility"] == PRIVATE
    assert [item["principal"] for item in body["grants"]] == [{"kind": "USER", "id": alice_id}]
    assert readable.status_code == 200, f"granted read failed: {readable.status_code} {readable.text}"


@allure.epic("Authorization")
@allure.feature("REQ-AUTHORIZATION-017")
@allure.story("SCN-AUTHORIZATION-017-A")
def test_the_root_administers_another_callers_private_gallery(server: Sut, root_token: str) -> None:
    """The Root Admin reads, appends media to and deletes another caller's private gallery."""
    _alice_id, alice = provision(server, root_token, "alice")
    gallery = create_gallery(server, alice, "holiday")
    image = (FIXTURE_DIRECTORY / "cover.png").read_bytes()
    file = upload(server, alice, "cover.png", image)

    read = server.request("GET", link_of(gallery, "self"), token=root_token)
    added = server.request(
        "POST",
        link_of(gallery, "items"),
        token=root_token,
        json={"type": "image", "media_id": linked_id(link_of(file, "image"))},
    )
    removed = server.request("DELETE", link_of(gallery, "self"), token=root_token)

    assert read.status_code == 200, f"root read failed: {read.status_code} {read.text}"
    assert read.json()["is_public"] is False
    assert added.status_code == 201, f"root append failed: {added.status_code} {added.text}"
    assert removed.status_code == 204, f"root delete failed: {removed.status_code} {removed.text}"
    assert server.request("GET", link_of(gallery, "self"), token=alice).status_code == 404


@allure.epic("Authorization")
@allure.feature("REQ-GALLERY-006")
@allure.story("SCN-GALLERY-006-C")
def test_an_admin_cannot_delete_a_readable_public_gallery(server: Sut, root_token: str) -> None:
    """An Admin deleting a readable public gallery answers 403 and the gallery survives."""
    _alice_id, alice = provision(server, root_token, "alice")
    _admin_id, admin = provision(server, root_token, "admina", role="ADMIN")
    gallery = create_gallery(server, alice, "holiday", is_public=True)

    deleted = server.request("DELETE", link_of(gallery, "self"), token=admin)
    kept = server.request("GET", link_of(gallery, "self"), token=alice)

    assert deleted.status_code == 403, f"gallery deleted: {deleted.status_code} {deleted.text}"
    assert kept.status_code == 200, f"the gallery must survive: {kept.status_code} {kept.text}"
