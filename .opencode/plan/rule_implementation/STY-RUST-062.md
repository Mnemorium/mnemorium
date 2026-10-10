# Add check — STY-RUST-062

## Rule guard — STY-RUST-062

- **Rule:** "**Lazy instantiation** — every accessor builds the use case on the spot. It rebuilds its configuration-derived adapters (e.g. password hasher, token provider) from the live configuration and never caches them; it builds its stateless adapters at the point of use and never stores them in a long-lived struct (see [§ 9.2](#92-use-case-factory)). The startup-only settings listed in `STY-RUST-076` are exempt." — `docs/development/TechnicalDesign.md`, row `STY-RUST-062`, section **Use cases** (line 172; elaborated in § 9.2, lines 907–925).
- **Check state:** missing — `script/rule_enforcer.sh uncovered` lists `STY-RUST-062`; there is no `check_sty_rust_062` function and no `RULES=(...)` entry, and no `rule/sty-rust-062.yml`.
- **Mechanism:** bash (cross-file logic) — the rule relates each factory struct's *fields* to the *imports* that name adapters/use cases, and ast-grep cannot relate two nodes (a field and a `use` declaration) in one rule. A pure Bash check reads the file's own imports, then scans the struct block. No ast-grep rule and no `rule_enforcer.yaml` block: the forbidden set is derived from the source, so there is no data to externalize.

## Files

### `script/rule_enforcer.sh`

Two edits; the runner is untouched. The check enforces the two "never caches / never stores" clauses by rejecting any long-lived field typed by a use case or adapter. Those types are exactly the ones the file imports from `crate::application::`, `crate::domain::port::` or `crate::infrastructure::outbound::`; the single exception is `SqlxUnitOfWorkFactory`, which `STY-RUST-061` permits the struct to hold. The live configuration (`crate::domain::model::…::Configuration`), the unit-of-work factory, and startup-only captured values (`PathBuf`, `Arc<Semaphore>`) are therefore all allowed — the `STY-RUST-076` exemption falls out of the import-based definition, since those types are not application-layer imports.

1. Append one registry line to `RULES=(...)`, after the existing `"STY-RUST-024:check_sty_rust_024"` entry (currently the last entry, line 71), before the closing `)`:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-062:check_sty_rust_062"
)
```

2. Add the check function immediately after `check_sty_rust_024()` (ends line 752) and before the `# --- Runner ---` divider (line 754):

```bash
check_sty_rust_062() {
	local id="STY-RUST-062"
	local factory_dir="src/lib/infrastructure/use_case_factory"
	local file name struct_block result=0
	local -a forbidden=()

	if [ ! -d "$factory_dir" ]; then
		printf 'error: %s: missing %s\n' "$id" "$factory_dir" >&2
		return 1
	fi

	while IFS= read -r file; do
		[ -n "$file" ] || continue

		# Types the file imports from the application, domain-port or outbound
		# layers are use cases and adapters: the factory must build them per
		# use, never store them. The unit-of-work factory is the one exception
		# STY-RUST-061 permits the struct to hold.
		forbidden=()
		while IFS= read -r name; do
			[ -n "$name" ] || continue
			[ "$name" = "SqlxUnitOfWorkFactory" ] && continue
			forbidden+=("$name")
		done < <(grep -oE 'crate::(application|domain::port|infrastructure::outbound)::[A-Za-z0-9_:]+' "$file" |
			sed 's/.*:://' | sort -u)

		# The long-lived factory struct's own fields, up to its closing brace.
		struct_block="$(awk '/^pub struct [A-Za-z0-9_]+ ?\{/{f=1} f{print} f&&/^}/{f=0}' "$file")"

		if [ "${#forbidden[@]}" -gt 0 ]; then
			for name in "${forbidden[@]}"; do
				if printf '%s\n' "$struct_block" | grep -qE ":.*\b${name}\b"; then
					printf 'error: %s: %s: the factory struct stores %s; build it in the accessor instead\n' \
						"$id" "$file" "$name" >&2
					result=1
				fi
			done
		fi
	done < <(find "$factory_dir" -maxdepth 1 -name '*.rs' | sort)

	return "$result"
}
```

Notes: the function runs under the runner's `cd -- "$REPO_ROOT"`, so `src/lib/infrastructure/use_case_factory` resolves. It calls no external tool beyond `grep`/`sed`/`awk`/`find`, so it adds no `command -v` install guard. The import regex catches both short-name imports (`use crate::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher;`) and a fully-qualified path written directly in a field type. `rule_enforcer.yaml` and `sgconfig.yaml` are unchanged.

## Verification

The candidate check is `/tmp/opencode/sty-rust-062/check.sh` (the literal function above, invoked from the fixture root). Fixtures mirror the repository layout under `/tmp/opencode/sty-rust-062/{green,red,extra}/src/lib/infrastructure/use_case_factory/`.

- Red: `/tmp/opencode/sty-rust-062/red/` — `identity.rs` with a cached configuration-derived adapter (`password_hasher: Arc<Argon2PasswordHasher>`), `asset.rs` with a cached stateless adapter (`file_storage: Arc<FileSystemStorage>`), and `library.rs` with a cached use case (`create_gallery: Arc<CreateGallery>`). Command: `cd /tmp/opencode/sty-rust-062/red && bash /tmp/opencode/sty-rust-062/check.sh` — observed:
  ```
  error: STY-RUST-062: src/lib/infrastructure/use_case_factory/asset.rs: the factory struct stores FileSystemStorage; build it in the accessor instead
  error: STY-RUST-062: src/lib/infrastructure/use_case_factory/identity.rs: the factory struct stores Argon2PasswordHasher; build it in the accessor instead
  error: STY-RUST-062: src/lib/infrastructure/use_case_factory/library.rs: the factory struct stores CreateGallery; build it in the accessor instead
  ```
  exit `1`. A second red set, `/tmp/opencode/sty-rust-062/extra/`, proves the port-trait form (`password_hasher: Arc<dyn PasswordHasher>` → flags `PasswordHasher`) and the fully-qualified form (`Arc<crate::infrastructure::outbound::argon2::password_hasher::Argon2PasswordHasher>` → flags `Argon2PasswordHasher`); exit `1`.
- Green: `/tmp/opencode/sty-rust-062/green/` — byte-identical copies of the four real factory adapters (`asset.rs`, `identity.rs`, `library.rs`, `user.rs`), which build their adapters and use cases in the accessors and store only `configuration`, `unit_of_work_factory`, `storage_root`, `probe_permits`. Command: `cd /tmp/opencode/sty-rust-062/green && bash /tmp/opencode/sty-rust-062/check.sh` — observed: no output, exit `0`.
- Repository: `cd /home/will/project/mine/Mnemorium/mnemorium && devenv shell -- bash /tmp/opencode/sty-rust-062/check.sh` — observed: no output, exit `0`. All four files under `src/lib/infrastructure/use_case_factory/` are compliant: `identity.rs` builds `Argon2PasswordHasher`/`JwtTokenProvider` per accessor from `self.configuration.load()`, `asset.rs` builds `FileSystemStorage`/`LocalMediaProbe`/`Sha2ContentHasher` at the point of use, and `library.rs`/`user.rs` store only `Arc<SqlxUnitOfWorkFactory>`. Baseline `devenv shell -- script/rule_enforcer.sh list` registers 22 checks and does not include `STY-RUST-062`; after applying the two edits it is registered and passes.

## Next

- Apply: `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
