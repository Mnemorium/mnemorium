# Add check — STY-RUST-051

## Rule guard — STY-RUST-051

- **Rule:** "Declare the filter struct (e.g. `UserFilter`) first in a repository port file." (`docs/development/TechnicalDesign.md`, § 8.3 "Declaration order in a repository port file", row `STY-RUST-051`, line 161)
- **Check state:** missing — `devenv shell -- script/rule_enforcer.sh uncovered` lists `STY-RUST-051`; it is absent from the `RULES=(...)` registry and no `check_sty_rust_051` function exists, and `rule/sty-rust-051.yml` does not exist.
- **Mechanism:** ast-grep — a relational rule (`struct_item … follows trait_item`, `stopBy: end`) expresses "a `*Filter` struct declared after the repository trait" directly; the thin Bash wrapper mirrors `check_sty_rust_003`.
- **Clarification:** This check is **self-contained**. It deliberately does **not** call `script/check_repository_declaration_order.sh`, because that standalone script is slated for removal. It enforces STY-RUST-051 entirely inside `script/rule_enforcer.sh` through `rule/sty-rust-051.yml` plus the wrapper function below, and it adds **no** `rule_enforcer.yaml` block. (`sgconfig.yaml` already sets `ruleDirs: [rule]`, so it needs no edit either.)

## Files

### `rule/sty-rust-051.yml` (new)

```yaml
---
id: sty-rust-051
language: Rust
severity: error
message: a *Filter struct is declared after the repository trait
rule:
  all:
    - kind: struct_item
    - pattern: pub struct $FILTER
    - regex: '^pub struct \w+Filter\b'
    - follows:
        kind: trait_item
        pattern: 'pub trait $TRAIT'
        stopBy: end
```

Notes: `follows: {kind: trait_item, stopBy: end}` matches a `*Filter` struct that has the repository trait declared before it anywhere in the file — exactly the violation; a compliant file (filter first) has no preceding trait, so nothing matches. `severity: error` is required so the harness's `if output="$(ast-grep scan …)"` exit-code idiom sees a non-zero exit; a `hint`/`warning` severity would exit `0` even on a finding. This mirrors `sty-rust-013.yml`/`sty-rust-019.yml`, which likewise use `follows … stopBy: end`.

### `script/rule_enforcer.sh`

Two additions, both inside this file. The runner is never edited.

1. Register the check by appending one line to `RULES=(...)`, immediately after the existing `"STY-RUST-024:check_sty_rust_024"` entry (line 71), before the closing `)`:

```bash
	"STY-RUST-051:check_sty_rust_051"
```

2. Declare the check function immediately after `check_sty_rust_024()` (which ends at line 752) and before the `# --- Runner ---` divider (line 754):

```bash
check_sty_rust_051() {
	local id="STY-RUST-051"
	local rule_id="sty-rust-051"
	local output

	if output="$(ast-grep scan --config "$SGCONFIG" --filter "$rule_id" \
		--globs 'src/lib/domain/port/*_repository.rs' \
		--report-style=short --color=never src 2>&1)"; then
		return 0
	fi
	printf '%s\n' "$output"
	return 1
}
```

Notes: the runner already `cd`s to `REPO_ROOT`, so `$SGCONFIG` (`sgconfig.yaml`) and the cwd-relative `--globs` resolve; the glob scopes the rule to `src/lib/domain/port/*_repository.rs`, as `check_sty_rust_003` scopes with its own `--globs`. The function calls the tool directly, with no `command -v` install guard; a non-zero exit carries the ast-grep diagnostic out through `output` for the runner to indent.

## Verification

- Red: `/tmp/opencode/rg051/red/src/lib/domain/port/user_repository.rs` (trait before the filter struct) —

  ```rust
  pub trait UserRepository: Send + Sync {
      fn search(&mut self, filter: &UserFilter);
  }

  pub struct UserFilter {
      pub name: Option<String>,
  }
  ```

  `devenv shell -- bash -c "cd /tmp/opencode/rg051/red && ast-grep scan --rule /tmp/opencode/rg051/rules/sty-rust-051.yml --globs 'src/lib/domain/port/*_repository.rs' --report-style=short --color=never src"` — observed: `src/lib/domain/port/user_repository.rs:5:1: error[sty-rust-051]: a *Filter struct is declared after the repository trait`, exit `1`.

- Green: `/tmp/opencode/rg051/green/src/lib/domain/port/user_repository.rs` (filter struct first, trait second) —

  ```rust
  pub struct UserFilter {
      pub name: Option<String>,
  }

  pub trait UserRepository: Send + Sync {
      fn search(&mut self, filter: &UserFilter);
  }
  ```

  `devenv shell -- bash -c "cd /tmp/opencode/rg051/green && ast-grep scan --rule /tmp/opencode/rg051/rules/sty-rust-051.yml --globs 'src/lib/domain/port/*_repository.rs' --report-style=short --color=never src"` — observed: no finding, exit `0`.

- Repository: `devenv shell -- ast-grep scan --config /tmp/opencode/rg051/repo-sgconfig.yaml --filter sty-rust-051 --globs 'src/lib/domain/port/*_repository.rs' --report-style=short --color=never src` from the repository root — observed: exit `0`, no finding. The scratch config differs from the committed `sgconfig.yaml` only in `ruleDirs` pointing at the candidate rule (the plan is not applied); once applied the harness runs this same invocation with `$SGCONFIG`. A probe rule over the same `--globs` confirmed the scan reaches all eight `*Filter` structs in the eight `src/lib/domain/port/*_repository.rs` files (`FileFilter`, `GalleryFilter`, `GalleryItemFilter`, `UploadFilter`, `CredentialFilter`, `UserFilter`, `ImageFilter`, `VideoFilter`), so the green run exercises the check on the real surface.

## Next

- Apply: `rule/sty-rust-051.yml` (new), `script/rule_enforcer.sh`
- Verify: `devenv shell -- script/rule_enforcer.sh check -v`
