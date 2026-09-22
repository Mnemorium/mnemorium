import { execFile } from "node:child_process"
import { existsSync } from "node:fs"
import { join } from "node:path"
import { promisify } from "node:util"

import { Plugin } from "@opencode/plugin"

const run = promisify(execFile)

const ALL_MARKDOWN = ["**/*.md"]

/**
 * Run devenv against the project dir, not whatever devenv the server inherited.
 * A stale DEVENV_ROOT (for example, another worktree) would otherwise make
 * `devenv shell` resolve the wrong root.
 */
function cleanEnv(): NodeJS.ProcessEnv {
  const env = { ...process.env }
  delete env.DEVENV_ROOT
  delete env.DEVENV_DOTFILE
  return env
}

function failure(error: unknown): string {
  if (error instanceof Error) {
    const e = error as { stdout?: string; stderr?: string; code?: number | string }
    const body = `${e.stderr ?? ""}${e.stdout ?? ""}`.trim()
    if (body) return body
    return e.code !== undefined ? `${error.message} (exit ${e.code})` : error.message
  }
  return String(error)
}

export default Plugin.define({
  id: "markdownlint",
  async setup(ctx) {
    const projectDir = ctx.location.directory

    await ctx.tool.transform((editor) => {
      editor.add({
        name: "markdownlint",
        description:
          "Lint Markdown with markdownlint-cli2 using the repository's .markdownlint-cli2.jsonc. " +
          "Omit `paths` to lint every Markdown file (**/*.md), mirroring the `lint:md` devenv task. " +
          "Runs through `devenv shell` when the project is a devenv environment.",
        input: {
          type: "object",
          properties: {
            paths: {
              type: "array",
              items: { type: "string" },
              description: "Files or globs to lint. Omit or leave empty to lint all Markdown.",
            },
            fix: {
              type: "boolean",
              description: "Apply markdownlint auto-fixes (--fix) instead of only reporting.",
            },
          },
          additionalProperties: false,
        },
        async execute(input, context) {
          const { paths, fix } = input as { paths?: string[]; fix?: boolean }
          const targets = paths && paths.length > 0 ? paths : ALL_MARKDOWN
          const linter = ["markdownlint-cli2", ...(fix ? ["--fix"] : []), ...targets]

          const inThisDevenv = process.env.DEVENV_ROOT === projectDir
          const useDevenv = !inThisDevenv && existsSync(join(projectDir, "devenv.nix"))
          const command = useDevenv ? "devenv" : linter[0]
          // --quiet keeps devenv's own task logs off stderr so only lint output remains.
          const args = useDevenv ? ["--quiet", "shell", "--", ...linter] : linter.slice(1)

          try {
            const { stdout, stderr } = await run(command, args, {
              cwd: projectDir,
              env: useDevenv ? cleanEnv() : process.env,
              signal: context.signal,
              maxBuffer: 10 * 1024 * 1024,
            })
            // Findings go to stderr, the summary to stdout; lead with the findings.
            return { content: `${stderr}${stdout}`.trim() || "markdownlint: no problems found" }
          } catch (error) {
            return { content: `markdownlint:\n${failure(error)}` }
          }
        },
      })
    })
  },
})
