"""Black-box proof of the `authorization` capability."""

import json
from itertools import count

from conftest import (
    SEED_MEDIA_DIRECTORY,
    add_to_gallery,
    begin,
    create_gallery,
    provision,
    upload,
)
from harness import ROOT_ADMIN_USERNAME, SECRET_PASSWORD, Sut
from labels import scenario
from mnemorium_client.api.asset import get_file, get_upload
from mnemorium_client.api.identity import (
    delete_permission_share,
    get_permission_summary,
    get_resource_permission,
    post_login,
    post_permission_share,
    post_register,
    put_resource_permission,
)
from mnemorium_client.api.library import (
    delete_gallery,
    get_album,
    get_gallery,
    get_gallery_item,
    get_movie,
    get_music_group,
    get_playlist,
    get_series,
    list_albums,
    list_movies,
    list_music_groups,
    patch_album,
    patch_movie,
    patch_music_group,
    patch_playlist,
    post_album,
    post_gallery,
    post_gallery_item,
    post_movie,
    post_music_group,
    post_playlist,
)
from mnemorium_client.api.user import delete_user, get_me, get_user, list_users, update_user
from mnemorium_client.models import (
    AddPlaylistTrackRequest,
    ChildKindWire,
    GalleryItemType,
    GetPermissionSummaryAction,
    GetPermissionSummaryResourceKind,
    GrantEffectWire,
    GrantPayload,
    GrantRoleWire,
    LoginRequest,
    PatchAlbumRequest,
    PatchMovieRequest,
    PatchMusicGroupRequest,
    PatchPlaylistRequest,
    PatchUserRequest,
    PostAlbumRequest,
    PostGalleryItemRequest,
    PostGalleryRequest,
    PostMovieRequest,
    PostMusicGroupRequest,
    PostPlaylistRequest,
    PrincipalKindWire,
    PrincipalPayload,
    PutPermissionRequest,
    RegisterRequest,
    Role,
    ShareRequest,
    ShareRoleWire,
    VisibilityWire,
)

GALLERY = "GALLERY"
MUSIC_RECORDING = "MUSIC_RECORDING"
PLAYLIST_COLLECTION = "PLAYLIST_COLLECTION"
MOVIE = "MOVIE"

VIEWER = GrantRoleWire.VIEWER
EDITOR = GrantRoleWire.EDITOR
MANAGER = GrantRoleWire.MANAGER
CONTRIBUTOR = GrantRoleWire.CONTRIBUTOR

ALLOW = GrantEffectWire.ALLOW
DENY = GrantEffectWire.DENY
PRIVATE = VisibilityWire.PRIVATE
PUBLIC = VisibilityWire.PUBLIC

SEEDED_MOVIE = 13011
SEEDED_PRIVATE_MOVIE = 13012
SEEDED_GROUP = 11001
SEEDED_ALBUM = 11003
SEEDED_SERIES = 11014
SEEDED_RECORDING = 12301

_ACCOUNTS = count()


def _user(id: int) -> PrincipalPayload:
    """The principal naming one user."""
    return PrincipalPayload(kind=PrincipalKindWire.USER, id=id)


def _grant(
    id: int,
    role: GrantRoleWire,
    effect: GrantEffectWire = ALLOW,
    child_kind: ChildKindWire | None = None,
) -> GrantPayload:
    """One user grant at the resource role the case names."""
    return GrantPayload(
        principal=_user(id),
        role=role,
        effect=effect,
        child_kind=child_kind,
    )


def _resource_permission_href(server: Sut, gallery_id: int) -> str:
    """The permission route of one gallery, addressed outside the typed client."""
    return f"{server.href('permission')}/resource/GALLERY/{gallery_id}"


def _share_href(server: Sut, gallery_id: int) -> str:
    """The share route of one gallery, addressed outside the typed client."""
    return f"{_resource_permission_href(server, gallery_id)}/share"


@scenario("SCN-AUTHORIZATION-031-A")
def test_a_caller_reads_its_own_summary(server: Sut, root_token: str) -> None:
    """A Standard user reaches its own summary and receives no grant internals."""
    alice_id, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")

    response = get_permission_summary.sync_detailed(client=server.client(alice), user_id=alice_id)

    assert response.status_code == 200
    body = json.loads(response.content)
    assert body["user_id"] == alice_id
    assert body["role"] == "STANDARD"
    assert all("grant_id" not in item for item in body["grants"])


@scenario("SCN-AUTHORIZATION-031-B")
def test_an_admin_reads_a_standard_users_summary(server: Sut, root_token: str) -> None:
    """An Admin reaches a Standard user's summary."""
    bob_id, _bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    _, admin = provision(server, root_token, f"azadmin{next(_ACCOUNTS)}", role="ADMIN")

    response = get_permission_summary.sync_detailed(client=server.client(admin), user_id=bob_id)

    assert response.status_code == 200


@scenario("SCN-AUTHORIZATION-031-C")
def test_another_users_summary_is_forbidden(server: Sut, root_token: str) -> None:
    """A Standard user may not read another user's summary."""
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    carol_id, _carol = provision(server, root_token, f"azcarol{next(_ACCOUNTS)}")

    response = get_permission_summary.sync_detailed(client=server.client(bob), user_id=carol_id)

    assert response.status_code == 403


@scenario("SCN-AUTHORIZATION-031-D")
def test_the_explanation_is_returned_to_a_manager(server: Sut, root_token: str) -> None:
    """The Root Admin receives the decision and its explanation."""
    alice_id, _alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, bob, "explanation")

    response = get_permission_summary.sync_detailed(
        client=server.client(root_token),
        user_id=alice_id,
        resource_kind=GetPermissionSummaryResourceKind.GALLERY,
        resource_id=gallery["id"],
        action=GetPermissionSummaryAction.VIEW,
    )

    assert response.status_code == 200
    decision = json.loads(response.content)["decision"]
    assert decision["allowed"] is False
    assert decision["reason"] == "default"
    assert decision["auth_path"][0]["kind"] == GALLERY


@scenario("SCN-AUTHORIZATION-032-A")
def test_an_owner_reads_the_resource_permission(server: Sut, root_token: str) -> None:
    """The owner reads its gallery's visibility and grants."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    response = get_resource_permission.sync_detailed(GALLERY, gallery["id"], client=server.client(alice))

    assert response.status_code == 200
    body = json.loads(response.content)
    assert body["visibility"] == PRIVATE
    assert body["address"]["kind"] == GALLERY


@scenario("SCN-AUTHORIZATION-032-B")
@scenario("SCN-DOWNLOAD-002-D")
def test_an_owner_replaces_the_set(server: Sut, root_token: str) -> None:
    """A replacement stores exactly the submitted grant set, and the dropped recipient is concealed."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    replaced = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER)]),
    )
    followed = get_resource_permission.sync_detailed(GALLERY, gallery["id"], client=server.client(alice))
    granted = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))
    dropped = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[]),
    )
    concealed = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))

    assert replaced.status_code == 200
    assert followed.status_code == 200
    body = json.loads(followed.content)
    assert body["visibility"] == PRIVATE
    assert [item["role"] for item in body["grants"] if item["principal"]["id"] == bob_id] == [VIEWER]
    assert granted.status_code == 200, "the granted caller must read the gallery"
    assert dropped.status_code == 200
    assert concealed.status_code == 404, "the dropped caller must be concealed"


@scenario("SCN-AUTHORIZATION-032-C")
def test_a_manager_cannot_replace(server: Sut, root_token: str) -> None:
    """A caller holding Manager but not ManagePermissions may read but not replace."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    shared = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, MANAGER)]),
    )
    read = get_resource_permission.sync_detailed(GALLERY, gallery["id"], client=server.client(bob))
    replaced = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(bob),
        body=PutPermissionRequest(visibility=PUBLIC, grants=[]),
    )

    assert shared.status_code == 200
    assert read.status_code == 200
    assert replaced.status_code == 403


@scenario("SCN-AUTHORIZATION-032-D")
@scenario("SCN-AUTHORIZATION-034-A")
def test_a_rejected_replacement_leaves_the_previous_set(server: Sut, root_token: str) -> None:
    """A replacement naming the Owner role answers 422 and stores nothing."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, _bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    refused = server.request(
        "PUT",
        _resource_permission_href(server, gallery["id"]),
        token=alice,
        json={
            "visibility": "PUBLIC",
            "grants": [
                {
                    "principal": {"id": bob_id, "kind": "USER"},
                    "role": "OWNER",
                    "effect": "ALLOW",
                }
            ],
        },
    )
    followed = get_resource_permission.sync_detailed(GALLERY, gallery["id"], client=server.client(alice))

    assert refused.status_code == 422
    body = json.loads(followed.content)
    assert body["visibility"] == PRIVATE
    assert body["grants"] == []


@scenario("SCN-AUTHORIZATION-033-A")
def test_a_manager_shares_a_viewer(server: Sut, root_token: str) -> None:
    """An owner shares Viewer and the recipient can read the gallery."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    shared = post_permission_share.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=ShareRequest(principal=_user(bob_id), role=ShareRoleWire.VIEWER),
    )
    read = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))

    assert shared.status_code == 200
    assert read.status_code == 200


@scenario("SCN-AUTHORIZATION-033-B")
def test_sharing_cannot_escalate(server: Sut, root_token: str) -> None:
    """A share naming a role outside the published read roles answers 422 and stores nothing."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    shared = server.request(
        "POST",
        _share_href(server, gallery["id"]),
        token=alice,
        json={"principal": {"id": bob_id, "kind": "USER"}, "role": "EDITOR"},
    )
    read = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))

    assert shared.status_code == 422, "a role outside the published share vocabulary must be refused"
    assert read.status_code == 404


@scenario("SCN-AUTHORIZATION-033-C")
def test_a_share_is_revoked(server: Sut, root_token: str) -> None:
    """A revoked share stops reaching the resource."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")
    shared = post_permission_share.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=ShareRequest(principal=_user(bob_id), role=ShareRoleWire.VIEWER),
    )
    assert shared.status_code == 200
    grant_id = json.loads(shared.content)["grant_id"]

    revoked = delete_permission_share.sync_detailed(GALLERY, gallery["id"], grant_id, client=server.client(alice))
    read = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))

    assert revoked.status_code == 204
    assert read.status_code == 404


@scenario("SCN-AUTHORIZATION-034-B")
def test_permission_administration_is_refused(server: Sut, root_token: str) -> None:
    """A grant naming ManagePermissions answers 422."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, _bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    response = server.request(
        "PUT",
        _resource_permission_href(server, gallery["id"]),
        token=alice,
        json={
            "visibility": "PRIVATE",
            "grants": [
                {
                    "principal": {"id": bob_id, "kind": "USER"},
                    "role": "MANAGE_PERMISSIONS",
                    "effect": "ALLOW",
                }
            ],
        },
    )

    assert response.status_code == 422


@scenario("SCN-AUTHORIZATION-034-C")
def test_a_duplicate_is_refused(server: Sut, root_token: str) -> None:
    """A set naming the same grant twice answers 422."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, _bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")
    duplicate = _grant(bob_id, VIEWER)

    response = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[duplicate, duplicate]),
    )

    assert response.status_code == 422


@scenario("SCN-AUTHORIZATION-034-D")
def test_an_unknown_principal_is_refused(server: Sut, root_token: str) -> None:
    """A grant naming a user that does not exist answers 422."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    response = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(999_999, VIEWER)]),
    )

    assert response.status_code == 422


@scenario("SCN-AUTHORIZATION-034-E")
def test_admin_creation_is_refused(server: Sut, root_token: str) -> None:
    """A CreateChild grant naming the Admin child kind answers 422."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, _bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    response = server.request(
        "PUT",
        _resource_permission_href(server, gallery["id"]),
        token=alice,
        json={
            "visibility": "PRIVATE",
            "grants": [
                {
                    "principal": {"id": bob_id, "kind": "USER"},
                    "role": "CONTRIBUTOR",
                    "child_kind": "ADMIN",
                    "effect": "ALLOW",
                }
            ],
        },
    )

    assert response.status_code == 422


@scenario("SCN-AUTHORIZATION-035-A")
def test_a_role_grant_cannot_be_written(server: Sut, root_token: str) -> None:
    """A replacement naming a role principal answers 422."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    response = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(
            visibility=PRIVATE,
            grants=[
                GrantPayload(
                    principal=PrincipalPayload(
                        kind=PrincipalKindWire.ROLE,
                        role=Role.STANDARD,
                    ),
                    role=VIEWER,
                    effect=ALLOW,
                )
            ],
        ),
    )

    assert response.status_code == 422


@scenario("SCN-AUTHORIZATION-035-B")
def test_the_seeded_role_applies(server: Sut, root_token: str) -> None:
    """A Standard user with no per-resource grant creates a gallery through its seeded role."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")

    created = post_gallery.sync_detailed(client=server.client(alice), body=PostGalleryRequest(name="owned"))

    assert created.status_code == 201


@scenario("SCN-AUTHORIZATION-016-C")
def test_a_public_resource_is_readable_by_every_caller(server: Sut, root_token: str) -> None:
    """A public gallery is readable by a user with no grant."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday", is_public=True)

    response = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))

    assert response.status_code == 200


@scenario("SCN-AUTHORIZATION-014-A")
def test_a_deny_wins_over_a_grant(server: Sut, root_token: str) -> None:
    """A deny stored on the resource defeats a viewer grant."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    permitted = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER)]),
    )
    readable = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))
    denied = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(
            visibility=PRIVATE,
            grants=[_grant(bob_id, VIEWER), _grant(bob_id, VIEWER, effect=DENY)],
        ),
    )
    unreadable = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))
    stored = get_resource_permission.sync_detailed(GALLERY, gallery["id"], client=server.client(alice))
    restored = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[]),
    )

    assert permitted.status_code == 200
    assert readable.status_code == 200
    assert denied.status_code == 200
    assert unreadable.status_code == 404
    assert restored.status_code == 200
    assert all(grant["principal"]["kind"] == "USER" for grant in json.loads(stored.content)["grants"]), (
        "a replacement must not rewrite the seeded role grants"
    )


@scenario("SCN-AUTHORIZATION-021-A")
def test_a_caller_reads_its_own_account(server: Sut, root_token: str) -> None:
    """A caller reads its own account without a grant."""
    alice_id, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")

    response = get_user.sync_detailed(alice_id, client=server.client(alice))

    assert response.status_code == 200


@scenario("SCN-AUTHORIZATION-018-B")
def test_the_root_cannot_read_another_users_upload(server: Sut, root_token: str) -> None:
    """An upload is owner-only, even for the Root Admin."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    session = begin(server, alice, "cover.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())

    response = get_upload.sync_detailed(session.id, client=server.client(root_token))

    assert response.status_code == 404


@scenario("SCN-AUTHORIZATION-025-B")
def test_an_asset_is_reachable_through_its_gallery(server: Sut, root_token: str) -> None:
    """A gallery item's image is readable by the gallery owner and concealed from others."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")
    file = upload(server, alice, "cover.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())
    item = add_to_gallery(server, alice, gallery, file["id"])

    owner_read = get_gallery_item.sync_detailed(gallery["id"], item["id"], client=server.client(alice))
    stranger_read = get_gallery_item.sync_detailed(gallery["id"], item["id"], client=server.client(bob))

    assert owner_read.status_code == 200
    assert stranger_read.status_code == 404


@scenario("SCN-AUTHORIZATION-019-A")
def test_an_admin_creates_a_standard_account(server: Sut, root_token: str) -> None:
    """An Admin creates a Standard account, which then authenticates."""
    _, admin = provision(server, root_token, f"azadmin{next(_ACCOUNTS)}", role="ADMIN")
    username = f"azcreated{next(_ACCOUNTS)}"

    created = post_register.sync_detailed(
        client=server.client(admin),
        body=RegisterRequest(password=SECRET_PASSWORD, role=Role.STANDARD, username=username),
    )

    assert created.status_code == 201
    assert json.loads(created.content)["role"] == "STANDARD"
    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=SECRET_PASSWORD, identifier=username)
        ).status_code
        == 200
    )


@scenario("SCN-AUTHORIZATION-019-B")
@scenario("SCN-AUTHORIZATION-023-D")
def test_an_admin_cannot_create_an_admin(server: Sut, root_token: str) -> None:
    """Neither an Admin nor a Standard user creates an Admin account, while the root does."""
    _, admin = provision(server, root_token, f"azadmin{next(_ACCOUNTS)}", role="ADMIN")
    _, standard = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    refused_name = f"azmallory{next(_ACCOUNTS)}"

    for token in (admin, standard):
        refused = post_register.sync_detailed(
            client=server.client(token),
            body=RegisterRequest(password=SECRET_PASSWORD, role=Role.ADMIN, username=refused_name),
        )
        assert refused.status_code == 403

    listed = list_users.sync_detailed(client=server.client(root_token), username=refused_name)
    assert json.loads(listed.content)["items"] == []
    assert (
        post_login.sync_detailed(
            client=server.client(), body=LoginRequest(password=SECRET_PASSWORD, identifier=refused_name)
        ).status_code
        == 401
    )

    promoted_name = f"azpromoted{next(_ACCOUNTS)}"
    created = post_register.sync_detailed(
        client=server.client(root_token),
        body=RegisterRequest(password=SECRET_PASSWORD, role=Role.ADMIN, username=promoted_name),
    )

    assert created.status_code == 201
    assert json.loads(created.content)["role"] == "ADMIN"


@scenario("SCN-AUTHORIZATION-019-C")
def test_an_admin_cannot_touch_an_admin(server: Sut, root_token: str) -> None:
    """An Admin deleting an Admin peer or itself answers 403 and applies nothing."""
    admin_id, admin = provision(server, root_token, f"azadmina{next(_ACCOUNTS)}", role="ADMIN")
    peer_id, peer = provision(server, root_token, f"azadminb{next(_ACCOUNTS)}", role="ADMIN")

    peer_deleted = delete_user.sync_detailed(peer_id, client=server.client(admin))
    self_deleted = delete_user.sync_detailed(admin_id, client=server.client(admin))

    assert peer_deleted.status_code == 403
    assert self_deleted.status_code == 403
    assert get_me.sync_detailed(client=server.client(peer)).status_code == 200
    assert get_me.sync_detailed(client=server.client(admin)).status_code == 200


@scenario("SCN-AUTHORIZATION-019-D")
def test_the_root_admin_deletes_an_admin(server: Sut, root_token: str) -> None:
    """The Root Admin deletes an Admin account and the account is gone."""
    admin_id, admin = provision(server, root_token, f"azadmina{next(_ACCOUNTS)}", role="ADMIN")

    deleted = delete_user.sync_detailed(admin_id, client=server.client(root_token))

    assert deleted.status_code == 204
    assert get_user.sync_detailed(admin_id, client=server.client(root_token)).status_code == 404
    assert get_me.sync_detailed(client=server.client(admin)).status_code == 401


@scenario("SCN-AUTHORIZATION-019-E")
def test_the_root_account_is_untouchable(server: Sut, root_token: str) -> None:
    """Deleting or updating account 0 answers 403 and applies nothing."""
    deleted = delete_user.sync_detailed(0, client=server.client(root_token))
    updated = update_user.sync_detailed(
        0,
        client=server.client(root_token),
        body=PatchUserRequest(username="notroot"),
    )

    assert deleted.status_code == 403
    assert updated.status_code == 403
    stored = get_user.sync_detailed(0, client=server.client(root_token))
    assert stored.status_code == 200
    assert json.loads(stored.content)["username"] == ROOT_ADMIN_USERNAME


@scenario("SCN-AUTHORIZATION-020-A")
def test_an_admin_replaces_a_standard_users_grants(server: Sut, root_token: str) -> None:
    """An Admin replaces the grant set naming a Standard user and reads the replacement back."""
    _, admin = provision(server, root_token, f"azadmin{next(_ACCOUNTS)}", role="ADMIN")
    alice_id, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    gallery = create_gallery(server, admin, "holiday")

    replaced = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(admin),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(alice_id, VIEWER)]),
    )
    followed = get_resource_permission.sync_detailed(GALLERY, gallery["id"], client=server.client(admin))
    readable = get_gallery.sync_detailed(gallery["id"], client=server.client(alice))

    assert replaced.status_code == 200
    assert followed.status_code == 200
    body = json.loads(followed.content)
    assert body["visibility"] == PRIVATE
    assert [item["principal"] for item in body["grants"]] == [{"kind": "USER", "id": alice_id}]
    assert readable.status_code == 200


@scenario("SCN-AUTHORIZATION-017-A")
def test_the_root_administers_another_callers_private_gallery(server: Sut, root_token: str) -> None:
    """The Root Admin reads, appends media to and deletes another caller's private gallery."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")
    file = upload(server, alice, "cover.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())

    read = get_gallery.sync_detailed(gallery["id"], client=server.client(root_token))
    added = post_gallery_item.sync_detailed(
        gallery["id"],
        client=server.client(root_token),
        body=PostGalleryItemRequest(media_id=file["id"], type_=GalleryItemType.IMAGE),
    )
    removed = delete_gallery.sync_detailed(gallery["id"], client=server.client(root_token))

    assert read.status_code == 200
    assert json.loads(read.content)["is_public"] is False
    assert added.status_code == 201
    assert removed.status_code == 204
    assert get_gallery.sync_detailed(gallery["id"], client=server.client(alice)).status_code == 404


@scenario("SCN-GALLERY-006-C")
def test_an_admin_cannot_delete_a_readable_public_gallery(server: Sut, root_token: str) -> None:
    """An Admin deleting a readable public gallery answers 403 and the gallery survives."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, admin = provision(server, root_token, f"azadmin{next(_ACCOUNTS)}", role="ADMIN")
    gallery = create_gallery(server, alice, "holiday", is_public=True)

    deleted = delete_gallery.sync_detailed(gallery["id"], client=server.client(admin))
    kept = get_gallery.sync_detailed(gallery["id"], client=server.client(alice))

    assert deleted.status_code == 403
    assert kept.status_code == 200


@scenario("SCN-AUTHORIZATION-022-A")
def test_a_standard_user_reads_the_catalogue(server: Sut, root_token: str) -> None:
    """A Standard user with the seeded role reads a film, a show, a music group and an album."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")

    film = get_movie.sync_detailed(SEEDED_MOVIE, client=server.client(alice))
    show = get_series.sync_detailed(SEEDED_SERIES, client=server.client(alice))
    group = get_music_group.sync_detailed(SEEDED_GROUP, client=server.client(alice))
    album = get_album.sync_detailed(SEEDED_ALBUM, client=server.client(alice))

    assert film.status_code == 200
    assert show.status_code == 200
    assert group.status_code == 200
    assert album.status_code == 200


@scenario("SCN-AUTHORIZATION-022-B")
def test_a_standard_user_cannot_rename_the_catalogue(server: Sut, root_token: str) -> None:
    """A Standard user renaming a film, a music group or an album answers 403 and applies nothing."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    renamed = f"renamed{next(_ACCOUNTS)}"

    film = patch_movie.sync_detailed(
        SEEDED_MOVIE,
        client=server.client(alice),
        body=PatchMovieRequest(name=renamed),
    )
    group = patch_music_group.sync_detailed(
        SEEDED_GROUP,
        client=server.client(alice),
        body=PatchMusicGroupRequest(name=renamed),
    )
    album = patch_album.sync_detailed(
        SEEDED_ALBUM,
        client=server.client(alice),
        body=PatchAlbumRequest(name=renamed),
    )

    assert film.status_code == 403
    assert group.status_code == 403
    assert album.status_code == 403
    stored_film = get_movie.sync_detailed(SEEDED_MOVIE, client=server.client(alice))
    stored_group = get_music_group.sync_detailed(SEEDED_GROUP, client=server.client(alice))
    stored_album = get_album.sync_detailed(SEEDED_ALBUM, client=server.client(alice))
    assert json.loads(stored_film.content)["name"] == "Listed One"
    assert json.loads(stored_group.content)["name"] == "Covered Group"
    assert json.loads(stored_album.content)["name"] == "Covered Album"


@scenario("SCN-AUTHORIZATION-023-G")
def test_a_standard_user_cannot_create_a_music_group(server: Sut, root_token: str) -> None:
    """A Standard user creating a music group answers 403 and creates nothing."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    name = f"forbidden-group{next(_ACCOUNTS)}"

    created = post_music_group.sync_detailed(
        client=server.client(alice),
        body=PostMusicGroupRequest(name=name),
    )
    listed = list_music_groups.sync_detailed(client=server.client(root_token), name=name)

    assert created.status_code == 403
    assert json.loads(listed.content)["_embedded"]["items"] == []


@scenario("SCN-AUTHORIZATION-023-G")
def test_a_standard_user_cannot_create_a_music_album(server: Sut, root_token: str) -> None:
    """A Standard user creating a music album answers 403 and creates nothing."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    name = f"forbidden-album{next(_ACCOUNTS)}"

    created = post_album.sync_detailed(
        client=server.client(alice),
        body=PostAlbumRequest(name=name, music_group_id=SEEDED_GROUP),
    )
    listed = list_albums.sync_detailed(client=server.client(root_token), name=name)

    assert created.status_code == 403
    assert json.loads(listed.content)["_embedded"]["items"] == []


@scenario("SCN-AUTHORIZATION-023-B")
def test_a_standard_user_cannot_create_a_film(server: Sut, root_token: str) -> None:
    """A Standard user creating a film answers 403 and creates nothing."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    name = f"forbidden-film{next(_ACCOUNTS)}"

    created = post_movie.sync_detailed(
        client=server.client(alice),
        body=PostMovieRequest(name=name, video_id=13212),
    )
    listed = list_movies.sync_detailed(client=server.client(root_token), name=name)

    assert created.status_code == 403
    assert json.loads(listed.content)["_embedded"]["items"] == []


@scenario("SCN-AUTHORIZATION-023-F")
def test_a_denied_playlist_surface_refuses_creation(server: Sut, root_token: str) -> None:
    """A denied CreateChild on the playlist collection refuses creation and leaves the seeded role intact."""
    alice_id, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    current = get_resource_permission.sync_detailed(PLAYLIST_COLLECTION, 0, client=server.client(root_token))
    assert current.status_code == 200
    visibility = VisibilityWire(json.loads(current.content)["visibility"])

    denied = put_resource_permission.sync_detailed(
        PLAYLIST_COLLECTION,
        0,
        client=server.client(root_token),
        body=PutPermissionRequest(
            visibility=visibility,
            grants=[_grant(alice_id, CONTRIBUTOR, effect=DENY, child_kind=ChildKindWire.MUSIC_PLAYLIST)],
        ),
    )
    refused = post_playlist.sync_detailed(client=server.client(alice), body=PostPlaylistRequest(name="denied-surface"))
    restored = put_resource_permission.sync_detailed(
        PLAYLIST_COLLECTION,
        0,
        client=server.client(root_token),
        body=PutPermissionRequest(visibility=visibility, grants=[]),
    )
    allowed = post_playlist.sync_detailed(
        client=server.client(alice), body=PostPlaylistRequest(name="restored-surface")
    )

    assert denied.status_code == 200
    assert refused.status_code == 403
    assert restored.status_code == 200
    assert allowed.status_code == 201, "the seeded role grant must survive the deny"


@scenario("SCN-AUTHORIZATION-040-A")
def test_an_unreadable_recording_refuses_the_append(server: Sut, root_token: str) -> None:
    """A caller that cannot read the named recording answers 403 and appends nothing."""
    alice_id, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    playlist = post_playlist.sync_detailed(
        client=server.client(alice), body=PostPlaylistRequest(name="unreadable-recording")
    )
    assert playlist.status_code == 201
    playlist_id = json.loads(playlist.content)["id"]
    current = get_resource_permission.sync_detailed(MUSIC_RECORDING, SEEDED_RECORDING, client=server.client(root_token))
    assert current.status_code == 200
    visibility = VisibilityWire(json.loads(current.content)["visibility"])

    denied = put_resource_permission.sync_detailed(
        MUSIC_RECORDING,
        SEEDED_RECORDING,
        client=server.client(root_token),
        body=PutPermissionRequest(visibility=visibility, grants=[_grant(alice_id, VIEWER, effect=DENY)]),
    )
    refused = patch_playlist.sync_detailed(
        playlist_id,
        client=server.client(alice),
        body=PatchPlaylistRequest(add_track=AddPlaylistTrackRequest(recording_id=SEEDED_RECORDING)),
    )
    restored = put_resource_permission.sync_detailed(
        MUSIC_RECORDING,
        SEEDED_RECORDING,
        client=server.client(root_token),
        body=PutPermissionRequest(visibility=visibility, grants=[]),
    )
    stored = get_playlist.sync_detailed(playlist_id, client=server.client(alice))

    assert denied.status_code == 200
    assert refused.status_code == 403
    assert restored.status_code == 200
    body = json.loads(stored.content)
    assert body["track_count"] == 0
    assert body["tracks"] == []


@scenario("SCN-AUTHORIZATION-039-A")
def test_a_filtered_film_listing_returns_discoverable_films(server: Sut, root_token: str) -> None:
    """A film hidden from one caller is absent from their listing and present for the root."""
    alice_id, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    current = get_resource_permission.sync_detailed(MOVIE, SEEDED_PRIVATE_MOVIE, client=server.client(root_token))
    assert current.status_code == 200
    visibility = VisibilityWire(json.loads(current.content)["visibility"])

    denied = put_resource_permission.sync_detailed(
        MOVIE,
        SEEDED_PRIVATE_MOVIE,
        client=server.client(root_token),
        body=PutPermissionRequest(visibility=visibility, grants=[_grant(alice_id, VIEWER, effect=DENY)]),
    )
    filtered = list_movies.sync_detailed(client=server.client(alice), limit=100)
    everything = list_movies.sync_detailed(client=server.client(root_token), limit=100)
    restored = put_resource_permission.sync_detailed(
        MOVIE,
        SEEDED_PRIVATE_MOVIE,
        client=server.client(root_token),
        body=PutPermissionRequest(visibility=visibility, grants=[]),
    )

    assert denied.status_code == 200
    visible = [item["id"] for item in json.loads(filtered.content)["_embedded"]["items"]]
    assert SEEDED_MOVIE in visible
    assert SEEDED_PRIVATE_MOVIE not in visible
    assert SEEDED_PRIVATE_MOVIE in [item["id"] for item in json.loads(everything.content)["_embedded"]["items"]]
    assert restored.status_code == 200


@scenario("SCN-AUTHORIZATION-001-C")
def test_a_hidden_resource_is_concealed(server: Sut, root_token: str) -> None:
    """A denied film answers 404 for a read and a write, and another user's private file answers 404."""
    alice_id, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    current = get_resource_permission.sync_detailed(MOVIE, SEEDED_MOVIE, client=server.client(root_token))
    assert current.status_code == 200
    visibility = VisibilityWire(json.loads(current.content)["visibility"])

    denied = put_resource_permission.sync_detailed(
        MOVIE,
        SEEDED_MOVIE,
        client=server.client(root_token),
        body=PutPermissionRequest(visibility=visibility, grants=[_grant(alice_id, VIEWER, effect=DENY)]),
    )
    read = get_movie.sync_detailed(SEEDED_MOVIE, client=server.client(alice))
    write = patch_movie.sync_detailed(
        SEEDED_MOVIE,
        client=server.client(alice),
        body=PatchMovieRequest(name="concealed-rename"),
    )
    restored = put_resource_permission.sync_detailed(
        MOVIE,
        SEEDED_MOVIE,
        client=server.client(root_token),
        body=PutPermissionRequest(visibility=visibility, grants=[]),
    )
    private = upload(server, bob, "private.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())
    concealed = get_file.sync_detailed(private["id"], client=server.client(alice))

    assert denied.status_code == 200
    assert read.status_code == 404, "a denied film must be concealed from a read"
    assert write.status_code == 404, "a denied film must be concealed from a write"
    assert restored.status_code == 200
    assert concealed.status_code == 404, "another user's private file must be concealed"
