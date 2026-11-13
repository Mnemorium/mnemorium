# Design

## Context

See `proposal.md` — Why.

The user area currently holds two capabilities, `user/account-creation` and `user/account-deletion`, and neither covers
the account as a resource. The code surface is wider than either spec:

- `src/lib/infrastructure/inbound/rest/handler/user/` serves `GET /user/me`, `GET /user/{id}`, `PATCH /user/{id}`,
  `DELETE /user/{id}` and `GET /user`; only the register route (`POST /api/v1/identity/register`) and the delete route
  have a capability today.
- `UpdateUser` applies the same `UsernamePolicy` and email parser as `RegisterUser`, checks uniqueness on rename, and
  uses `user.update` for the profile and `user.role_change` for the role.
- `Authorization::allows` carries three user rules that no spec states: a self-read short-circuit for `user.read`, an
  update relation guard for a privileged account, and a refusal for the account with identifier `0`.

The repository's naming convention is area-scoped (`user/account-creation`), but a capability at
`openspec/specs/user/spec.md` resolves to the id `user` and is the direct sibling of `authentication`, which the
previous change placed at the top level for the same reason: the capability names the behaviour, not the URL or the
package.

## Goals / Non-Goals

**Goals:**

- One capability owns the account contract: attributes, profile, and the create, read, list, self, update and delete
  operations.
- The two child capabilities retire, with their account semantics moved and their files deleted at archive.
- Every requirement carries `REQ-USER-*` and every scenario `SCN-USER-*`, so the capability is indexable like
  `authentication`.
- The authorization boundary is explicit: nothing in the user spec decides who may act, and the discarded rules are
  handed to the authorization workstream by name.

**Non-Goals:**

- Any authorization requirement, reference, actor, `401`/`403` outcome or concealment rule. The authorization capability
  is being specified separately.
- Any API change beyond case-insensitive email uniqueness, and any code change beyond the datastore collation and check
  in Decisions 8 and 9 and the conditional HAL extraction in Decision 10.
- Fixing the remaining gaps the spec exposes: the unpaginated collection, the missing way to clear an email, and the
  absence of self-service profile update.
- Retrofitting `REQ-*` identifiers into other main specs.
- Changing `docs/development/UseCases.md` or `docs/Permissions.md`.

## Decisions

### 1. One flat `user` capability, not a third sibling

**Decision:** create `openspec/specs/user/spec.md` as capability `user` and retire both child capabilities.

The account is one resource with one contract; creation and deletion are two of its operations, not two capabilities.
Keeping them as siblings would leave the shared attribute rules (`REQ-USER-003`, `REQ-USER-004`) owned by whichever
operation happened to be specified first — the same ownership inversion that `authentication` Decision 8 rejected for
the password policy.

**Alternative considered:** keep `user/account-creation` and `user/account-deletion` and add `user/account` for the
read/list/update surface. Rejected: three specs would state one resource's rules, and the new spec would depend on a
creation spec for the username and email rules.

**Migration:** the new capability is added with every requirement; the two child capabilities are removed with their
whole requirement sets. Both new and removed files live in this change's `specs/` tree, and the archive applies the
deletions and deletes the emptied specs.

### 2. Retire through deltas, never by deleting files

**Decision:** express each child capability's retirement as `## REMOVED Requirements` entries with a **Reason** and
**Migration**, one per requirement.

The archive tool removes the requirements from the main specs, deletes a main spec when its last requirement is gone,
and keeps the reason in the archive. Deleting the files by hand would lose that trail.

**Alternative considered:** delete the two files in this change. Rejected: the archive would see a capability vanish
with no record, and a reviewer could not tell whether the requirements moved or were dropped.

### 3. Authorization is excluded and handed off by name

**Decision:** no requirement in `user/spec.md` decides who may act, and the discarded rules are listed here for the
authorization workstream. This design is the handoff record.

The list to carry forward:

| Source                 | Discarded rule                                                                                                                                 |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| account-creation       | every creation needs a bearer token and `user.create`                                                                                          |
| account-creation       | creating an `ADMIN` needs `user.create_admin`; creating a `STANDARD` needs `user.create`; the role-grant matrix and its override case          |
| account-deletion       | every deletion needs a bearer token and `user.delete`; Standard `NONE`, Admin `ALL`, Root Admin always                                         |
| account-deletion       | a non-root caller may not delete an Admin account; the account with identifier `0` is never deletable                                          |
| code, `Permissions.md` | `user.read` short-circuits to allow a caller to read its own account, even when the preset is `NONE`                                           |
| code, `Permissions.md` | a non-root caller may not update a different privileged account; the caller's own account is excepted                                          |
| code, `Permissions.md` | the account with identifier `0` is never updatable, by anyone                                                                                  |
| code, `Permissions.md` | `user.read` is decided against `Global` for the collection and against the target for one account; denial concealment follows the uniform rule |

The post-deletion `401` is the one rule deliberately left with neither capability. Both child specs state it, the
authorization proposal in flight as `specify-authorization` records it and does not absorb it — it treats the `401` as a
consequence of account deletion rather than a permission decision — and this change excludes identity and token outcomes
from the account contract. It stays implemented and is asserted by the UC-005 journey; neither the handoff table nor the
`user` capability claims it.

**Alternative considered:** move the rules into `identity/authorization` in this change. Rejected: that capability is
being specified separately, and two changes writing one spec would conflict. The handoff list is the input to that work.

**Risk:** until that workstream lands, these rules are unowned. The list above is the mitigation; the authorization
change must absorb it.

### 4. Requirements are actor-free

**Decision:** write every requirement and scenario from the account's point of view: "a creation request carrying valid
values creates the account", not "an Admin creates an account".

Actors, tokens, scopes and denials belong to the other two capabilities. The system suite still needs a caller to reach
the endpoint; it provisions one in setup, but the scenario asserts the account outcome, not the gate.

**Alternative considered:** keep the actors and reference `identity/authorization` for the gate. Rejected: it reinports
the vocabulary the boundary just excluded, and a reference to a capability whose requirements are still being rewritten
would rot.

### 5. Capture current behaviour, correcting only the email rules

**Decision:** the spec states what the server does today, except for the two email corrections in Decisions 8 and 9. The
remaining gaps it exposes are recorded here and left for a separate change:

- `GET /user` returns every match and has no pagination, unlike the library collections.
- `PATCH /user/{id}` sets an email but has no way to clear one: an omitted field means "unchanged", and an empty string
  is invalid.
- A Standard user cannot update its own profile; `/me` is read-only.
- Role and profile fields travel in one `PATCH` and commit together.

**Rationale:** the change is a contract capture, and the authorization workstream may rename the caller story for
self-service. Fixing the API now would mix a behaviour change into a spec consolidation and make the suite red for a
reason the change did not set out to address.

**Alternative considered:** add pagination and email clearing in this change. Rejected: both are observable API changes
that need their own proposal, tests and compatibility note.

### 6. Prove the scenarios in `test/system/user.py`, and move replaced cases

**Decision:** a new `test/system/user.py` proves the capability's scenarios black box, following the pattern
`test/system/authentication.py` established: one module per feature area, `Sut` from the shared `harness` plugin, and
one `story` label per scenario.

The account-outcome cases in `test/e2e/user/uc001_create_a_user_account.py` and
`test/e2e/user/uc005_delete_a_user_account.py` move to the system suite and are deleted there. Their authorization cases
(unauthenticated access, the forbidden Admin creations, the Root and peer-Admin refusals) stay with the use-case journey
until the authorization workstream takes them, because no account scenario proves them. The UC-005 journey also keeps
the post-deletion `401` and login assertions that Decision 3 leaves with neither capability; if the account-outcome case
is deleted whole, those assertions move into a case of their own rather than disappearing with it.

Conformance stays with Schemathesis: a declared status, content type or schema is checked by the API suite, so the
system tests assert only what the schema cannot express — which values are rejected, what state changed, which field is
absent.

**Alternative considered:** keep the E2E cases and add system cases only for the unspecced read/update surface.
Rejected: the same account outcome would be asserted twice, and one copy would rot.

### 7. Identifier scope follows the capability, not the area

**Decision:** use `REQ-USER-001`…`REQ-USER-019` and `SCN-USER-001-A`…, per the repository rule that an identifier is
scoped to the capability the spec defines. The moved requirements gain IDs for the first time; the retired specs never
had any.

The spec refers to "the account with identifier `0`" for the media-transfer rules rather than redefining the Root Admin.
Its bootstrap — username `root`, Admin role, no email, created at first start — stays with `REQ-AUTHENTICATION-001`, and
the password policy stays with `REQ-AUTHENTICATION-018`.

**Alternative considered:** give the new capability `REQ-ACCOUNT-*` to match the retired names. Rejected: the capability
id is `user`, and the rule scopes the identifier to it.

### 8. Email uniqueness is case-insensitive, with the submitted case preserved

**Decision:** compare an email case-insensitively for uniqueness, at creation, on update and in the `email` filter,
while storing and returning the submitted value unchanged. Give the `email` column a `NOCASE` collation in
`migrations/20260904012829_create_user.up.sql`; the `uq_user_email` constraint and the repository's `email = ?` filter
inherit it, so one lookup feeds both the `409` pre-check and the race-closing index.

The pre-check in `RegisterUser` and `UpdateUser` searches the same filter, so a case variant is reported as a taken
email with the spec's message; the unique constraint closes the window when two writes race, and maps to `Conflict`.

**Alternative considered:** store the address lowercased. Rejected: the spec stores and returns the submitted value, and
a client that entered `Alice@example.com` would see its own value changed. **Alternative considered:** a unique index on
`lower(email)`. Rejected: it has the same ASCII-only folding as `NOCASE` and forces the index and the lookup to be kept
in step separately.

**Limitation:** SQLite's `NOCASE` folds ASCII letters only. Internationalized addresses that differ by non-ASCII case
would still be distinct; a normalized-key column would be needed, and that is out of scope.

**Proof:** the HTTP outcomes — a case variant answering `409`, the filter matching it and the stored case surviving —
are scenarios of the `user` capability, so `test/system/user.py` proves them black box. No application test asserts
them. The datastore's own contract — that `uq_user_email` and the `email` lookup compare case-insensitively while the
stored value keeps its case — is proved by a SQLx adapter integration test, the lowest layer that can observe the
collation.

### 9. The datastore also enforces the email shape

**Decision:** reconcile `chk_user_email` with the parser. The current `LIKE '%_@_%._%'` is both too weak — it accepts a
leading or trailing space and a display name — and too strong — it rejects `admin@mailserver1`, which the parser
accepts. The check becomes: `NULL`, or a trimmed, non-empty value with at least one character before and after an `@`.
The domain-dot requirement goes.

The parser remains authoritative for full syntax. A SQLite `CHECK` cannot express RFC 5322 without false rejections,
because a quoted local part may contain spaces, `@`, `<` and `>`; enforcing those would turn an address the API accepts
into a `500` on insert.

**Alternative considered:** register a `REGEXP` function and reproduce the parser. Rejected: the function must exist on
every connection before the migration and before each write, and it would duplicate the one parser the domain already
uses.

### 10. HAL stays a separate functional area

**Decision:** the `user` capability states the profile detail and nothing about media types, `_links`, self links or URI
templates. Hypermedia is a cross-cutting API concern, specified by the API functional area and implemented in
`src/lib/infrastructure/inbound/rest/hal.rs`.

The user handlers currently build their own `SelfLinks` and `_links` fields. If that construction is still handler-local
when the implementation starts, extract it into the HAL module (or a HAL-owned helper) so the handler returns the
profile and its URI, not a hypermedia envelope. No requirement is added for this; the task is conditional and closes
with evidence when the construction is already centralized.

**Alternative considered:** keep the HAL detail in the user spec. Rejected: it belongs to the API functional area, and
stating it per capability duplicates it in every spec.

## Risks / Trade-offs

- **Authorization coverage gap until the other workstream lands.** → The handoff table in Decision 3 names every rule,
  and the tasks record it as an external dependency rather than an omission.
- **The post-deletion `401` is owned by no capability after both changes.** → The UC-005 journey keeps asserting it and
  the `specify-authorization` proposal records the decision; a later authentication change is the natural owner.
- **A large new system module.** → It reuses the existing harness and container lifecycle; the scenarios are grouped by
  operation so a failure names the operation.
- **Moving E2E cases can leave a journey unproven.** → Only account-outcome cases move; the authorization cases stay in
  place until their new owner exists.
- **The spec blesses current gaps.** → Decision 5 records them explicitly as follow-ups, so a reader cannot mistake them
  for intended permanence.
- **The retired specs' scenarios carried no IDs, so their proofs carry no `story` labels today.** → The system module
  labels the scenarios it proves; no test is relabelled retroactively.
- **Email uniqueness becomes breaking.** → **BREAKING** is marked in the proposal; a client relying on two case-variant
  addresses now gets `409`, and the change has no compatibility shim by design.
- **`NOCASE` folds ASCII only.** → The limitation is recorded in Decision 8; the parser's non-ASCII case handling is
  unchanged, and a normalized-key column is the follow-up if internationalized addresses need it.
- **The datastore check cannot equal the parser.** → Decision 9 keeps the check to invariants no valid address can
  violate, with adapter cases for the parser-accepted edge cases; the parser remains authoritative.
- **The HAL extraction could ripple across user handlers.** → The task is conditional and scoped to the user handlers;
  the JSON and `application/hal+json` stay identical, so no endpoint behavior changes.

## Migration Plan

1. Land the new `user` capability and the two removal deltas; archive applies them and deletes the emptied child specs.
2. Edit `migrations/20260904012829_create_user.up.sql` in place (`PERS-014`): the `email` collation and the reconciled
   `chk_user_email`. Recreate the local database (`mnemorium.db` and its `-wal`/`-shm` sidecars) before starting the
   server, because the migration checksum changes.
3. Write the failing system and SQLx cases for case-insensitive uniqueness, apply the collation, then add the adapter
   cases for the email check and the rest of `test/system/user.py`, and move the account-outcome E2E cases into it.
4. No data migration beyond the in-place schema edit; the schema is unreleased and no shipped database holds
   case-variant emails.
5. Rollback is a code revert: the child specs return with the archive history, the E2E cases return with the code, and a
   reverted migration requires a database reset.
