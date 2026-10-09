#!/usr/bin/env python3
"""Validate the `.opencode/config.json` the triage workflow loads.

The file declares `$schema` https://opencode.ai/config.json. Under the pinned
opencode v2 CLI servers live under `mcp.servers`; the v1 `mcp` map shape
registers no server, so the unattended sweep runs without its GitHub tools while
every other check passes. The guard also constrains the rest of the file: it
allow-lists the top-level keys and the server keys, so the config cannot override
the `.md` frontmatter that holds the `ci-triage` containment rules. Fail the run
instead of loading a config that silently disables the tools.

Lives in `script/` and is run by the CI `python` job. See
docs/development/TechnicalDesign.md § 6 (Dependencies & Dev Environment),
`DEPS-002`.
"""

import json
import sys
from pathlib import Path

CONFIG = Path(".opencode/config.json")
SCHEMA = "https://opencode.ai/config.json"
GITHUB_URL = "https://api.githubcopilot.com/mcp/"
TOKEN_PLACEHOLDER = "{env:GITHUB_MCP_TOKEN}"
TOP_LEVEL_KEYS = {"$schema", "mcp"}
MCP_KEYS = {"servers"}
SERVER_KEYS = {"type", "url", "oauth", "headers", "enabled"}


def fail(message: str) -> None:
    sys.exit(f"opencode config: {message}")


def main() -> None:
    if not CONFIG.is_file():
        fail("missing .opencode/config.json")
    config = json.loads(CONFIG.read_text(encoding="utf-8"))

    unknown_top = set(config) - TOP_LEVEL_KEYS
    if unknown_top:
        fail(f"unknown top-level keys: {sorted(unknown_top)}")
    if config.get("$schema") != SCHEMA:
        fail(f"$schema must be {SCHEMA}")

    mcp = config.get("mcp")
    if not isinstance(mcp, dict):
        fail("mcp must be an object")
    unknown_mcp = set(mcp) - MCP_KEYS
    if unknown_mcp:
        fail(f"unknown mcp keys: {sorted(unknown_mcp)} (v2 servers live under `servers`)")

    servers = mcp.get("servers")
    if not isinstance(servers, dict):
        fail("mcp.servers must be an object")
    unknown_servers = set(servers) - {"github"}
    if unknown_servers:
        fail(f"unknown mcp.servers keys: {sorted(unknown_servers)}")

    github = servers.get("github")
    if not isinstance(github, dict):
        fail("mcp.servers.github must be an object")
    unknown_server = set(github) - SERVER_KEYS
    if unknown_server:
        fail(f"unknown mcp.servers.github keys: {sorted(unknown_server)}")
    if github.get("disabled") is True or github.get("enabled") is False:
        fail("mcp.servers.github must be enabled")
    if github.get("type") != "remote":
        fail("mcp.servers.github.type must be remote")
    if github.get("url") != GITHUB_URL:
        fail(f"mcp.servers.github.url must be {GITHUB_URL}")
    if github.get("oauth") is not False:
        fail("mcp.servers.github.oauth must be false")
    headers = github.get("headers")
    if not isinstance(headers, dict):
        fail("mcp.servers.github.headers must be an object")
    authorization = headers.get("Authorization")
    if not isinstance(authorization, str) or TOKEN_PLACEHOLDER not in authorization:
        fail(f"mcp.servers.github.headers.Authorization must interpolate {TOKEN_PLACEHOLDER}")

    print("opencode config OK")


if __name__ == "__main__":
    main()
