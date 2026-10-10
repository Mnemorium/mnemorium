# Add check — STY-RUST-065

## Rule guard — STY-RUST-065

- **Rule:** "Never cache a configuration-derived value (pepper, `JWT` secret, TTL) in a long-lived adapter built at startup; read it from the live configuration. The startup-only settings listed in `STY-RUST-076` are the only exception." — `docs/development/TechnicalDesign.md`, row `STY-RUST-065`, section **Configuration** (line 175; elaborated in § 10 "Configuration", lines 935–941, which names the compliant pattern: the auth middleware builds its `JwtTokenProvider` from the live configuration through `FromRef`).
- **Check state:** missing — `script/rule_enforcer.sh list` registers 22 checks and `uncovered` still lists `STY-RUST-065`; there is no `check_sty_rust_065` function, no `RULES=(...)` entry, and no `rule/sty-rust-065.yml`.
- **Mechanism:** ast-grep + bash — the rule's syntactic half (a field of a long-lived adapter struct) is an ast-grep extraction over the long-lived adapter surfaces; the semantic half (does that field hold a configuration-derived value?) is the Bash wrapper's decision, which an ast-grep rule cannot make. No `rule_enforcer.yaml` block: the check holds no forward-extensible data, and `sgconfig.yaml` already declares `ruleDirs: [rule]`.

### Interpretation (why the check is shaped this way)

The caching the rule forbids is exactly a **struct field**: an adapter only "caches" a configuration-derived value by storing it. The long-lived adapter surfaces in this codebase are `AppState` (`src/lib/infrastructure/inbound/rest/app_state.rs`), the middleware structs (`src/lib/infrastructure/inbound/rest/middleware/*.rs`, e.g. `LoginRateLimiter`), and the `Runtime<Context>UseCaseFactory` structs (`src/lib/infrastructure/use_case_factory/*.rs`). The rule's named values — pepper, JWT secret, TTL — are carried by exactly two adapter types, `Argon2PasswordHasher` (`pepper: Vec<u8>`) and `JwtTokenProvider` (`secret: String`, `ttl: u64`), plus scalars a future adapter might name `pepper`/`secret`/`ttl` directly.

The per-use adapters in `src/lib/infrastructure/outbound/` (`argon2/password_hasher.rs`, `jwt/token_provider.rs`) legitimately hold those fields — the compliant design is to rebuild them per use from `Arc<ArcSwap<Configuration>>` (`STY-RUST-062`), so their own definitions must be out of scope; the rule is violated only if such a value is stored on a **long-lived** surface. The startup-only settings of `STY-RUST-076` (`security.rate_limit.*`, `asset.storage.root`, …) fall outside the forbidden name/type set, so the `STY-RUST-076` exemption holds without special-casing.

## Files

### `rule/sty-rust-065.yml` (new)

New ast-grep extraction rule (severity `hint`, like `sty-rust-006`/`sty-rust-013`/`sty-rust-019`/`sty-rust-023`). It captures every field declaration of a struct on the long-lived adapter surfaces, skipping structs inside `#[cfg(test)]` modules (the `sty-rust-067` idiom). `sgconfig.yaml` registers it automatically; no `sgconfig.yaml` change.

```yaml
---
id: sty-rust-065
language: Rust
severity: hint
message: >-
  extract the fields of a long-lived adapter struct, for STY-RUST-065
files:
  - src/lib/infrastructure/inbound/rest/app_state.rs
  - src/lib/infrastructure/inbound/rest/middleware/*.rs
  - src/lib/infrastructure/use_case_factory/*.rs
rule:
  all:
    - kind: field_declaration
    - pattern: '$FIELD: $TYPE'
    - inside:
        kind: field_declaration_list
        inside:
          kind: struct_item
          stopBy: end
    - not:
        inside:
          kind: mod_item
          stopBy: end
          follows:
            kind: attribute_item
            regex: 'cfg\(test\)'
```

### `script/rule_enforcer.sh`

Two edits; the runner is untouched.

1. Append one registry line to `RULES=(...)`, after the existing `"STY-RUST-024:check_sty_rust_024"` entry (currently the last entry, line 71), before the closing `)`:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-065:check_sty_rust_065"
)
```

2. Add the check function immediately after `check_sty_rust_024()` (ends line 752) and before the `# --- Runner ---` divider (line 754):

```bash
check_sty_rust_065() {
	local id="STY-RUST-065"
	local rule_id="sty-rust-065"
	local long_lived="src/lib/infrastructure"
	local matches file field type result=0
	local -a forbidden_names=("pepper" "secret" "ttl" "jwt_secret" "jwt_ttl")

	if [ ! -d "$long_lived" ]; then
		printf 'error: %s: missing %s\n' "$id" "$long_lived" >&2
		return 1
	fi

	# sty-rust-065 extracts every field of a long-lived adapter struct in the
	# inbound layer and the use-case factories. The value check happens here
	# because an ast-grep rule cannot decide whether a field's value was read
	# from the configuration. The per-use adapters (Argon2PasswordHasher,
	# JwtTokenProvider in src/lib/infrastructure/outbound) are deliberately out
	# of scope: the rule requires them to be rebuilt from the live configuration,
	# so only the long-lived surfaces are inspected.
	if ! matches="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--json=stream src 2>&1)"; then
		printf '%s\n' "$matches"
		return 1
	fi
	if [ -z "$matches" ]; then
		printf 'error: %s: extracted no long-lived adapter field from %s\n' "$id" "$long_lived" >&2
		return 1
	fi

	while IFS=$'\t' read -r file field type; do
		[ -n "$field" ] || continue
		if printf '%s' "$type" | grep -Eq '(^|[^A-Za-z0-9_])(JwtTokenProvider|Argon2PasswordHasher)([^A-Za-z0-9_]|$)'; then
			printf 'error: %s: %s: long-lived adapter caches %s in field %s; read it from the live configuration\n' \
				"$id" "$file" "$type" "$field" >&2
			result=1
			continue
		fi
		for name in "${forbidden_names[@]}"; do
			if [ "$field" = "$name" ]; then
				printf 'error: %s: %s: long-lived adapter caches the configuration value %s; read it from the live configuration\n' \
					"$id" "$file" "$field" >&2
				result=1
			fi
		done
	done < <(printf '%s\n' "$matches" |
		jq -r '[.file, .metaVariables.single.FIELD.text, (.metaVariables.single.TYPE.text | gsub("\\s+"; ""))] | @tsv')

	return "$result"
}
```

Notes: the function runs under the runner's `cd -- "$REPO_ROOT"`, so `$SGCONFIG` (`sgconfig.yaml`) and `src` resolve. It calls only `ast-grep`/`jq`/`grep`, all dev-shell tools, so it adds no `command -v` install guard. The type match strips whitespace and anchors on non-identifier boundaries, so it catches `Arc<JwtTokenProvider>`, `Option<Argon2PasswordHasher>`, and a fully-qualified path ending in either name. `rule_enforcer.yaml` is unchanged.

## Verification

Candidate rule: `/tmp/opencode/STY-RUST-065/rule/sty-rust-065.yml` (byte-identical to the file above). Candidate check: `/tmp/opencode/STY-RUST-065/check.sh` (the literal function above plus a driver that `cd`s into a fixture root and runs `check_sty_rust_065`). Each fixture carries its own `sgconfig.yaml` (`ruleDirs: [rule]`) and `rule/sty-rust-065.yml`, so ast-grep resolves the rule from a non-ancestor project root. All runs were invoked from the repository root via `devenv shell --`.

- **Red (cached secret adapter):** `/tmp/opencode/STY-RUST-065/red_a` — a copy of the real `src/lib/infrastructure/inbound/rest/app_state.rs` with an injected field `token_provider: Arc<JwtTokenProvider>`. Command: `SGCONFIG=/tmp/opencode/STY-RUST-065/red_a/sgconfig.yaml devenv shell -- bash /tmp/opencode/STY-RUST-065/check.sh /tmp/opencode/STY-RUST-065/red_a` — observed:
  ```
  error: STY-RUST-065: src/lib/infrastructure/inbound/rest/app_state.rs: long-lived adapter caches Arc<JwtTokenProvider> in field token_provider; read it from the live configuration
  rc=1
  ```
- **Red (cached pepper scalar):** `/tmp/opencode/STY-RUST-065/red_b` — a copy of `src/lib/infrastructure/use_case_factory/identity.rs` with an injected field `pepper: Vec<u8>`. Same command with `red_b` — observed:
  ```
  error: STY-RUST-065: src/lib/infrastructure/use_case_factory/identity.rs: long-lived adapter caches the configuration value pepper; read it from the live configuration
  rc=1
  ```
- **Red-test exclusion is load-bearing:** `/tmp/opencode/STY-RUST-065/red_test_only` — `src/lib/infrastructure/inbound/rest/middleware/fixture.rs` with a compliant `pub struct RealAdapter` plus a `#[cfg(test)]` module whose `TestFixture` holds `jwt_secret: String` and `token_provider: Arc<JwtTokenProvider>`. Same command with `red_test_only` — observed: no finding, `rc=0` (the `not: inside cfg(test) mod_item` clause excludes the test fixture; without it the same tree is flagged).
- **Green:** `/tmp/opencode/STY-RUST-065/green` — byte copies of the real `app_state.rs`, `middleware/rate_limit.rs`, `use_case_factory/identity.rs`, and `use_case_factory/asset.rs`; their fields are `configuration`, `unit_of_work_factory`, `storage_root`, `probe_permits`, `header`, `trusted`, `prunes`, `apply`, `cleanup`, …, none forbidden. Same command with `green` — observed: no finding, `rc=0`. Direct engine check: `devenv shell -- ast-grep scan --config /tmp/opencode/STY-RUST-065/green/sgconfig.yaml --filter sty-rust-065 --report-style=short --color=never src` — observed: help-level extraction matches only, exit `0`.
- **Repository:** `SGCONFIG=/tmp/opencode/STY-RUST-065/sgconfig.yaml devenv shell -- bash /tmp/opencode/STY-RUST-065/check.sh /home/will/project/mine/Mnemorium/mnemorium` — observed: no finding, `rc=0`. The extraction rule matches only allowed long-lived fields (`AppState.configuration`, `Runtime*UseCaseFactory.{configuration,unit_of_work_factory,storage_root,probe_permits}`, `TrustedProxyKeyExtractor.{header,trusted}`, `LoginRateLimiter.{apply,cleanup}`, `RateLimitCleanup.prunes`); the JWT secret/TTL are read from `Arc<ArcSwap<Configuration>>` per request (`app_state.rs` `FromRef`, `use_case_factory/identity.rs` accessors). Baseline: `devenv shell -- script/rule_enforcer.sh list | wc -l` → `22`; `... uncovered | grep -c 'STY-RUST-065'` → `1`. After applying the two edits the rule is registered and runs in place of its `uncovered` listing.

## Next

- Apply: `rule/sty-rust-065.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
