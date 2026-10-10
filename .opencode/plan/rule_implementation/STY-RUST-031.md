# Add check — STY-RUST-031

## Rule guard — STY-RUST-031

- **Rule:** "Use `#[fixture]` functions when a test case input is a struct." (`docs/development/TechnicalDesign.md`, Code Style Guidelines → Rust, Testing section; § 1 row links to `#5-testing`)
- **Check state:** missing — `STY-RUST-031` is in no registry entry in `script/rule_enforcer.sh`, has no `rule/sty-rust-031.yml`, and no `rule_enforcer.yaml` block. The whole `STY-RUST-026`–`031` testing-style family is unregistered.
- **Mechanism:** none — the rule is **not soundly mechanically checkable** with the available mechanisms. This is a verdict, not a plan.

## Why it is not mechanically checkable

The predicate "a test case input **is a struct**" needs two things the harness cannot supply: a policy for what counts as a struct, and type resolution.

**1. The predicate is under-specified.** § 5 (Testing, `docs/development/TechnicalDesign.md:2123`) contains no `#[fixture]` example and no further text for this rule — the only occurrences of `#[fixture]`/`rstest` in the docs are the row at line 141 and the § 1 `#[case]` example at line 606. Nothing narrows "struct". Taken literally it includes framework value types: the repository already passes `#[case] status: StatusCode` and `#[case] expected: StatusCode` (`http::StatusCode`, a struct) in `.../handler/library/post_gallery.rs`, `get_gallery_list.rs`, `delete_gallery.rs`, `post_gallery_item.rs`, `.../middleware/auth.rs`, `hal_errors.rs`, `api_error.rs`. A literal check makes the repository violate immediately; a narrowed check ("a struct declared under `src`") is a policy choice the rule text does not authorise — and on that narrow reading the repo currently has zero `#[case]` inputs of a local struct (verified: all 146 `#[case]` params are `&str`, `&'static str`, `NumericID = i64`, an enum, or `StatusCode`). Either reading is invented.

**2. Attribute arguments are opaque token trees, so ast-grep cannot even see a struct literal in a `#[case(...)]`.** A rule of `kind: struct_expression` matches the same text outside an attribute but finds nothing inside `#[case(...)]`:

- Probe A — `ast-grep scan --rule <kind: struct_expression>` over a fixture containing `#[case(Input { value: 1 })]`: **no matches**.
- Probe B — `ast-grep run -l rust -p '#[case($$$A)]' --debug-query=ast`: parses as `attribute_item > attribute > token_tree`, not as expressions.

So the one candidate syntactic signal ("struct literal inline in a `#[case]`") is out of reach, and a raw-text/regex proxy over the token tree would be the only route — fragile, and still only catches brace-literals, missing struct inputs passed as variables or constructor calls (`#[case(make_input())]`), while false-positiving on enum struct-variants (`#[case(Rotation::Custom { .. })]`).

**3. No mechanism in the ordered list applies.** No command performs this check (no `clippy`/`rustc` lint for `rstest` fixtures; `clippy.toml` sets only `too-many-lines-threshold`). A Bash cross-file check could pair `#[case]` parameter type names against `struct` declarations, but that is exactly the under-specified, type-resolution-dependent heuristic above. `rule_enforcer.yaml` config would only encode the same undecidable policy as data.

## What would unblock it

A type-aware enforcement tool, or a rulebook amendment that fixes the predicate — e.g. "a `#[case]` input whose type names a struct declared in the workspace must instead be a `#[fixture]` parameter" with the fixture-name↔parameter-name convention spelled out. With that narrower, grounded predicate, a Bash check could extract `#[case]` parameters via ast-grep `--json=stream` and pair them with `struct_item` names, in the idiom of `check_sty_rust_013`/`019`/`023`. Until the text is amended, any shipped check would enforce invented behaviour, so I am returning no files and no check.

## Verification (probes run, scratch only)

- `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-031/probe-rule.yml --report-style=short --color=never /tmp/opencode/sty-rust-031/probe.rs` — observed: no match for the struct literal inside `#[case(Input { value: 1 })]`.
- `devenv shell -- ast-grep run -l rust -p '#[case($$$A)]' --debug-query=ast /tmp/opencode/sty-rust-031/probe.rs` — observed: `arguments: token_tree`, i.e. not parsed expressions.
- `devenv shell -- script/rule_enforcer.sh list` — observed: no `STY-RUST-031` (and no `STY-RUST-026`–`030`) registered.

No tracked file was read-modified or written; all scratch lives under `/tmp/opencode/sty-rust-031/`.

## Next

- Apply: none — `rule-guard` returned a verdict that `STY-RUST-031` is not soundly mechanically checkable, not a plan to apply.
- Verify: `devenv shell -- script/rule_enforcer.sh list` (confirm no `STY-RUST-031` registration; the probes above are the evidence).

To unblock, amend the rule text in `docs/development/TechnicalDesign.md` § 5 to fix the predicate, then re-run this command.
