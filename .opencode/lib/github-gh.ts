import { execFile } from "node:child_process"
import { promisify } from "node:util"

const run = promisify(execFile)

let labelCache: Set<string> | undefined

/**
 * Reduce a failed `gh` invocation to a single message. `execFile` attaches the
 * captured stderr to the error, which is where `gh` writes its diagnostics.
 */
export function message(error: unknown): string {
  if (error instanceof Error) {
    const stderr = (error as { stderr?: string }).stderr
    return stderr?.trim() || error.message
  }
  return String(error)
}

/**
 * Run `gh` against the repository the review is running for.
 *
 * The repository and token are read per call rather than at module load: a
 * plugin can reload during a run, and the values live in the server process
 * environment. `GH_TOKEN` is set explicitly so `gh` authenticates without a
 * prior `gh auth login`.
 */
export async function gh(args: string[]): Promise<string> {
  const repo = process.env.GITHUB_REPOSITORY ?? ""
  const token = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN ?? ""

  if (!repo) {
    throw new Error("GITHUB_REPOSITORY is not set")
  }
  if (!token) {
    throw new Error("GITHUB_TOKEN or GH_TOKEN is not set")
  }

  const { stdout } = await run("gh", [...args, "--repo", repo], {
    env: { ...process.env, GH_TOKEN: token },
    maxBuffer: 10 * 1024 * 1024,
  })
  return stdout.trim()
}

async function findExistingLabels(): Promise<Set<string>> {
  const out = await gh(["label", "list", "--limit", "200", "--json", "name"])
  const parsed = JSON.parse(out || "[]") as { name: string }[]
  labelCache = new Set(parsed.map((label) => label.name))
  return labelCache
}

/**
 * Labels that exist in the repository, cached for the lifetime of the plugin.
 */
export async function existingLabels(): Promise<Set<string>> {
  return labelCache ?? findExistingLabels()
}
