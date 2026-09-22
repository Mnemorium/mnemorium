# OpenCode

OpenCode configuration lives under `.opencode/` and is versioned with the repository: agents, skills, and plugins.

## Plugin dependencies

Plugins may import the OpenCode plugin SDK, which must be installed before the server can load them. Dependencies are
pinned in `.opencode/package.json` and installed with:

```sh
devenv tasks run plugins:install
```

Re-run this after changing `.opencode/package.json`, or after checking out a branch that changes it. The server watches
local plugin sources under `.opencode/plugins/`, but not `node_modules`, so restart the service after installing or
updating dependencies:

```sh
opencode service restart
```

## Markdown linting

The `markdownlint` tool lints Markdown with `markdownlint-cli2` using `.markdownlint-cli2.jsonc`. With no arguments it
lints every `**/*.md` file, matching the `lint:md` devenv task; pass `paths` to lint specific files.
