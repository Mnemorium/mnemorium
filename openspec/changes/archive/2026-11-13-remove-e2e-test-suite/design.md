# Design

## Context

See [`proposal.md`](proposal.md) for motivation.

- `test/e2e/` holds 27 modules and 46 tests. `script/e2e.sh` starts one container and exports its base URL, so the whole
  suite shares one process; session fixtures provision three accounts once and tolerate `409` from an earlier run.
- `test/system/` holds one module per feature area, collected by `test/system/conftest.py`, and starts a fresh container
  per case (`TEST-058`). Every case carries `epic`/`feature`/`story` labels (`TEST-054`), and every route comes from
  `Sut.href`, a named link, or the response's own `_links.self`.
- `docs/development/TechnicalDesign.md` § 5 already states the rule this change completes: a use case whose whole
  behavior is a spec scenario is proven by the system suite, and the E2E case is deleted in the same change.
- The resource-and-grant authorization model replaced the scope model. `tests/e2e/identity/uc024…` still drives the
  removed `/api/v1/user/{id}/permission` route, and `uc024`'s expectations (scopes `OWNED`, `PUBLIC_OR_OWNED`, `LISTED`)
  match only the retired `UseCases.md` UC-024 entry. `uc009`'s "an Admin can add another caller's media" encodes the
  retired rule that an Admin may use every file.

## Goals / Non-Goals

**Goals:**

- Remove the E2E suite, its harness and its CI step, leaving one black-box suite and one harness.
- Keep every requirement the suite alone proved, as a system case with its existing requirement and scenario labels.
- Leave no reference to `test/e2e`, `script/e2e.sh` or the retired scope model in code, CI, tooling or documentation.

**Non-Goals:**

- No requirement, API, schema or runtime change; `.openspec.yaml` sets `skip_specs: true`.
- No shared-instance soak coverage; the property is recorded as a residual, not reconstructed.
- No rename of the `test-e2e` commit scope (`GOV-001`); parked.
- No migration of the duplicate journeys, and no change to `test/configuration` or the Schemathesis harness.

## Decisions

### D1 — Retire the suite rather than keep it minimal

Every remaining assertion is either proven elsewhere or stale. Keeping a third black-box suite that shares a container
buys only a second image lifecycle and a second harness to maintain. **Alternative considered:** keep the directory with
the 13 unique cases — rejected, because they belong next to the feature they prove, in the suite that already owns the
labels and the per-case isolation.

### D2 — Migrate only the cases with no other proof

A case was migrated when no test in `test/system`, `test/configuration` or `src` carries its scenario label and no
lower-layer test asserts the same outcome. The mapping:

| E2E case                                                                   | Destination                    | Label(s)                                            |
| -------------------------------------------------------------------------- | ------------------------------ | --------------------------------------------------- |
| `uc001` unauthenticated registration, `uc016`/`uc017`/`uc018` token guards | `test/system/api.py`           | `REQ-API-015` / `SCN-API-015-C`                     |
| `uc001` Admin grants the Admin role                                        | `test/system/authorization.py` | `REQ-AUTHORIZATION-019` / `SCN-AUTHORIZATION-019-B` |
| `uc005` root, Admin-peer and self deletion refusals                        | `test/system/authorization.py` | `REQ-AUTHORIZATION-019` / `-C`, `-E`                |
| Adjacent gap: Admin creates a Standard account                             | `test/system/authorization.py` | `REQ-AUTHORIZATION-019` / `-A`                      |
| Adjacent gap: Admin replaces a Standard user's grants                      | `test/system/authorization.py` | `REQ-AUTHORIZATION-020` / `-A`                      |
| `uc008`, `uc009`, `uc010`, `uc011` root cases                              | `test/system/authorization.py` | `REQ-AUTHORIZATION-017` / `SCN-AUTHORIZATION-017-A` |
| `uc011` Admin deletes a readable public gallery                            | `test/system/authorization.py` | `REQ-GALLERY-006` / `SCN-GALLERY-006-C`             |
| `uc009` add to a hidden gallery, `uc010` remove from one                   | `test/system/gallery.py`       | `REQ-AUTHORIZATION-038` / `SCN-AUTHORIZATION-038-A` |

The root cases collapse into one case that reads, contributes to and deletes another caller's private **Gallery**; the
four 401 guards collapse into one case that walks the **API index** links (public: `self`, `login`, `register`, `token`,
`health`) and asserts the rest answer `401`.

**Alternative considered:** move every retained case one-for-one — rejected, it would import duplicate assertions and
their shared-instance coupling.

### D3 — Denial outcomes live in `authorization.py`

`authorization.py` already groups denial outcomes behind one fixture and is the primary proof of the authorization
capability, so the account-administration and root cases live there. The `SCN-GALLERY-006-C` case stays there too,
because it is the same shape of proof (visible but not permitted → `403`); its labels remain the gallery capability's,
since the scenario is owned by `openspec/specs/gallery/spec.md`. The module docstring records that split so the labels
are not mistaken for drift.

### D4 — Do not port `uc024`

The route it drives no longer exists (`user_router` registers only `me`, `user/{id}` and the collection), and the scope
model is documented as gone. The resource-and-grant equivalents are already proven by `REQ-AUTHORIZATION-031`…`035` in
`authorization.py`. Migrating the file would either fail or re-assert the same rules through a surface that does not
exist. Its conftest fixtures `get_permission` and `put_permission` are deleted with it.

### D5 — Drop the "Admin adds another caller's media" case

A `FILE` follows its typed asset's owner, and adding an item requires `View` on the file, so a non-root **Admin** cannot
append a **Standard User**'s private image; the E2E expectation (`201`) describes the retired rule. The root half of the
same test is kept through `SCN-AUTHORIZATION-017-A`. `UseCases.md` UC-009 and UC-020 alternative flow 4a are corrected
to say only the **Root Admin** may use every file.

### D6 — No spec delta

No requirement or scenario text changes: every migrated case labels an existing scenario. The add/remove-on-an-
unreadable-gallery behavior has no scenario of its own; it is the concealment rule of `REQ-AUTHORIZATION-038` applied to
a write, so the case stacks `SCN-AUTHORIZATION-038-A` rather than inventing a requirement. A new scenario would claim a
behavior change that did not happen.

### D7 — Prove first, delete second

The system cases are written and run against the image before `test/e2e` is deleted. A migrated case that fails
immediately is a defect the E2E suite was masking (or a stale expectation like D5), and it must be understood before the
file that carried it disappears. A test-only change has no red phase of its own; the assertion passes on first run when
the behavior exists.

### D8 — Delete the harness last

`script/e2e.sh` is deleted only after `script/system.sh` and `script/configuration.sh` stop pointing at it, and after
the CI job no longer calls it, so no gate references a missing file in an intermediate commit.

## Risks / Trade-offs

- **A deleted case secretly covered an unlisted behavior** → the mapping table is exhaustive over the 46 tests; the full
  system suite and the container run happen before deletion, and git history keeps the files recoverable.
- **Documentation keeps describing the retired model** → the UC-024 rewrite and the UC-009/UC-020 correction are tasks
  in this change, not follow-ups; the scope-model grep is part of the verification.
- **CI keeps a dead step or cache name** → the CI task removes the E2E step, the `e2e-report` artifact, the
  stop-container step and the buildx cache keys; the grep gate catches `e2e` strings under `.github/`.
- **The system job grows by one container per new case** → seven cases add roughly seven short container lifetimes to a
  30-minute job; accepted against the removed E2E job.
- **Shared-instance coverage is lost** → recorded as a residual in `proposal.md`; a soak case is a later change.
- **`pytest.ini` still lists the directory** → `testpaths` drops `test/e2e` in the same task as the deletion, or a bare
  `pytest` fails on a missing path.

## Migration Plan

1. Add and run the system cases.
2. Delete `test/e2e/`, `script/e2e.sh`, then the references in tooling, CI and documentation.
3. Run `cargo fmt`, `cargo clippy`, `script/rule_enforcer.sh check`, the Markdown gates and `script/system.sh`.

Rollback is `git revert` of the change commit; the deleted tests return with it. No data or runtime state is involved.
