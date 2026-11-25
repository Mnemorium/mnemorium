"""Black-box proof of the `authorization` capability."""

import json
from itertools import count

from conftest import (
    SEED_MEDIA_DIRECTORY,
    add_to_gallery,
    create_gallery,
    provision,
    upload,
)
from harness import Sut
from labels import TestType, business_rule, scenario, security
from mnemorium_client.api.asset import get_file, get_file_content
from mnemorium_client.api.identity import (
    delete_permission_share,
    get_permission_summary,
    get_resource_permission,
    post_permission_share,
    put_resource_permission,
)
from mnemorium_client.api.library import (
    delete_gallery,
    get_gallery,
    get_gallery_item,
    get_movie,
    get_playlist,
    list_galleries,
    list_movies,
    patch_movie,
    patch_playlist,
    post_gallery,
    post_gallery_item,
    post_playlist,
)
from mnemorium_client.api.user import get_user, update_user
from mnemorium_client.models import (
    AddPlaylistTrackRequest,
    ChildKindWire,
    GalleryItemType,
    GetPermissionSummaryAction,
    GetPermissionSummaryResourceKind,
    GrantEffectWire,
    GrantPayload,
    GrantRoleWire,
    PatchMovieRequest,
    PatchPlaylistRequest,
    PatchUserRequest,
    PostGalleryItemRequest,
    PostGalleryRequest,
    PostPlaylistRequest,
    PrincipalKindWire,
    PrincipalPayload,
    PutPermissionRequest,
    Role,
    ShareRequest,
    ShareRoleWire,
    VisibilityWire,
)

GALLERY = "GALLERY"
MUSIC_RECORDING = "MUSIC_RECORDING"
MOVIE = "MOVIE"

VIEWER = GrantRoleWire.VIEWER
EDITOR = GrantRoleWire.EDITOR
MANAGER = GrantRoleWire.MANAGER

ALLOW = GrantEffectWire.ALLOW
DENY = GrantEffectWire.DENY
PRIVATE = VisibilityWire.PRIVATE
PUBLIC = VisibilityWire.PUBLIC
RESTRICTED = VisibilityWire.RESTRICTED

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


@security
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


@security
@scenario("SCN-AUTHORIZATION-031-B")
def test_an_admin_reads_a_standard_users_summary(server: Sut, root_token: str) -> None:
    """An Admin reaches a Standard user's summary."""
    bob_id, _bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    _, admin = provision(server, root_token, f"azadmin{next(_ACCOUNTS)}", role="ADMIN")

    response = get_permission_summary.sync_detailed(client=server.client(admin), user_id=bob_id)

    assert response.status_code == 200


@security
@scenario("SCN-AUTHORIZATION-031-C")
def test_another_users_summary_is_forbidden(server: Sut, root_token: str) -> None:
    """A Standard user may not read another user's summary."""
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    carol_id, _carol = provision(server, root_token, f"azcarol{next(_ACCOUNTS)}")

    response = get_permission_summary.sync_detailed(client=server.client(bob), user_id=carol_id)

    assert response.status_code == 403


@business_rule
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


@security
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


@security
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


@security
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


@security
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


@security
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


@security
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


@security
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


@security
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


@security
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


@security
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


@security
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


@security
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


@security
@scenario("SCN-AUTHORIZATION-035-B")
def test_the_seeded_role_applies(server: Sut, root_token: str) -> None:
    """A Standard user with no per-resource grant creates a gallery through its seeded role."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")

    created = post_gallery.sync_detailed(client=server.client(alice), body=PostGalleryRequest(name="owned"))

    assert created.status_code == 201


@security
@scenario("SCN-AUTHORIZATION-016-C")
def test_a_public_resource_is_readable_by_every_caller(server: Sut, root_token: str) -> None:
    """A gallery published through the permission endpoint is readable by a user with no grant."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")
    published = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PUBLIC, grants=[]),
    )

    response = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))

    assert published.status_code == 200
    assert response.status_code == 200
    assert json.loads(response.content)["is_public"] is True, "the flag must mirror the resource visibility"


@security
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


@security
@scenario("SCN-AUTHORIZATION-025-B")
def test_an_asset_is_reachable_through_its_gallery(server: Sut, root_token: str) -> None:
    """A gallery item's image is readable by its owner and by a caller granted the gallery, and concealed otherwise."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    _, carol = provision(server, root_token, f"azcarol{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")
    file = upload(server, alice, "cover.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())
    item = add_to_gallery(server, alice, gallery, file["id"])

    owner_read = get_gallery_item.sync_detailed(gallery["id"], item["id"], client=server.client(alice))
    stranger_read = get_gallery_item.sync_detailed(gallery["id"], item["id"], client=server.client(bob))

    assert owner_read.status_code == 200
    assert stranger_read.status_code == 404

    shared = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER)]),
    )
    granted_item = get_gallery_item.sync_detailed(gallery["id"], item["id"], client=server.client(bob))
    granted_file = get_file.sync_detailed(item["file_id"], client=server.client(bob))
    unshared_file = get_file.sync_detailed(item["file_id"], client=server.client(carol))

    assert shared.status_code == 200
    assert granted_item.status_code == 200, "the granted caller must read the item"
    assert granted_file.status_code == 200, "the granted caller must read the item's asset"
    assert unshared_file.status_code == 404, "a caller without a grant must not read the item's asset"

    asset = json.loads(granted_file.content)
    image = server.follow(asset, "image", token=bob)
    assert image.status_code == 200, f"image read failed: {image.status_code} {image.text!r}"
    content = server.request("GET", json.loads(image.content)["_links"]["content"]["href"], token=bob)
    assert content.status_code == 200, f"image content failed: {content.status_code} {content.content!r}"


@security
@scenario("SCN-AUTHORIZATION-016-E")
def test_visibility_is_not_inherited(server: Sut, root_token: str) -> None:
    """A public gallery lends the gallery and its items, never the private asset behind an item."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "open", is_public=True)
    file = upload(server, alice, "public.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())
    item = add_to_gallery(server, alice, gallery, file["id"])

    gallery_read = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))
    item_read = get_gallery_item.sync_detailed(gallery["id"], item["id"], client=server.client(bob))
    asset_read = get_file.sync_detailed(item["file_id"], client=server.client(bob))

    assert gallery_read.status_code == 200, "a public gallery is readable by every authenticated caller"
    assert item_read.status_code == 200, "an item of a public gallery is readable"
    assert asset_read.status_code == 404, "public visibility on the gallery must not lend the item's asset"


@security
@scenario("SCN-AUTHORIZATION-025-C")
def test_an_orphan_asset_is_owner_only(server: Sut, root_token: str) -> None:
    """An uploaded asset that nothing names is readable by its uploader and concealed from everyone else."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    file = upload(server, bob, "orphan.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())

    owner_read = get_file.sync_detailed(file["id"], client=server.client(bob))
    stranger_read = get_file.sync_detailed(file["id"], client=server.client(alice))

    assert owner_read.status_code == 200, "the uploader reads its own orphaned asset"
    assert stranger_read.status_code == 404, "an orphaned asset is owner-only"


@security
@scenario("SCN-AUTHORIZATION-005-A")
def test_a_user_grant_applies_to_one_user(server: Sut, root_token: str) -> None:
    """A grant naming one user reaches that user and no other caller of the same role."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    _, carol = provision(server, root_token, f"azcarol{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    shared = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER)]),
    )
    granted = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))
    other = get_gallery.sync_detailed(gallery["id"], client=server.client(carol))

    assert shared.status_code == 200
    assert granted.status_code == 200, "the named user must receive the grant"
    assert other.status_code == 404, "a caller of the same role must not receive it"


@security
@scenario("SCN-AUTHORIZATION-007-A")
def test_restricted_visibility_separates_discovery_from_content(server: Sut, root_token: str) -> None:
    """A Restricted gallery is discoverable in a listing while its content stays refused."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    _, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "restricted")
    restricted = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=RESTRICTED, grants=[]),
    )
    listed = list_galleries.sync_detailed(client=server.client(bob), name="restricted", limit=100)
    read = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))
    restored = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[]),
    )

    assert restricted.status_code == 200
    assert gallery["id"] in [item["id"] for item in json.loads(listed.content)["items"]]
    assert read.status_code == 404, "Restricted lends discovery and metadata, not the content read"
    assert restored.status_code == 200


@security
@scenario("SCN-AUTHORIZATION-008-B")
def test_metadata_does_not_change_visibility(server: Sut, root_token: str) -> None:
    """An Editor renames a playlist while the visibility change needs ManagePermissions."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    created = post_playlist.sync_detailed(
        client=server.client(alice),
        body=PostPlaylistRequest(name="shared"),
    )
    playlist = json.loads(created.content)
    shared = put_resource_permission.sync_detailed(
        "MUSIC_PLAYLIST",
        playlist["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, EDITOR)]),
    )

    renamed = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(bob),
        body=PatchPlaylistRequest(name="renamed"),
    )
    refused_visibility = patch_playlist.sync_detailed(
        playlist["id"],
        client=server.client(bob),
        body=PatchPlaylistRequest(is_public=True),
    )
    refused = put_resource_permission.sync_detailed(
        "MUSIC_PLAYLIST",
        playlist["id"],
        client=server.client(bob),
        body=PutPermissionRequest(visibility=PUBLIC, grants=[]),
    )

    assert created.status_code == 201
    assert shared.status_code == 200
    assert renamed.status_code == 200, "EditMetadata must apply the metadata write"
    assert refused_visibility.status_code == 403, "changing visibility needs ManagePermissions"
    assert refused.status_code == 403, "ManagePermissions alone changes visibility"


@security
@scenario("SCN-AUTHORIZATION-009-B")
def test_administration_grants_a_higher_role(server: Sut, root_token: str) -> None:
    """The owner stores a Manager grant through the permission endpoint."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, _bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    stored = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, MANAGER)]),
    )
    read = get_resource_permission.sync_detailed(GALLERY, gallery["id"], client=server.client(alice))

    assert stored.status_code == 200
    roles = [item["role"] for item in json.loads(read.content)["grants"] if item["principal"]["id"] == bob_id]
    assert roles == [MANAGER], "the higher role must be stored as submitted"


@security
@scenario("SCN-AUTHORIZATION-011-A")
def test_a_viewer_reads_but_cannot_download(server: Sut, root_token: str) -> None:
    """A Viewer grant reads the gallery, its item and the asset, but refuses the download."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")
    file = upload(server, alice, "viewer.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())
    item = add_to_gallery(server, alice, gallery, file["id"])
    shared = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER)]),
    )

    read_gallery = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))
    read_item = get_gallery_item.sync_detailed(gallery["id"], item["id"], client=server.client(bob))
    read_asset = get_file.sync_detailed(item["file_id"], client=server.client(bob))
    download = get_file_content.sync_detailed(item["file_id"], client=server.client(bob))

    assert shared.status_code == 200
    assert read_gallery.status_code == 200
    assert read_item.status_code == 200
    assert read_asset.status_code == 200, "Viewer includes the content read"
    assert download.status_code == 403, "Viewer does not include Download"


@security
@scenario("SCN-AUTHORIZATION-011-B")
def test_a_manager_shares_and_deletes(server: Sut, root_token: str) -> None:
    """A Manager shares a Viewer and deletes the gallery, without administering it."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    carol_id, _carol = provision(server, root_token, f"azcarol{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")
    granted = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, MANAGER)]),
    )

    shared = post_permission_share.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(bob),
        body=ShareRequest(principal=_user(carol_id), role=ShareRoleWire.VIEWER),
    )
    administered = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(bob),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[]),
    )
    removed = delete_gallery.sync_detailed(gallery["id"], client=server.client(bob))

    assert granted.status_code == 200
    assert shared.status_code == 200, "Manager includes Share"
    assert administered.status_code == 403, "Manager does not include ManagePermissions"
    assert removed.status_code == 204, "Manager includes Delete"


@security
@scenario("SCN-AUTHORIZATION-012-B")
def test_catalogue_resources_have_no_owner(server: Sut, root_token: str) -> None:
    """A film, show, album and group report no owning user."""
    for kind, identifier in (
        ("MOVIE", SEEDED_MOVIE),
        ("SERIES", SEEDED_SERIES),
        ("MUSIC_ALBUM", SEEDED_ALBUM),
        ("MUSIC_GROUP", SEEDED_GROUP),
    ):
        response = get_resource_permission.sync_detailed(kind, identifier, client=server.client(root_token))
        assert response.status_code == 200, f"{kind} permission failed: {response.content!r}"
        assert "owner_id" not in json.loads(response.content), f"{kind} must not be owned"


@security
@scenario("SCN-AUTHORIZATION-013-B")
@scenario("SCN-AUTHORIZATION-016-F")
def test_a_deny_withholds_its_role(server: Sut, root_token: str) -> None:
    """A Deny grant withholds the viewer actions with no allow competing with it."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "holiday")

    denied = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PUBLIC, grants=[_grant(bob_id, VIEWER, effect=DENY)]),
    )
    read = get_gallery.sync_detailed(gallery["id"], client=server.client(bob))

    assert denied.status_code == 200
    assert read.status_code == 404, "the deny must withhold the role even under public visibility"


@security
@scenario("SCN-AUTHORIZATION-014-B")
@scenario("SCN-AUTHORIZATION-014-C")
def test_a_deny_wins_across_the_chain(server: Sut, root_token: str) -> None:
    """A deny on the gallery and a deny on its item each defeat the allow on the other."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "chain")
    file = upload(server, alice, "chain.png", (SEED_MEDIA_DIRECTORY / "file-linked.png").read_bytes())
    item = add_to_gallery(server, alice, gallery, file["id"])

    allowed_item = put_resource_permission.sync_detailed(
        "GALLERY_ITEM",
        item["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER)]),
    )
    denied_gallery = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER, effect=DENY)]),
    )
    inherited_deny = get_file.sync_detailed(item["file_id"], client=server.client(bob))

    allowed_gallery = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER)]),
    )
    denied_item = put_resource_permission.sync_detailed(
        "GALLERY_ITEM",
        item["id"],
        client=server.client(alice),
        body=PutPermissionRequest(visibility=PRIVATE, grants=[_grant(bob_id, VIEWER, effect=DENY)]),
    )
    child_deny = get_file.sync_detailed(item["file_id"], client=server.client(bob))

    assert allowed_item.status_code == 200
    assert denied_gallery.status_code == 200
    assert inherited_deny.status_code == 404, "an ancestor deny must defeat the item's allow"
    assert allowed_gallery.status_code == 200
    assert denied_item.status_code == 200
    assert child_deny.status_code == 404, "a deny on the item must defeat the gallery's allow"


@security
@scenario("SCN-AUTHORIZATION-029-A")
@scenario("SCN-AUTHORIZATION-029-B")
def test_the_explanation_names_the_grant_or_the_deny(server: Sut, root_token: str) -> None:
    """The decision carries the granting reason, and the denying grant when one matched."""
    _, alice = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    bob_id, _bob = provision(server, root_token, f"azbobby{next(_ACCOUNTS)}")
    carol_id, _carol = provision(server, root_token, f"azcarol{next(_ACCOUNTS)}")
    gallery = create_gallery(server, alice, "explained")
    stored = put_resource_permission.sync_detailed(
        GALLERY,
        gallery["id"],
        client=server.client(alice),
        body=PutPermissionRequest(
            visibility=PRIVATE,
            grants=[_grant(bob_id, VIEWER), _grant(carol_id, VIEWER, effect=DENY)],
        ),
    )

    allowed = get_permission_summary.sync_detailed(
        client=server.client(root_token),
        user_id=bob_id,
        resource_kind=GetPermissionSummaryResourceKind.GALLERY,
        resource_id=gallery["id"],
        action=GetPermissionSummaryAction.VIEW,
    )
    denied = get_permission_summary.sync_detailed(
        client=server.client(root_token),
        user_id=carol_id,
        resource_kind=GetPermissionSummaryResourceKind.GALLERY,
        resource_id=gallery["id"],
        action=GetPermissionSummaryAction.VIEW,
    )

    assert stored.status_code == 200
    assert json.loads(allowed.content)["decision"]["reason"] == "grant"
    refusal = json.loads(denied.content)["decision"]
    assert refusal["reason"] == "deny"
    assert refusal["denied_by"] is not None


@security
@scenario("SCN-AUTHORIZATION-030-B")
def test_a_role_change_needs_both_actions(server: Sut, root_token: str) -> None:
    """An Admin renaming a Standard account and promoting it in one request applies neither."""
    _, admin = provision(server, root_token, f"azadmin{next(_ACCOUNTS)}", role="ADMIN")
    user_id, _user = provision(server, root_token, f"azalice{next(_ACCOUNTS)}")
    before = json.loads(get_user.sync_detailed(user_id, client=server.client(root_token)).content)
    renamed = f"{before['username']}x"

    updated = update_user.sync_detailed(
        user_id,
        client=server.client(admin),
        body=PatchUserRequest(username=renamed, role=Role.ADMIN),
    )
    stored = json.loads(get_user.sync_detailed(user_id, client=server.client(root_token)).content)

    assert updated.status_code == 403
    assert stored["username"] == before["username"], "the permitted part must not apply"
    assert stored["role"] == "STANDARD"


@security
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


@security
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


@security
@scenario("SCN-AUTHORIZATION-040-A")
@scenario("SCN-MUSIC-025-B", TestType.SECURITY)
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


@business_rule
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


@security
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
