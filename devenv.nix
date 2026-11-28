{
  config,
  multiverse,
  pkgs,
  lib,
  inputs,
  ...
}:

let
  # k6 with the `xk6-subcommand-openapi` extension, pinned to the release tag.
  # The project publishes no checksum asset, so "verified" means the tag is
  # pinned here and `script/performance.sh` records `k6 version` in the run
  # fingerprint.
  k6Platform =
    {
      x86_64-linux = "linux_amd64";
      aarch64-linux = "linux_arm64";
      x86_64-darwin = "darwin_amd64";
      aarch64-darwin = "darwin_arm64";
    }
    .${pkgs.stdenv.hostPlatform.system}
      or (throw "no k6 release for ${pkgs.stdenv.hostPlatform.system}");

  k6Hashes = {
    linux_amd64 = "sha256-MJpu0IIqavrfmim1LFdN297ZC3TdhfVYUhpxPhoYFSE=";
    linux_arm64 = "sha256-/NuTjklxWrOLL5nQajEgo6wFeuVI/o/ZLPE/pkzeaPg=";
    darwin_amd64 = "sha256-Rz9/xUd07H4FXfDotGCq1SsFRaMWZNqcGJWt2kn8kPE=";
    darwin_arm64 = "sha256-ZeBoeIrDB+CsyKWlpbvA+WjudP9qGLr9NQMoEaMxzAs=";
  };

  k6 = pkgs.stdenv.mkDerivation {
    pname = "k6-openapi";
    version = "0.1.0";
    src = pkgs.fetchurl {
      url = "https://github.com/grafana/xk6-subcommand-openapi/releases/download/v0.1.0/xk6-subcommand-openapi_v0.1.0_${k6Platform}.tar.gz";
      hash = k6Hashes.${k6Platform};
    };
    sourceRoot = "xk6-subcommand-openapi_v0.1.0_${k6Platform}";
    installPhase = ''
      install -Dm755 k6 $out/bin/k6
    '';
  };
in
{
  env = {
    NO_MKDOCS_2_WARNING = 1;
    LLVM_COV = "${pkgs.llvm}/bin/llvm-cov";
    LLVM_PROFDATA = "${pkgs.llvm}/bin/llvm-profdata";
    DATABASE_URL = "sqlite://dev.db";
    # npm global installs go to a user-writable prefix inside the devenv state
    # directory, so `npm install -g` never needs sudo and is not wiped by nix.
    NPM_CONFIG_PREFIX = "${config.devenv.state}/npm";
    # `uv tool install` binaries go to the same user-writable state directory.
    UV_TOOL_BIN_DIR = "${config.devenv.state}/uv/bin";
  };

  # Put the npm and uv tool prefixes' binaries on PATH so the task-installed
  # CLIs (e.g. openspec, schemathesis) are runnable directly.
  enterShell = ''
    export PATH="$UV_TOOL_BIN_DIR:$NPM_CONFIG_PREFIX/bin:$PATH"
  '';

  # https://devenv.sh/packages/
  packages = [
    pkgs.git
    pkgs.ls-lint
    pkgs.cargo-llvm-cov
    pkgs.llvm
    pkgs.shellcheck
    pkgs.cargo-deny
    pkgs.cargo-about
    pkgs.cargo-machete
    pkgs.cargo-flamegraph
    k6
    pkgs.semgrep
    pkgs.xdg-utils
    pkgs.shfmt
    pkgs.taplo
    pkgs.sqlx-cli
    pkgs.sqlfluff
    pkgs.ast-grep
    pkgs.uv
    pkgs.jq
    pkgs.yq
    multiverse.ffmpeg-headless."6.1.2"
    multiverse.nixfmt."1.5.0"
    multiverse.nodejs."24.19.0"
    multiverse.prettier."3.8.3"
    multiverse.sqlite."3.51.2"
    multiverse.yamllint."1.37.1"
    multiverse.betterleaks."1.8.1"
    multiverse.hadolint."2.13.1"
  ];

  languages.rust = {
    enable = true;
    channel = "stable";
    version = "1.98.0";
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "llvm-tools-preview"
    ];
  };

  languages.python = {
    enable = true;
    version = "3.14";
    venv = {
      enable = true;
      requirements = builtins.readFile ./requirements.txt;
    };
  };

  git-hooks.hooks = {
    # === commit message
    commitizen.enable = true;

    # === shell
    shellcheck.enable = true;
    shfmt.enable = true;
    # === rust
    rustfmt = {
      enable = true;
      pass_filenames = false;
      files = "(\\.rs$|(^|/)Cargo\\.(toml|lock)$|^rustfmt\\.toml$|^\\.rustfmt\\.toml$)";
    };
    clippy = {
      enable = true;
      name = "clippy";
      entry = "cargo clippy --all-targets --all-features -- -D warnings";
      pass_filenames = false; # It work on a package basis
      files = "(\\.rs$|(^|/)Cargo\\.(toml|lock)$|^clippy\\.toml$)";
    };
    clippy-default-features = {
      enable = true;
      name = "clippy (default features)";
      entry = "cargo clippy --all-targets -- -D warnings";
      pass_filenames = false; # It work on a package basis
      files = "(\\.rs$|(^|/)Cargo\\.(toml|lock)$|^clippy\\.toml$)";
    };
    rule-enforcer = {
      enable = true;
      name = "Rulebook checks";
      entry = "script/rule_enforcer.sh check";
      language = "system";
      pass_filenames = false;
      files = "(\\.rs$|\\.md$|^src/|^test/|^docs/|^migrations/.*\\.sql$|^rule/.*|^rule_enforcer\\.yaml$|^devenv\\.nix$|^sgconfig\\.yaml$|^Cargo\\.(toml|lock)$|^requirements\\.txt$|^pyproject\\.toml$|^\\.github/.*|^\\.opencode/.*|^script/.*)";
      fail_fast = true;
    };
    # === python
    ruff = {
      enable = true;
      types = [ "file" ];
      pass_filenames = false;
      files = "(\\.py$|^pyproject\\.toml$|^requirements\\.txt$|^ruff\\.toml$|^\\.ruff\\.toml$)";
    };
    ruff-format = {
      enable = true;
      types = [ "file" ];
      pass_filenames = false;
      files = "(\\.py$|^pyproject\\.toml$|^requirements\\.txt$|^ruff\\.toml$|^\\.ruff\\.toml$)";
    };
    vulture = {
      enable = true;
      name = "vulture";
      entry = "vulture";
      # The config's `paths` own the scope: vulture resolves uses across files, so a
      # changed-file subset would report every cross-module helper as dead.
      pass_filenames = false;
      files = "(\\.py$|^pyproject\\.toml$|^requirements\\.txt$)";
      fail_fast = true;
    };
    # === Nix (This file)
    nixfmt = {
      enable = true;
      package = multiverse.nixfmt."1.5.0";
      fail_fast = true;
    };
    # === file/dir names
    ls-lint = {
      enable = true;
      name = "ls-lint";
      entry = "ls_lint";
      language = "system";
      pass_filenames = false;
      fail_fast = true;
    };
    # === toml
    taplo.enable = true;

    bundle-licenses = {
      enable = true;
      name = "Regenerate THIRD_PARTY_NOTICES.txt";
      entry = "bash -c 'script/generate_third_party_notices.sh && git add -- THIRD_PARTY_NOTICES.txt'";
      language = "system";
      pass_filenames = false;
      files = "(^|/)Cargo\\.(toml|lock)$|^about\\.(toml|hbs)$|^script/generate_third_party_notices\\.sh$";
    };

    # === test coverage
    coverage = {
      enable = true;
      name = "Coverage >= 80%";
      entry = "cargo llvm-cov --lib --features test-seed --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler|src/lib/infrastructure/seed.rs' --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80";
      pass_filenames = false;
      files = "(\\.rs$|(^|/)Cargo\\.(toml|lock)$)";
      fail_fast = true;
    };

    # === sql
    sql = {
      enable = true;
      name = "sqlfluff";
      entry = "sqlfluff lint --dialect sqlite migrations test/api/seed/sql test/performance/seed/sql test/system/seed/sql";
      pass_filenames = false;
      files = "(^migrations/.*\\.sql$|^test/(api|performance|system)/seed/sql/.*\\.sql$|^\\.sqlfluff$|^\\.sqlfluffignore$|^requirements\\.txt$)";
    };

    cargo-deny = {
      enable = true;
      name = "cargo-deny";
      entry = "cargo deny check";
      pass_filenames = false;
      files = "(^|/)Cargo\\.(toml|lock)$|^deny\\.toml$";
    };

    cargo-machete = {
      enable = true;
      name = "cargo-machete";
      entry = "cargo machete";
      pass_filenames = false;
      files = "(\\.rs$|(^|/)Cargo\\.(toml|lock)$)";
    };

    betterleaks = {
      enable = true;
      name = "betterleaks";
      entry = "betterleaks git --pre-commit --staged --redact --verbose";
      pass_filenames = false;
      fail_fast = true;
    };

    investigation = {
      enable = true;
      name = "Verify the requirements coverage report";
      entry = "script/investigation.sh --check";
      pass_filenames = false;
      files = "(openspec/.*|test/system/.*|test/api/scenarios\\.toml|test/util/.*|src/.*\\.rs$|script/investigation\\.sh|requirements\\.txt|docs/development/artifacts/.*)";
      before = [ "documentation" ];
      fail_fast = true;
    };

    documentation = {
      enable = true;
      name = "Verify Mkdocs documentation build";
      entry = "mkdocs build --strict";
      pass_filenames = false;
      files = "(^docs/.*\\.(md|html|json|css|js)$|^mkdocs\\.yml$|^requirements\\.txt$)";
    };

    markdownlint = {
      enable = true;
      package = pkgs.markdownlint-cli2;
      entry = "markdownlint-cli2 '**/*.md'";
      pass_filenames = false;
      files = "(.*\\.md$|^\\.markdownlint-cli2\\.jsonc$)";
      fail_fast = true;
    };
    prettier = {
      enable = true;
      name = "prettier";
      entry = "prettier --write '**/*.md'";
      language = "system";
      types = [ "file" ];
      pass_filenames = false;
      files = "(\\.md$|^\\.prettierrc$|(^|/)\\.prettierignore$|(^|/)\\.editorconfig$)";
      before = [ "markdownlint" ];
      fail_fast = true;
    };
    yamllint = {
      enable = true;
      package = multiverse.yamllint."1.37.1";
      entry = "yamllint -c .yamllint .";
      pass_filenames = false;
      files = "(\\.(yaml|yml)$|^\\.yamllint$)";
      fail_fast = true;
    };

    # === docker
    hadolint = {
      enable = true;
      package = multiverse.hadolint."2.13.1";
      entry = "hadolint";
      files = "(^|/)Dockerfile(\\..*)?$";
      fail_fast = true;
    };

    build = {
      enable = true;
      name = "Build rust code";
      entry = "cargo build";
      files = "(\\.rs$|(^|/)Cargo\\.(toml|lock)$)";
      pass_filenames = false;
      fail_fast = true;
    };

  };

  # Build Commands

  tasks."build:server" = {
    exec = "cargo build --bin server";
    description = "Build the server";
  };

  tasks."build:all" = {
    exec = "cargo build";
    description = "Build everything";
  };

  tasks."docs:html-coverage" = {
    exec = "cargo llvm-cov --lib --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler|src/lib/infrastructure/seed.rs' --html && xdg-open target/llvm-cov/html/index.html";
    description = "Generate HTML coverage report and open it in the browser";
  };

  tasks."test:coverage" = {
    exec = "cargo llvm-cov --lib --ignore-filename-regex 'src/lib/infrastructure/inbound/rest/handler|src/lib/infrastructure/seed.rs'";
    description = "Generate test coverage report";
  };

  tasks."test:api" = {
    exec = "script/api_fuzz.sh";
    description = "Fuzz the API with Schemathesis against a disposable, seeded server";
  };

  # System suite

  tasks."system:client" = {
    exec = "script/openapi_client.sh generate";
    description = "Regenerate the system suite's typed OpenAPI client from the image under test";
  };

  tasks."system:clean" = {
    exec = "script/openapi_client.sh clean";
    description = "Remove the generated system-suite client";
  };

  tasks."system:investigation" = {
    exec = "script/investigation.sh";
    description = "Regenerate the requirement and scenario matrix";
  };

  # Performance suite

  tasks."setup:perf" = {
    exec = "npm ci --prefix test/performance";
    description = "Install the TypeScript toolchain for the performance suite";
  };

  tasks."perf:client" = {
    exec = "script/performance.sh client";
    description = "Build, boot and gate the SUT, then regenerate the typed client";
  };

  tasks."perf:smoke" = {
    exec = "script/performance.sh run smoke --profile all";
    description = "Run the 30 s smoke scenario for every flow";
  };

  tasks."perf:load" = {
    exec = "script/performance.sh run load --profile all";
    description = "Run the 2 min load scenario for every flow";
  };

  tasks."perf:stress" = {
    exec = "script/performance.sh run stress --profile read";
    description = "Run the 100-VU stress ladder with one upload worker";
  };

  tasks."perf:soak" = {
    exec = "script/performance.sh run soak --profile read";
    description = "Run the 15 min read soak";
  };

  tasks."perf:baseline" = {
    exec = "script/performance.sh baseline";
    description = "Run smoke and load for both profiles and write the baseline candidate";
  };

  tasks."perf:flamegraph" = {
    exec = "script/performance.sh flamegraph --profile both";
    description = "Capture the read and write local flamegraphs";
  };

  tasks."perf:clean" = {
    exec = "script/performance.sh clean";
    description = "Remove performance artifacts and the generated client";
  };

  tasks."lint:typescript" = {
    exec = "npm --prefix test/performance run typecheck && npm --prefix test/performance run lint && npm --prefix test/performance run format:check";
    description = "Type-check, lint and format-check the performance suite";
  };

  # Tooling

  tasks."setup:openspec" = {
    exec = "npm install -g --prefix \"$NPM_CONFIG_PREFIX\" @fission-ai/openspec@latest";
    description = "Install the OpenSpec CLI globally into the devenv state prefix";
  };

  tasks."setup:schemathesis" = {
    exec = "uv tool install --force schemathesis==4.33.1";
    description = "Install the Schemathesis CLI as a uv tool";
  };

  # Formatting tasks

  tasks."fmt:nix" = {
    exec = "nixfmt devenv.nix";
    description = "Format this file";
  };

  tasks."fmt:rust" = {
    exec = "cargo fmt";
    description = "Format rust code";
  };

  tasks."fmt:shell" = {
    exec = "shfmt script/*.sh";
    description = "Format shell code";
  };

  tasks."fmt:python" = {
    exec = "ruff format .";
    description = "Format python code";
  };

  tasks."fmt:toml" = {
    exec = "taplo fmt";
    description = "Format toml code";
  };

  tasks."fmt:md" = {
    exec = "prettier --write 'docs/**/*.md' README.md 'openspec/**/*.md'";
    description = "Format markdown code";
  };

  tasks."fmt:sql" = {
    exec = "sqlfluff format --dialect sqlite migrations test/api/seed/sql test/performance/seed/sql test/system/seed/sql";
    description = "Format sql file";
  };

  # Linting

  tasks."lint:rust" = {
    exec = "cargo clippy --all-targets --all-features -- -D warnings && cargo clippy --all-targets -- -D warnings";
    description = "Lint rust code with and without all features";
  };

  tasks."lint:yaml" = {
    exec = "yamllint -c .yamllint .";
    description = "Lint yaml file";
  };

  tasks."lint:sql" = {
    exec = "sqlfluff lint --dialect sqlite migrations test/api/seed/sql test/performance/seed/sql test/system/seed/sql";
    description = "Lint sql file";
  };

  tasks."lint:python" = {
    exec = "ruff check . && vulture";
    description = "Lint python code";
  };

  tasks."lint:md" = {
    exec = "markdownlint-cli2 '**/*.md'";
    description = "Lint markdown file";
  };

  tasks."lint:shell" = {
    exec = "shellcheck script/*.sh";
    description = "Lint shell file";
  };

  # Security

  tasks."security:semgrep" = {
    exec = "mkdir -p .artifacts && semgrep scan --config p/rust --config p/security-audit --config p/cwe-top-25 --config p/github-actions --config security/semgrep --metrics=off --sarif --output .artifacts/semgrep.sarif .";
    description = "Run the Semgrep scan and write the SARIF report to .artifacts/semgrep.sarif";
  };

  tasks."licenses:generate" = {
    exec = "script/generate_third_party_notices.sh";
    description = "Regenerate THIRD_PARTY_NOTICES.txt from the crate graph via cargo-about";
  };

  tasks."deps:cargo-modules" = {
    exec = "cargo install cargo-modules --locked";
    description = "Install cargo modules util";
  };

  # Scripts

  # Launch the Memorium server
  scripts = {
    server.exec = "cargo run --bin server";
  };

  processes = {
    docs.exec = "mkdocs serve --dev-addr 0.0.0.0:8000";
  };

}
