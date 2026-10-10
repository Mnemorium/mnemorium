# Add check — STY-RUST-052

## Rule guard — STY-RUST-052

- **Rule:** "Declare the repository trait second in a repository port file." (`docs/development/TechnicalDesign.md`, Repository, § 8.3)
- **Check state:** missing — `script/rule_enforcer.sh uncovered` lists `STY-RUST-052 (Repository) — Declare the repository trait second in a repository port file.`; `list` does not register it, and `rule/sty-rust-052.yml` does not exist.
- **Mechanism:** ast-grep — an ordering constraint over top-level declarations that a relational `precedes` pattern expresses directly; a single rule file plus the harness's standard thin wrapper. No config block.

**Clarification.** This check is self-contained inside `script/rule_enforcer.sh`. It deliberately does **not** wrap or invoke `script/check_repository_declaration_order.sh`; that standalone script is slated for removal. The order logic is re-expressed as an ast-grep rule (`rule/sty-rust-052.yml`), so removing the standalone script leaves the check intact.

## Files

### `rule/sty-rust-052.yml` (new)

Top-level `struct`/`trait` declarations of a repository port file are siblings under `source_file`. A violation is exactly a `pub struct` that is not a `*Filter` and that precedes the `pub trait …: Send + Sync`. A read model after the trait (`GalleryItemDetail` in `gallery_repository.rs`) is not matched; filter structs before the trait are not matched; `impl` blocks are not `struct`/`trait` items, so they are out of scope as required.

```yaml
---
id: sty-rust-052
language: Rust
severity: error
message: >-
  Declare the repository trait second in a repository port file: a
  non-`*Filter` struct must not precede the `<Aggregate>Repository` trait.
files:
  - src/lib/domain/port/*_repository.rs
rule:
  all:
    - kind: struct_item
    - pattern: pub struct $NAME
    - inside:
        kind: source_file
    - not:
        regex: 'pub struct \w*Filter\b'
    - precedes:
        kind: trait_item
        pattern: 'pub trait $TRAIT: Send + Sync'
        stopBy: end
```

`severity: error` is required so `ast-grep scan` exits non-zero on a finding (a `warning`/`hint` severity exits 0 — the extractor rules that use `hint` read `--json=stream` instead). This mirrors the negative rules `sty-rust-003`…`sty-rust-024`.

### `script/rule_enforcer.sh`

Registry line — append after `"STY-RUST-024:check_sty_rust_024",`:

```bash
	"STY-RUST-052:check_sty_rust_052"
```

Function — add beside `check_sty_rust_024` (the standard exit-code wrapper used by `check_sty_rust_003`, `check_sty_rust_004`, and the other `severity: error` rules):

```bash
check_sty_rust_052() {
	local id="STY-RUST-052"
	local rule_id="sty-rust-052"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

No `rule_enforcer.yaml` block is added; the check has no per-check data.

## Verification

- **Red:** `/tmp/opencode/sty052/red.rs` — a `GalleryItemDetail` struct declared before `pub trait GalleryRepository: Send + Sync` — `ast-grep scan --rule sty-rust-052.yml --report-style=short --color=never red.rs` — observed: `red.rs:5:1: error[sty-rust-052]: Declare the repository trait second …`, exit 1.
- **Green:** `/tmp/opencode/sty052/green.rs` (`GalleryFilter`, `GalleryItemFilter`, trait, then `GalleryItemDetail`) and `/tmp/opencode/sty052/green_trait_first.rs` (trait first, no filter) — same command — observed: no finding, exit 0.
- **Repository:** `ast-grep scan --config sgconfig.yaml --filter sty-rust-052 --report-style=short --color=never src` over a `/tmp/opencode/sty052/mirror/` copy of the real `src/lib/domain/port/*.rs` with the candidate rule registered in its `rule/`: observed exit 0 on the real files; injecting `red.rs` as `gallery_repository.rs` made the same command flag `src/lib/domain/port/gallery_repository.rs:5:1 … error[sty-rust-052]` and exit 1; `ast-grep scan --rule /tmp/opencode/sty052/sty-rust-052.yml src` against the real repository also exited 0.

## Next

- Apply: `rule/sty-rust-052.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
