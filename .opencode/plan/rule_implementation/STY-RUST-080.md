# Add check — STY-RUST-080

## Rule guard — STY-RUST-080

- **Rule:** "A filter field is named after the aggregate field it constrains, e.g. `UserFilter.user_id` for `User::user_id()`." (`docs/development/TechnicalDesign.md` line 190, row `| STY-RUST-080 | Repository | … |` with an empty More info cell; section **Repository**).
- **Check state:** missing — `script/rule_enforcer.sh list` has no `STY-RUST-080` entry, `uncovered` prints `STY-RUST-080 (Repository) — …`, no `check_sty_rust_080` is declared, and `rule/sty-rust-080.yml` does not exist.
- **Mechanism:** ast-grep extraction rule + Bash — the violation is a correlation between a `*Filter` field name and the accessor names of the aggregate named by the filter (`<Aggregate>Filter` → `<Aggregate>`), which ast-grep cannot express (it cannot relate two identifiers). `rule/sty-rust-080.yml` extracts both sides and the Bash check pairs them. No `rule_enforcer.yaml` block: the two paths and the `<Aggregate>Filter` → `<Aggregate>` derivation are fixed by the rule text.

### Interpretation (why the check is shaped this way)

The "aggregate field" is read as the aggregate's public accessor, because the rule's own example cites a method (`User::user_id()`). So a field of `<Aggregate>Filter` is compliant when the same-named `pub fn …(&self)` exists on `impl <Aggregate>`. This makes the rule mechanical and matches every non-identity filter field today (`FileFilter.user_id` ↔ `File::user_id()`, `ImageFilter.file_id` ↔ `Image::file_id()`, `VideoFilter.color_id` ↔ `Video::color_id()`, `UserFilter.id` ↔ `User::id()`). The identity filters of the aggregates whose accessor is `<aggregate>_id` (`Gallery::gallery_id()`, `GalleryItem::gallery_item_id()`, `Image::image_id()`, `Video::video_id()`, `Upload::upload_id()`) currently use `id`, so they are the deviations this rule names — see the repository result.

## Files

### `rule/sty-rust-080.yml` (new)

```yaml
---
id: sty-rust-080
language: Rust
severity: hint
message: >-
  extract filter struct fields and aggregate accessors for STY-RUST-080
files:
  - src/lib/domain/port/*.rs
  - src/lib/domain/model/*.rs
rule:
  any:
    # A public field of a filter struct: capture the declaring struct and the
    # field name. The Bash layer keeps only names ending in `Filter`.
    - all:
        - kind: field_declaration
        - pattern: 'pub $FIELD: $TYPE'
        - inside:
            kind: field_declaration_list
            inside:
              kind: struct_item
              pattern: 'pub struct $FILTER { $$$FIELDS }'
              stopBy: end
    # A public accessor of an aggregate: capture the impl target and the method
    # name. Every aggregate getter takes `&self`.
    - all:
        - pattern: 'pub fn $METHOD(&self) -> $RET { $$$BODY }'
        - inside:
            kind: impl_item
            has:
              field: type
              pattern: $IMPL
            stopBy: end
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no change there. The field arm also sees the public-fielded model structs (`ProbedImage`, `ProbedVideo`); the Bash layer keeps only names ending in `Filter`, so they are ignored.

### `script/rule_enforcer.sh`

Two edits; the runner is untouched.

**1. Registry** — append one line to `RULES=( … )` after the current last entry (`"STY-RUST-024:check_sty_rust_024"`, line 71), before the closing `)` on line 72:

```diff
 	"STY-RUST-067:check_sty_rust_067"
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-080:check_sty_rust_080"
 )
```

**2. Check function** — add in the `# --- Checks ---` section, immediately before the `# --- Runner ---` banner (line 754), after `check_sty_rust_024()` (which ends at line 752):

```bash
check_sty_rust_080() {
	local id="STY-RUST-080"
	local rule_id="sty-rust-080"
	local port_dir="src/lib/domain/port"
	local model_dir="src/lib/domain/model"
	local matches filter field aggregate accessor result=0
	local -A accessors=()

	if [ ! -d "$port_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$port_dir" >&2
		return 1
	fi
	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# sty-rust-080 extracts each public `*Filter` field and each aggregate
	# accessor; the field-to-aggregate pairing happens here because an ast-grep
	# rule cannot relate two identifiers.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no filter field or aggregate accessor\n' "$id" >&2
		return 1
	fi

	while IFS=$'\t' read -r aggregate accessor; do
		[ -n "$accessor" ] || continue
		accessors["${aggregate}:${accessor}"]=1
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.IMPL != null and .metaVariables.single.METHOD != null) | [.metaVariables.single.IMPL.text, .metaVariables.single.METHOD.text] | @tsv')

	while IFS=$'\t' read -r filter field; do
		[ -n "$field" ] || continue
		case "$filter" in
		*Filter) ;;
		*) continue ;;
		esac
		aggregate="${filter%Filter}"
		if [ -z "${accessors[${aggregate}:${field}]+set}" ]; then
			printf 'error: %s: %s: field %s is not named after a field of %s (no %s::%s())\n' \
				"$id" "$filter" "$field" "$aggregate" "$aggregate" "$field" >&2
			result=1
		fi
	done < <(printf '%s\n' "$matches" |
		jq -r 'select(.metaVariables.single.FILTER != null and .metaVariables.single.FIELD != null) | [.metaVariables.single.FILTER.text, .metaVariables.single.FIELD.text] | @tsv')

	return "$result"
}
```

### `rule_enforcer.yaml`

No change. The check reads no per-rule data.

## Verification

Scratch under `/tmp/opencode/sty-rust-080/` (untracked). Each fixture root carries `sgconfig.yaml` (`ruleDirs: [rule]`) and the candidate `rule/sty-rust-080.yml` byte-identical to the proposed file; the candidate function is `/tmp/opencode/sty-rust-080/check_sty_rust_080.sh` (byte-identical to `check_sty_rust_080` above, plus one trailing call and `SGCONFIG="${SGCONFIG:-sgconfig.yaml}"`). `bash -n` and `shellcheck -s bash` are clean.

- **Red:** `/tmp/opencode/sty-rust-080/red/src/lib/domain/port/gallery_repository.rs` declares `GalleryFilter { pub id: Option<NumericID>, pub owner_id: Option<NumericID> }`, and `.../red/src/lib/domain/model/gallery.rs` declares `Gallery { gallery_id, user_id }` with accessors `Gallery::gallery_id()`/`Gallery::user_id()`.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-080/red && SGCONFIG=sgconfig.yaml bash /tmp/opencode/sty-rust-080/check_sty_rust_080.sh 2>&1; echo "rc=$?"'`
  observed: `rc=1` with
  ```
  error: STY-RUST-080: GalleryFilter: field id is not named after a field of Gallery (no Gallery::id())
  error: STY-RUST-080: GalleryFilter: field owner_id is not named after a field of Gallery (no Gallery::owner_id())
  ```
  (`id` is the real deviation pattern — the aggregate's identity accessor is `gallery_id()`; `owner_id` is an arbitrary non-accessor.)
- **Green:** `/tmp/opencode/sty-rust-080/green/src/lib/domain/port/gallery_repository.rs` declares `GalleryFilter { pub gallery_id: Option<NumericID>, pub user_id: Option<NumericID> }` against `Gallery::gallery_id()`/`Gallery::user_id()`, and `.../green/src/lib/domain/port/user_repository.rs` declares `UserFilter { pub id: Option<NumericID> }` against `.../green/src/lib/domain/model/user.rs`'s `User::id()`.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-080/green && SGCONFIG=sgconfig.yaml bash /tmp/opencode/sty-rust-080/check_sty_rust_080.sh 2>&1; echo "rc=$?"'`
  observed: `rc=0`, no finding (the foreign-key fields `gallery_id`/`user_id` and the short accessor `id` all pass).
- **Repository:** the candidate rule mounted over the live tree via `/tmp/opencode/sty-rust-080/repocfg/sgconfig.yaml` (`ruleDirs: [/tmp/opencode/sty-rust-080/rule]`), run from the repository root:
  `devenv shell -- bash -c 'SGCONFIG=/tmp/opencode/sty-rust-080/repocfg/sgconfig.yaml bash /tmp/opencode/sty-rust-080/check_sty_rust_080.sh 2>&1; echo "rc=$?"'`
  observed: `rc=1` with
  ```
  error: STY-RUST-080: GalleryFilter: field id is not named after a field of Gallery (no Gallery::id())
  error: STY-RUST-080: GalleryItemFilter: field id is not named after a field of GalleryItem (no GalleryItem::id())
  error: STY-RUST-080: ImageFilter: field id is not named after a field of Image (no Image::id())
  error: STY-RUST-080: VideoFilter: field id is not named after a field of Video (no Video::id())
  error: STY-RUST-080: UploadFilter: field id is not named after a field of Upload (no Upload::id())
  ```
  The extraction is non-vacuous: 166 matches over `src` — 36 public fields (the 8 `*Filter` structs plus the public-fielded `ProbedImage`/`ProbedVideo`, which the `*Filter` guard skips) and 130 public `&self` accessors across the domain aggregates. Every other filter field (`FileFilter.id`↔`File::id()`, `FileFilter.user_id`↔`File::user_id()`, `ImageFilter.file_id`↔`Image::file_id()`, `VideoFilter.color_id`↔`Video::color_id()`, `GalleryItemFilter.media`↔`GalleryItem::media()`, `UserFilter.*`↔`User::*()`, …) passes; only the five identity `id` fields of `Gallery`/`GalleryItem`/`Image`/`Video`/`Upload` are flagged, because their aggregate accessors are `gallery_id`/`gallery_item_id`/`image_id`/`video_id`/`upload_id`. After applying the two `script/rule_enforcer.sh` edits and adding `rule/sty-rust-080.yml`, the registered command is `devenv shell -- script/rule_enforcer.sh check -v`, which reports the same five findings.

## Next

- Apply: `rule/sty-rust-080.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
