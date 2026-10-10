# Add check — STY-RUST-042

## Rule guard — STY-RUST-042

- **Rule:** "Declare the `UnitOfWork` trait first in `domain/port/unit_of_work.rs`." (`docs/development/TechnicalDesign.md` line 152; section `Unit of Work`, [§ 7.4](#74-declaration-order-in-the-unit-of-work-port-file))
- **Check state:** missing — `STY-RUST-042` has no entry in `RULES` and there is no `rule/sty-rust-042.yml`; the rulebook row parses (confirmed via the enforcer simulation below).
- **Mechanism:** ast-grep — a sibling `follows` relation expresses the declaration order the rule names; the violation is a `UnitOfWorkFactory` trait not preceded by a `UnitOfWork` trait, i.e. `UnitOfWork` is not declared first. Scoped to the one file by `files:`, so no `rule_enforcer.yaml` block is needed (the target path is fixed by the rule text).

## Files

### `rule/sty-rust-042.yml`

New ast-grep rule. `severity: error` makes `ast-grep scan` exit non-zero on a match, which the Bash check relies on. A bare `pattern: pub trait UnitOfWorkFactory` does **not** match a trait that carries a body, so the name is matched with a regex anchored at the item start; `\b` keeps `UnitOfWork` from matching `UnitOfWorkFactory`.

```yaml
---
id: sty-rust-042
language: Rust
severity: error
message: >-
  declare the UnitOfWork trait first in
  src/lib/domain/port/unit_of_work.rs, before UnitOfWorkFactory
files:
  - src/lib/domain/port/unit_of_work.rs
rule:
  all:
    - kind: trait_item
    # The factory is the offending node: it must be preceded by UnitOfWork.
    - regex: '^pub trait UnitOfWorkFactory\b'
    # `follows` is the sibling relation; `stopBy: end` skips doc comments and
    # other intervening siblings. A body-carrying trait is not matched by a bare
    # `pattern`, so the name is matched with a regex anchored at the item start.
    - not:
        follows:
          kind: trait_item
          regex: '^pub trait UnitOfWork\b'
          stopBy: end
```

### `script/rule_enforcer.sh`

One registry line, appended after the `STY-RUST-024` entry:

```bash
	"STY-RUST-024:check_sty_rust_024"
	"STY-RUST-042:check_sty_rust_042"
)
```

One check function, inserted immediately before the `# --- Runner ---` comment (after `check_sty_rust_024`). It mirrors `check_sty_rust_018`: guard the target's existence, then run the filtered scan and surface its output on failure.

```bash
check_sty_rust_042() {
	local id="STY-RUST-042"
	local rule_id="sty-rust-042"
	local target="src/lib/domain/port/unit_of_work.rs"
	local output

	if [ ! -f "$target" ]; then
		printf 'error: %s: missing %s\n' "$id" "$target" >&2
		return 1
	fi

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never "$target" 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

## Verification

- **Red** — `/tmp/opencode/rg042/proj-red/src/lib/domain/port/unit_of_work.rs` (a `UnitOfWorkFactory` trait declared before `UnitOfWork`):
  `devenv shell -- ast-grep scan --config sgconfig.yaml --filter sty-rust-042 --report-style=short --color=never src/lib/domain/port/unit_of_work.rs` (run from `/tmp/opencode/rg042/proj-red`) — observed: `src/lib/domain/port/unit_of_work.rs:4:1: error[sty-rust-042]: declare the UnitOfWork trait first in src/lib/domain/port/unit_of_work.rs, before UnitOfWorkFactory`, exit 1.
- **Red (trait absent)** — `/tmp/opencode/rg042/proj-missing/src/lib/domain/port/unit_of_work.rs` (only `UnitOfWorkFactory`, no `UnitOfWork`): same command from `/tmp/opencode/rg042/proj-missing` — observed: the same `error[sty-rust-042]` finding, exit 1.
- **Green** — `/tmp/opencode/rg042/proj-green/src/lib/domain/port/unit_of_work.rs` (`UnitOfWork` then `UnitOfWorkFactory`): same command from `/tmp/opencode/rg042/proj-green` — observed: no finding, exit 0.
- **Repository** — `devenv shell -- ast-grep scan --rule /tmp/opencode/rg042/rule/sty-rust-042.yml --report-style=short --color=never src/lib/domain/port/unit_of_work.rs` (from the repository root) — observed: no finding, exit 0; the real file declares `pub trait UnitOfWork` at line 10 before `pub trait UnitOfWorkFactory` at line 19, so the rule holds. (`--config`/`--filter` cannot be exercised until the rule file is applied, since `sgconfig.yaml` has no `sty-rust-042` yet.)
- **Enforcer end-to-end (scratch copy)** — `devenv shell -- bash /tmp/opencode/rg042/enforcer/script/rule_enforcer.sh check -v` — observed: `ok STY-RUST-042 (Unit of Work)` with the real file (green); after swapping in the red file, `FAIL STY-RUST-042 (Unit of Work) — Declare the \`UnitOfWork\` trait first in \`domain/port/unit_of_work.rs\`.` with the finding indented, exit 1. The rulebook row and registry entry validate.

## Next

- Apply: `rule/sty-rust-042.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
