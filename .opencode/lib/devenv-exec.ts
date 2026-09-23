import { execFile } from "node:child_process"
import { existsSync } from "node:fs"
import { join } from "node:path"
import { promisify } from "node:util"

const run = promisify(execFile)

export interface RunInput {
  projectDir: string
  /** Command and arguments, for example ["prettier", "--check", "docs"]. Must be non-empty. */
  argv: [string, ...string[]]
  signal?: AbortSignal
}

export interface RunResult {
  ok: boolean
  content: string
}

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

/**
 * Execute a tool, entering the project's devenv shell when needed.
 *
 * Output is findings-first: tools such as markdownlint write findings to stderr
 * and their summary to stdout, so stderr leads. On a non-zero exit the captured
 * output is returned with `ok: false` instead of throwing, because linters and
 * `prettier --check` use a non-zero exit to report findings.
 */
export async function runTool(input: RunInput): Promise<RunResult> {
  const { projectDir, argv, signal } = input
  const [command, ...rest] = argv

  const inThisDevenv = process.env.DEVENV_ROOT === projectDir
  const useDevenv = !inThisDevenv && existsSync(join(projectDir, "devenv.nix"))

  const file = useDevenv ? "devenv" : command
  // --quiet keeps devenv's own task logs off stderr so only tool output remains.
  const args = useDevenv ? ["--quiet", "shell", "--", ...argv] : rest

  try {
    const { stdout, stderr } = await run(file, args, {
      cwd: projectDir,
      env: useDevenv ? cleanEnv() : process.env,
      signal,
      maxBuffer: 10 * 1024 * 1024,
    })
    return { ok: true, content: `${stderr}${stdout}`.trim() }
  } catch (error) {
    return { ok: false, content: failure(error) }
  }
}
