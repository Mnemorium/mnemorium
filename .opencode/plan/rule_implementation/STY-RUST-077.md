# Add check — STY-RUST-077

## Rule guard — STY-RUST-077

- **Rule:** **Validated deserialization**: a domain model that validates in `try_new` must not derive a `Deserialize` that bypasses it. Route serde through the validating constructor (`#[serde(try_from = "…")]` plus `TryFrom`, or a custom `Deserialize`) so every construction path enforces the same invariant.
- **Check state:** missing — `STY-RUST-077` has no registry entry (`list` shows 0) and appears in `uncovered`.
- **Mechanism:** bash — the check relates a struct's own attribute run (siblings in the CST) to the struct it annotates and to the inherent `impl` that declares `try_new`; ast-grep cannot bound attribute↔item association (`follows`/`precedes` with `stopBy: end` scan across unrelated items, and multi-node patterns are rejected), so a pure Bash `awk` scan is the smallest faithful check.

## Files

### `script/rule_enforcer.sh`

Add one registry line after `"STY-RUST-024:check_sty_rust_024"` (line 71):

```
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-077:check_sty_rust_077"
 )
```

Add the check function after `check_sty_rust_024` (ends line 752) and before `# --- Runner ---` (line 754):

```bash
check_sty_rust_077() {
	local id="STY-RUST-077"
	local model_dir="src/lib/domain/model"
	local output
	local -a files=()

	if [ ! -d "$model_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	mapfile -t files < <(find "$model_dir" -maxdepth 1 -name '*.rs' | sort)
	if [ "${#files[@]}" -eq 0 ]; then
		printf 'error: %s: no model files under %s\n' "$id" "$model_dir" >&2
		return 1
	fi

	# STY-RUST-077 pairs a model struct's own attributes with the inherent
	# `impl` that declares `try_new`. The attribute-to-item association and
	# the struct-to-impl pairing are both beyond ast-grep, so the scan is
	# pure Bash: gawk walks each file, binds an attribute run to the item it
	# annotates, and records which types declare `try_new`.
	if output="$(awk '
		function self_type(s,   t, idx) {
			t = s
			sub(/^[ \t]*impl/, "", t)
			idx = index(t, " for ")
			if (idx > 0) t = substr(t, idx + 5)
			sub(/^[ \t]+/, "", t)
			if (match(t, /^[A-Za-z_][A-Za-z0-9_]*/)) return substr(t, 1, RLENGTH)
			return ""
		}

		FNR == 1 {
			attrs = ""
			attr_depth = 0
			in_impl = 0
			impl_depth = 0
			impl_type = ""
		}

		# An attribute opens with `#[`/`#![` and may span lines until its `]`.
		attr_depth > 0 || $0 ~ /^[ \t]*#!?\[/ {
			attrs = attrs "\n" $0
			line = $0; opens = gsub(/\[/, "", line)
			line = $0; closes = gsub(/\]/, "", line)
			attr_depth += opens - closes
			if (attr_depth < 0) attr_depth = 0
			next
		}

		# Blank lines and comments do not end an attribute run.
		$0 ~ /^[ \t]*$/ || $0 ~ /^[ \t]*\/\// { next }

		$0 ~ /^[ \t]*pub struct [A-Za-z_]/ {
			line = $0
			sub(/^[ \t]*pub struct[ \t]+/, "", line)
			match(line, /^[A-Za-z_][A-Za-z0-9_]*/)
			name = substr(line, 1, RLENGTH)
			key = FILENAME SUBSEP name
			names[key] = name
			files_of[key] = FILENAME
			if (attrs ~ /(^|[^A-Za-z0-9_])Deserialize([^A-Za-z0-9_]|$)/) deser[key] = 1
			if (attrs ~ /serde\(try_from/) routed[key] = 1
			attrs = ""
			next
		}

		# Any other code line ends the attribute run and may open an impl.
		{
			if (!in_impl && $0 ~ /^[ \t]*impl[^A-Za-z0-9_]/) {
				in_impl = 1
				impl_depth = 0
				impl_type = self_type($0)
			}
			if (in_impl && impl_type != "" && $0 ~ /fn[ \t]+try_new[ \t]*\(/) {
				validates[FILENAME SUBSEP impl_type] = 1
			}
			line = $0; opens = gsub(/\{/, "", line)
			line = $0; closes = gsub(/\}/, "", line)
			if (in_impl) {
				impl_depth += opens - closes
				if (impl_depth <= 0) {
					in_impl = 0
					impl_depth = 0
					impl_type = ""
				}
			}
			attrs = ""
			next
		}

		END {
			bad = 0
			for (key in names) {
				if (deser[key] && !routed[key] && validates[key]) {
					printf "error: STY-RUST-077: %s: %s derives Deserialize but validates in try_new; route serde through #[serde(try_from = \"…\")] plus TryFrom\n", files_of[key], names[key]
					bad = 1
				}
			}
			exit bad
		}
	' "${files[@]}" 2>&1)"; then
		return 0
	fi

	printf '%s\n' "$output"
	return 1
}
```

No `rule/sty-rust-077.yml` is added and no `rule_enforcer.yaml` block is added: the mechanism is neither an ast-grep pattern nor config-driven data.

## Verification

- Red: `/tmp/opencode/sty-rust-077/red/src/lib/domain/model/widget.rs` (a `pub struct` deriving `serde::Deserialize` with an `impl Widget { pub fn try_new … }` and no `#[serde(try_from)]`) — `devenv shell -- bash /tmp/opencode/sty-rust-077/check.sh /tmp/opencode/sty-rust-077/red` — observed: rc=1 and `error: STY-RUST-077: src/lib/domain/model/widget.rs: Widget derives Deserialize but validates in try_new; route serde through #[serde(try_from = "…")] plus TryFrom`.
- Red (adjacency, proves no contamination from an earlier routed struct): `/tmp/opencode/sty-rust-077/red2/src/lib/domain/model/adjacent.rs` (routed `First` followed by plain `Second` that has `try_new`) — same command with `…/red2` — observed: rc=1 and the finding names only `Second`.
- Red (multi-line derive plus field-level `#[serde(default)]`): `/tmp/opencode/sty-rust-077/red3/src/lib/domain/model/gadget.rs` — same command with `…/red3` — observed: rc=1 and the finding names `Gadget`.
- Green: `/tmp/opencode/sty-rust-077/green/src/lib/domain/model/` (a routed model, a `Deserialize` model using infallible `new`, a `Deserialize` enum, a model with `try_new` and no `Deserialize`, and a custom `impl Deserialize` with `try_new`) — `devenv shell -- bash /tmp/opencode/sty-rust-077/check.sh /tmp/opencode/sty-rust-077/green` — observed: rc=0, no finding.
- Repository: `devenv shell -- bash /tmp/opencode/sty-rust-077/check.sh /home/will/project/mine/Mnemorium/mnemorium` — observed: rc=0, no findings. Classification of the real `src/lib/domain/model/*.rs` (via the same parser) confirms it is not trivially green: `Security`, `Jwt`, `RateLimit`, `Sqlite3`, `Logging`, `AssetStorage`, `AssetUpload` are `deser=1 routed=1 try_new=1`; `Asset`, `Configuration`, `Persistence` are `deser=1 try_new=0`; the data models (`User`, `Video`, `Image`, `Audio`, …) are `deser=0 try_new=1`. No struct is `deser=1 routed=0 try_new=1`.

## Next

- Apply: `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
