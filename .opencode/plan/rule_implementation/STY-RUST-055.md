# Add check — STY-RUST-055

## Rule guard — STY-RUST-055

- **Rule:** `Implementation named just \`<UseCaseName>\`, e.g. \`CreateNote\`.`
  Section: **Use cases** (`docs/development/TechnicalDesign.md`, line 165).
- **Check state:** missing — `script/rule_enforcer.sh list` has no `STY-RUST-055`; `uncovered` lists it.
- **Mechanism:** ast-grep extraction + bash pairing — the rule needs to strip the `UseCase` suffix from the trait identifier and compare it to the impl type, and ast-grep cannot relate two metavariables; a `sty-rust-055` extraction rule captures `TRAIT`/`TYPE` and a Bash check performs the suffix comparison (the idiom of `check_sty_rust_006` / `check_sty_rust_023`).

## Files

### `rule/sty-rust-055.yml` (new)

Extraction rule scoped to the use-case implementation directory, as an extraction (`severity: hint`) like `rule/sty-rust-015.yml`. `has field: trait` matches every trait impl regardless of a `where` clause (the pattern form `impl<...> $TRAIT for $TYPE { ... }` fails on the `where`-clause impls that dominate this directory); the nested `regex: 'UseCase$'` drops the test-local `std::io::Write` / `tracing_subscriber` helper impls.

```yaml
---
id: sty-rust-055
language: Rust
severity: hint
message: extract use-case trait implementations for STY-RUST-055
files:
  - src/lib/application/use_case/*.rs
rule:
  kind: impl_item
  all:
    - has:
        field: trait
        all:
          - pattern: $TRAIT
          - regex: 'UseCase$'
    - has:
        field: type
        pattern: $TYPE
```

### `script/rule_enforcer.sh`

Add one registry line after `"STY-RUST-024:check_sty_rust_024"` in `RULES`:

```diff
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-055:check_sty_rust_055"
 )
```

Add the check function between `check_sty_rust_024()` and the `# --- Runner ---` banner:

```bash
check_sty_rust_055() {
	local id="STY-RUST-055"
	local rule_id="sty-rust-055"
	local use_case_dir="src/lib/application/use_case"
	local matches file trait impl_type base expected result=0 count=0

	if [ ! -d "$use_case_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	# sty-rust-055 extracts each use-case trait implementation; the
	# trait-to-struct comparison happens here because an ast-grep rule cannot
	# strip the `UseCase` suffix from one identifier and compare it to another.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream "$use_case_dir" 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no use-case implementation from %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	while IFS=$'\t' read -r file trait impl_type; do
		[ -n "$trait" ] || continue
		count=$((count + 1))
		expected="${trait%UseCase}"
		base="${impl_type%%<*}"
		if [ "$base" != "$expected" ]; then
			printf 'error: %s: %s: %s implements %s; expected the implementation to be named %s\n' \
				"$id" "$file" "$base" "$trait" "$expected" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, .metaVariables.single.TRAIT.text, .metaVariables.single.TYPE.text] | @tsv')

	if [ "$count" -eq 0 ]; then
		printf 'error: %s: extracted no use-case implementation from %s\n' "$id" "$use_case_dir" >&2
		return 1
	fi

	return "$result"
}
```

No `rule_enforcer.yaml` block is needed: the check has no per-rule data to externalize.

## Verification

The red/green fixtures mirror the repository layout (`sgconfig.yaml` with `ruleDirs: [rule]`, `rule/sty-rust-055.yml` byte-identical to the shipped rule, and the target under `src/lib/application/use_case/`), so the proof runs the same invocation shape as the check (`ast-grep scan --config sgconfig.yaml --filter sty-rust-055` from the fixture root). The scratch check `/tmp/opencode/sty-rust-055/check.sh` is the literal function above with `return`/`exit` adapted.

- Red: `/tmp/opencode/sty-rust-055/red/src/lib/application/use_case/create_gallery.rs` (defines `GalleryCreator<F>` with `impl<F> CreateGalleryUseCase for GalleryCreator<F> where F: UnitOfWorkFactory`). Command `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-055/red && bash /tmp/opencode/sty-rust-055/check.sh'` — observed:
  `error: STY-RUST-055: src/lib/application/use_case/create_gallery.rs: GalleryCreator implements CreateGalleryUseCase; expected the implementation to be named CreateGallery` (exit 1).
- Green: `/tmp/opencode/sty-rust-055/green/src/lib/application/use_case/create_gallery.rs` (defines `CreateGallery<F>` with `impl<F> CreateGalleryUseCase for CreateGallery<F> where F: UnitOfWorkFactory`). Same command against `.../green` — observed: no output, exit 0.
- Repository: `devenv shell -- ast-grep scan --rule /tmp/opencode/sty-rust-055/sty-rust-055.yml --json=stream src/lib/application/use_case` from the repo root — observed: 21 impl pairs, one per use case (`CreateGalleryUseCase`→`CreateGallery`, `BeginUploadUseCase`→`BeginUpload`, …, `WriteUploadChunkUseCase`→`WriteUploadChunk`), every type equal to its trait minus `UseCase`; the same Bash pairing run against the repo exits 0. `devenv shell -- script/rule_enforcer.sh list` currently omits `STY-RUST-055`; after applying the two edits it will be registered and pass.

## Next

- Apply: `rule/sty-rust-055.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
