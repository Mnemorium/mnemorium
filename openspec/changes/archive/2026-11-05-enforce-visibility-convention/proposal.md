# Proposal

## Why

Nothing in the rulebook governs Rust visibility except `STY-RUST-085`, and only for `const`/`static`. The result is two
philosophies wearing one ID: `domain/` and `application/port/` are `pub` by decree, everything else is narrowest-that-
compiles. The crate therefore advertises far more than it uses — 2,354 `pub` declarations for 435 that the only external
consumer (`src/bin/server.rs`) ever names — and the same kind of item is spelled differently in the same directory. A
`pub` on an unreferenced item also silences `dead_code`, so the over-exposure hides itself.

## What Changes

- **BREAKING** — `src/bin/server.rs` stops being the composition root. The body of `main` moves to `src/lib/app.rs` as
  `pub async fn run() -> anyhow::Result<()>`, and the binary becomes a launcher. The crate's advertised surface becomes
  exactly `pub mod app;` in `src/lib/lib.rs` and `run` in `src/lib/app.rs`.
- Add `STY-RUST-087` (Visibility) as the single rule owning visibility, with a new `§ 11. Visibility` detail section
  holding the explanation.
- Delete the visibility clause from `STY-RUST-085`; it keeps site, naming and units and cites `STY-RUST-087`.
- Enable `unreachable_pub = "deny"` under a new `[lints.rust]` table, ramped through `warn`.
- Add a `script/rule_enforcer.sh` check for the half rustc cannot see: `pub(crate)` that could be private, and the
  banned `pub(super)` / `pub(in …)` forms.
- Reword `STY-RUST-067` so the composition root is `src/lib/app.rs` and the binary only launches it.
- Retarget the four enforcer checks that name `src/bin/server.rs` by path: `STY-RUST-067`, `STY-RUST-038`,
  `STY-RUST-064`, `STY-RUST-074`.
- Renarrow the tree: 152 declarations to private, 1,767 to `pub(crate)`, and 293 `pub mod` to `pub(crate) mod`.
- Out of scope: field visibility (clippy already pins its form), the `GOV-001` scope row for `src/lib/app.rs`, and any
  change to `pub use` (the project keeps none).

## Capabilities

### New Capabilities

None. This change alters a code-style rule and the crate's internal surface; it changes no product behavior, so no
capability spec applies (`skip_specs: true`).

### Modified Capabilities

None.

## Impact

- `src/lib/app.rs` (new), `src/lib/lib.rs`, `src/bin/server.rs`.
- `Cargo.toml` (`[lints.rust]`).
- `script/rule_enforcer.sh` and a new `rule/sty-rust-087.yml`.
- `docs/development/TechnicalDesign.md` § 1 (rule row, § 11, `STY-RUST-067`) and the path at § 10.1.
- Every `src/lib/**` module that declares a modifier; no behavior or API changes.
