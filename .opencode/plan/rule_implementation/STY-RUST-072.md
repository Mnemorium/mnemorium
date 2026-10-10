# Add check — STY-RUST-072

## Rule guard — STY-RUST-072

- **Rule:** "Outbound port traits are `'static`; implementations may borrow, but never store a repository beyond the use-case `execute` that obtained it." (`docs/development/TechnicalDesign.md` line 182, row `| STY-RUST-072 | Ports | … | [§ 6.4](#64-port-traits-are-static-implementations-may-borrow) |`; detail § 6.4 "Port traits are `'static`; implementations may borrow" at line 682. The row is present, so `validate_registry` accepts the ID.)
- **Check state:** missing — `script/rule_enforcer.sh list` has no `STY-RUST-072` entry, `uncovered` reports it, and `rule/sty-rust-072.yml` does not exist.
- **Mechanism:** ast-grep — a single error rule scoped with `files:` to the two surfaces the rule names (`src/lib/domain/port/**` for outbound port traits, `src/lib/application/use_case/**` where `execute` lives), plus the harness's thin `--filter` wrapper. Both assertions are local and syntactic (a trait's own parameter list carrying a lifetime; a struct field typed as a repository), so no Bash relation and no `rule_enforcer.yaml` block are needed. The `#[cfg(test)]` exclusion uses the harness's existing `not: inside: mod_item … follows: cfg(test)` idiom.

## Files

### `rule/sty-rust-072.yml` (new)

```yaml
---
id: sty-rust-072
language: Rust
severity: error
message: >-
  outbound port traits are `'static`: a port trait declares no lifetime
  parameter, and a use case never stores a repository beyond its `execute`
files:
  - src/lib/domain/port/**/*.rs
  - src/lib/application/use_case/**/*.rs
rule:
  all:
    - any:
        # An outbound port trait is `'static`: a lifetime parameter on the
        # trait makes it non-'static, so it cannot be injected as `Arc<F>`.
        # `'static` is not a lifetime parameter; a method's own `<...>` generics
        # are excluded by `stopBy: neighbor` on the trait's parameter list.
        - all:
            - kind: type_parameters
            - has:
                kind: lifetime
                not:
                  regex: "^'static$"
                stopBy: end
            - inside:
                kind: trait_item
                stopBy: neighbor
        # A use case never stores a repository: the repository is a short-lived
        # view obtained and dropped inside `execute`, never a struct field.
        # `Repository\b` matches a repository type (`UserRepository`) but not
        # `RepositoryError`.
        - all:
            - kind: field_declaration
            - has:
                field: type
                regex: 'Repository\b'
            - inside:
                kind: struct_item
                stopBy: end
    - not:
        inside:
          kind: mod_item
          stopBy: end
          follows:
            kind: attribute_item
            regex: cfg\(test\)
```

`sgconfig.yaml` already declares `ruleDirs: [rule]`, so no change there.

Two ast-grep mechanics worth recording, both verified below:

- The trait arm matches the trait's *own* parameter list by using `inside: {kind: trait_item, stopBy: neighbor}` — `stopBy: neighbor` means the immediate parent is the `trait_item`, so a method-level `fn peek<'a>(&'a self)` is not caught.
- `not: {regex: "^'static$"}` is required because `'static` parses as a `lifetime` node; a `T: 'static` bound must not count as a lifetime parameter.

### `script/rule_enforcer.sh`

Two edits; the runner is untouched.

**1. Registry** — append one line to `RULES=( … )` after the current last entry (`"STY-RUST-024:check_sty_rust_024"`, line 71):

```diff
 	"STY-RUST-067:check_sty_rust_067"
 	"STY-RUST-024:check_sty_rust_024"
+	"STY-RUST-072:check_sty_rust_072"
 )
```

**2. Check function** — add in the `# --- Checks ---` section, immediately before the `# --- Runner ---` banner (after `check_sty_rust_024`, which ends at line 752):

```bash
check_sty_rust_072() {
	local id="STY-RUST-072"
	local rule_id="sty-rust-072"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

### `rule_enforcer.yaml`

No change. The check reads no per-rule data; the two paths and the `Repository` marker are fixed by the rule text. `yq '.'` parses the candidate rule, and `shellcheck`/`shfmt -d -i 0 -ci` on the candidate function are clean.

## Verification

Scratch proof under `/tmp/opencode/sty-rust-072/` (untracked). Each fixture root is a self-contained project (`sgconfig.yaml` with `ruleDirs: [rule]` + `rule/sty-rust-072.yml` + `src/…`); `check_scratch.sh` holds the candidate function verbatim with `SGCONFIG="${SGCONFIG:-sgconfig.yaml}"` and a driver printing `check rc=$rc`.

- **Red:** `/tmp/opencode/sty-rust-072/red/src/lib/domain/port/bad_port.rs` declares `pub trait BadRepository<'a>: Send + Sync` and `pub trait BadPair<'a, 'b>: Send + Sync`; `/tmp/opencode/sty-rust-072/red/src/lib/application/use_case/bad_use_case.rs` declares `pub struct BadUseCase<F> { users: Arc<dyn UserRepository>, unit_of_work_factory: Arc<F> }`.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-072/red && bash /tmp/opencode/sty-rust-072/check_scratch.sh'`
  observed: `check rc=1` with
  ```
  src/lib/application/use_case/bad_use_case.rs:5:5: error[sty-rust-072]: outbound port traits are `'static`: a port trait declares no lifetime parameter, and a use case never stores a repository beyond its `execute`
  src/lib/domain/port/bad_port.rs:2:24: error[sty-rust-072]: outbound port traits are `'static`: a port trait declares no lifetime parameter, and a use case never stores a repository beyond its `execute`
  src/lib/domain/port/bad_port.rs:7:18: error[sty-rust-072]: outbound port traits are `'static`: a port trait declares no lifetime parameter, and a use case never stores a repository beyond its `execute`
  Error: 3 error(s) found in code.
  ```
  (Both arms fire: the single- and multi-lifetime trait, and the stored repository field.)
- **Green:** `/tmp/opencode/sty-rust-072/green/src/lib/domain/port/good_port.rs` holds `GoodRepository` (no lifetime), `GoodHasher<T: 'static>` (a `'static` bound is not a lifetime parameter), `GoodUnitOfWork` with `fn users(&mut self) -> impl GoodRepository + '_` (`'_` is not a trait-level parameter), and `GoodMethodGeneric` with a method-level `fn peek<'a>(&'a self)`; `/tmp/opencode/sty-rust-072/green/src/lib/application/use_case/good_use_case.rs` holds `GoodUseCase<F>` storing only `unit_of_work_factory` plus a `last_error: Option<RepositoryError>` field (the `RepositoryError` name must not match), and a `#[cfg(test)] mod tests` whose `Harness` stores `Arc<dyn UserRepository>`.
  `devenv shell -- bash -c 'cd /tmp/opencode/sty-rust-072/green && bash /tmp/opencode/sty-rust-072/check_scratch.sh'`
  observed: `check rc=0`, no findings. A control rule identical to the candidate but without the trailing `not:` block flags `good_use_case.rs:16:9` (the `cfg(test)` harness), proving the exclusion is doing the work rather than the file being skipped.
- **Repository:** the candidate rule mounted over the real tree via `/tmp/opencode/sty-rust-072/repocfg/sgconfig.yaml` (`ruleDirs: [/tmp/opencode/sty-rust-072/rule]`):
  `devenv shell -- bash -c 'cd /home/will/project/mine/Mnemorium/mnemorium && SGCONFIG=/tmp/opencode/sty-rust-072/repocfg/sgconfig.yaml bash /tmp/opencode/sty-rust-072/check_scratch.sh'`
  observed: `check rc=0`, no findings. Non-vacuity probes with the same config: an extraction rule over the same `files:` globs counts **24** outbound port traits under `src/lib/domain/port/**` (none declares a lifetime parameter) and **52** non-test struct fields under `src/lib/application/use_case/**` (none typed as a repository). After applying the two edits, `devenv shell -- script/rule_enforcer.sh check -v` registers `STY-RUST-072` in place of its `uncovered` listing and reports it clean.

## Next

- Apply: `rule/sty-rust-072.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
