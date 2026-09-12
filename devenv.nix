{
  multiverse,
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

{
  env = {
    NO_MKDOCS_2_WARNING = 1;
    LLVM_COV = "${pkgs.llvm}/bin/llvm-cov";
    LLVM_PROFDATA = "${pkgs.llvm}/bin/llvm-profdata";
    DATABASE_URL = "sqlite://dev.db";
  };

  # https://devenv.sh/packages/
  packages = [
    pkgs.git
    pkgs.ls-lint
    pkgs.cargo-llvm-cov
    pkgs.nixfmt
    pkgs.llvm
    pkgs.shellcheck
    pkgs.cargo-audit
    pkgs.xdg-utils
    pkgs.shfmt
    pkgs.taplo
    pkgs.sqlx-cli
    pkgs.sqlfluff
    multiverse.nodejs."24.19.0"
    multiverse.prettier."3.8.3"
    multiverse.sqlite."3.51.2"
    multiverse.yamllint."1.37.1"
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
      requirements = ''
        pytest==9.1.1
        mkdocs==1.6.0
        mkdocs-material==9.7.7
        mkdocs_puml==2.3.0
        neoteroi-mkdocs==1.2.0
        mkdocs-linkcheck==1.0.6
        ruff==0.16.5
        requests==2.34.2
      '';
    };
  };

  git-hooks.hooks = {
    # === shell
    shellcheck.enable = true;
    shfmt.enable = true;
    # === rust
    rustfmt.enable = true;
    clippy = {
      enable = true;
      name = "clippy";
      entry = "cargo clippy --all-targets --all-features -- -D warnings";
      language = "rust";
      pass_filenames = false; # It work on a package basis
      files = "\\.rs$";
    };
    # === python
    ruff.enable = true;
    ruff-format.enable = true;
    # === Nix (This file)
    nixfmt.enable = true;
    # === file/dir names
    ls-lint = {
      enable = true;
      name = "ls-lint";
      entry = "ls-lint";
      language = "system";
      pass_filenames = false;
      types = [
        "file"
        "symlink"
        "directory"
      ];
    };
    # === toml
    taplo.enable = true;
    # === test coverage
    coverage = {
      enable = true;
      name = "Coverage >= 80%";
      entry = "cargo llvm-cov --lib --fail-under-functions 80 --fail-under-regions 80 --fail-under-lines 80";
      pass_filenames = false;
      files = "\\.rs$";
      language = "rust";
    };

    # === sql
    sql = {
      enable = true;
      name = "sqlfluff";
      entry = "sqlfluff lint --dialect sqlite";
      files = "migrations/.*\\.sql$";
    };

    cargo-audit = {
      enable = true;
      name = "cargo-audit";
      entry = "cargo audit";
      pass_filenames = false;
      files = "Cargo\\.toml$";
    };

    documentation = {
      enable = true;
      name = "Verify Mkdocs documentation build";
      entry = "mkdocs build --strict";
      pass_filenames = false;
      files = "docs/.*\\.md$";
    };

    markdownlint = {
      enable = true;
      package = pkgs.markdownlint-cli2;
      entry = "markdownlint-cli2";
      files = ".*\\.md$";
    };
    mkdocs-link = {
      enable = true;
      entry = "mkdocs-linkcheck --files";
      files = "docs/.*\\.md$";
    };

    yamllint = {
      enable = true;
      package = multiverse.yamllint."1.37.1";
      entry = "yamllint -c .yamllint";
      files = "\\.(yaml|yml)$";
    };

    build = {
      enable = true;
      entry = "cargo build";
      files = "\\.rs$";
    };

  };

  # Build Commands

  tasks."build:openapi-gen" = {
    exec = "cargo build --bin openapi_gen";
    description = "Build the OpenAPI generator";
  };

  tasks."build:server" = {
    exec = "cargo build --bin server";
    description = "Build the server";
  };

  tasks."build:all" = {
    exec = "cargo build";
    description = "Build everything";
  };

  tasks."docs:openapi-gen" = {
    exec = "cargo run --bin openapi_gen";
    after = [ "build:openapi-gen" ];
    description = "Generate the OpenAPI specification to docs/development/api/openapi.json";
  };

  tasks."docs:html-coverage" = {
    exec = "cargo llvm-cov --lib --html && xdg-open target/llvm-cov/html/index.html";
    description = "Generate HTML coverage report and open it in the browser";
  };

  tasks."test:coverage" = {
    exec = "cargo llvm-cov --lib";
    description = "Generate test coverage report";
  };

  tasks."test:e2e" = {
    exec = "pytest test/e2e";
    description = "Run the E2E tests against a running server";
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
    exec = "prettier --write 'docs/**/*.md' README.md '**/TODO.md'";
    description = "Format markdown code";
  };

  tasks."fmt:sql" = {
    exec = "sqlfluff format --dialect sqlite migrations";
    description = "Format sql file";
  };

  # Linting

  tasks."lint:rust" = {
    exec = "cargo clippy --all-targets -- -D warnings";
    description = "Lint rust code";
  };

  tasks."lint:yaml" = {
    exec = "yamllint -c .yamllint .";
    description = "Lint yaml file";
  };

  tasks."lint:sql" = {
    exec = "sqlfluff lint --dialect sqlite migrations";
    description = "Lint sql file";
  };

  tasks."lint:python" = {
    exec = "ruff check .";
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

  # Scripts

  # Launch the Memorium server
  scripts = {
    server.exec = "cargo run --bin server";
  };

  processes = {
    docs.exec = "mkdocs serve --dev-addr 0.0.0.0:8000";
    openapi-spec.exec = "npx @redocly/cli preview -d docs/development/api --port 8001";
  };

}
