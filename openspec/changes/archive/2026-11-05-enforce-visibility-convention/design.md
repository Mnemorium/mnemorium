# Design

## Context

See `proposal.md` — Why. Three facts shape the approach:

- The package is one library plus one binary, with no `tests/`, no workspaces and no `pub use`. The only consumer
  outside the library crate is `src/bin/server.rs`, 284 lines whose body is the composition root required by
  `STY-RUST-067`.
- rustc already computes the crate boundary: `unreachable_pub` flags a `pub` item no outside crate can reach. A probe
  crate confirms it fires in both the library build and the `--test` build, and does not fire on a reachable `pub fn` at
  the crate root.
- rustc does not compute the step below it: a `pub(crate)` item used by one file is still "used", so neither a lint nor
  `dead_code` reports its width.

## Goals / Non-Goals

**Goals:**

- One rule owns visibility, with no layer carve-outs, and a checker that measures use rather than enumerating who is
  allowed to be `pub`.
- The advertised surface is a structural fact (two declarations), not a reviewed approximation.
- One test per step of the ladder, each enforced by the cheapest mechanism that can express it.

**Non-Goals:**

- Field visibility. `clippy::field_scoped_visibility_modifiers` and `partial_pub_fields` already pin the form, and
  choosing `pub` fields versus private fields plus accessors is an encapsulation decision per struct kind.
- The `GOV-001` scope row for `src/lib/app.rs` (deferred by decision; the closed-set table is stale until it lands).
- `pub use` re-exports, which the project does not use and `clippy::pub_use = "warn"` discourages.

## Decisions

### 1. Width derives from use, not from layer

A declaration is private, `pub(crate)` when another module names it, and `pub` only at the crate entry point. The
alternative — `domain/` and `application/port/` always `pub`, as `STY-RUST-085` states for constants — is rejected: it
is the carve-out that produced 28 file-local `pub` constants in `domain/` next to private ones in `infrastructure/`, and
any such table needs an exception list (DTOs, traits, filters) that the compiler would otherwise answer.

### 2. Shrink the boundary instead of policing it

Move the body of `main` into `src/lib/app.rs` as `pub async fn run() -> anyhow::Result<()>`; `src/bin/server.rs` keeps
`#[tokio::main]` and calls it. This makes `pub` legal exactly twice: `pub mod app;` and `run`. Alternatives:

- **Compiler-only, no shim** — leave 435 `pub` declarations and let `unreachable_pub` keep them honest. Rejected: the
  surface stays 435 declarations wide, and every new bin use reopens it.
- **Facade with re-exports** — one `pub` module re-exporting what the bin needs. Rejected: the project has zero
  `pub use`, and `clippy::pub_use` is already `warn`.

The composition root relocates with the code, so `STY-RUST-067` is reworded to name `src/lib/app.rs` and the binary as a
launcher. Its meaning is unchanged: one place constructs the process-lifetime adapters.

### 3. Split enforcement at the step each mechanism can see

- `[lints.rust] unreachable_pub = "deny"` covers `pub` → `pub(crate)`, including in `--all-targets` test builds.
- A new `script/rule_enforcer.sh` check covers `pub(crate)` → private and rejects `pub(super)` / `pub(in …)`.

`unreachable_pub` alone cannot see the second step; an enforcer-only approach discards a one-line compiler lint that
already answers the boundary question. The four checks that hardcode `src/bin/server.rs` (`STY-RUST-067`,
`STY-RUST-038`, `STY-RUST-064`, `STY-RUST-074`) retarget to `src/lib/app.rs` in the same change; without that, `067`
sees `AppState::new` outside the composition root and `064`/`074` find zero `LoadConfiguration` bindings where they
require one.

### 4. "Names it" is an identifier occurrence in another file, comments excluded

The same approximation `STY-RUST-085` already ships. A compiler-driven alternative (try the item private, keep
`pub(crate)` only if the build fails) is exact but means a compile per candidate, which no lint can do. The
approximation is safe in both directions: `unreachable_pub` catches exposure the check misses, `dead_code` catches
narrowing the check gets wrong.

### 5. One rule, one owner

`STY-RUST-087` owns visibility end to end; `STY-RUST-085` keeps site, naming and units and cites it. Two rules owning
one fact is what `STY-RUST-086` forbids. The detail lives in a new `§ 11. Visibility`, appended because detail numbers
are append-only. Removing 085's visibility clause also removes its visibility assertions from `check_sty_rust_085`
(`script/rule_enforcer.sh:2243-2279`), which today demands `pub` in `domain/` and would otherwise reject the sweep it is
meant to allow.

### 6. Test code is not exempt

`unreachable_pub` flags `pub` inside a private test module in the `--test` build (verified), so an exemption would be
prose the build contradicts. Tests follow the ladder, which converts `test_helpers`' 62 `pub` items to `pub(crate)` and
ends the `test_helpers` / `test_fixtures` split-spelling.

### 7. Ramp the gate, land the check last

`unreachable_pub` enters as `warn`, the tree is swept layer by layer, then it flips to `deny`. The enforcer check is
pass/fail with no `warn` setting, so it lands with the flip; a baseline file would add a second migration mechanism that
outlives its purpose. The sweep order is inward-out (`domain`, `application`, `infrastructure`, then the
`api`/`persistence` sub-trees), each layer including its `pub mod` conversions, so a reviewer sees one convention at a
time. `GOV-001`'s one-scope-per-title constraint is not applied to the sweep, by decision.

## Risks / Trade-offs

- **`mockall` mocks** — `#[cfg_attr(test, mockall::automock)]` generates `pub` `Mock*` types that may be unreachable;
  the `warn` pass will say so. Mitigation: one `#[allow(unreachable_pub)]` beside the attribute, visible and reviewed,
  rather than a rule clause.
- **~1,900-site sweep** — a mechanical edit can hide a semantic change. Mitigation: the edit only removes or narrows a
  keyword; the per-layer gates and the black-box E2E suite stay green at every step.
- **Grep approximation** — a name mentioned in a doc comment of another module reads as a use. Mitigation: the result is
  over-exposure, never under-exposure; a reviewer sees the narrowed candidate.
- **Relocated composition root** — four enforcer checks and one rule text follow the path. Mitigation: same commit, and
  each check keeps its assertion by pointing at the new file.
- **Truthful surface at the cost of a crate-shaped seam** — `run` is the only thing the library exports, so a future
  second binary must go through it. Accepted: that is the contract the change chooses.

## Migration Plan

1. Shim, rule, docs, check retargets, `unreachable_pub = "warn"`.
2. Sweep inward-out: `domain`, `application`, `infrastructure`, `api`/`persistence`.
3. Flip `unreachable_pub` to `deny` and land the `STY-RUST-087` enforcer check.

Rollback is keyword-only: reverting a commit widens declarations back without touching control flow, and the shim is a
move, so reverting it restores the binary's composition root.

Verification is `script/e2e.sh` and `script/configuration.sh`; `devenv test` is deliberately not run for this change.
