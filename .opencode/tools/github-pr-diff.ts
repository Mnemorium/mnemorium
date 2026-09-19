import { execFile } from "node:child_process"
import { promisify } from "node:util"

import { tool } from "@opencode-ai/plugin"

const run = promisify(execFile)

const REPO = process.env.GITHUB_REPOSITORY ?? ""
const TOKEN = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN ?? ""

function message(error: unknown): string {
  if (error instanceof Error) {
    const stderr = (error as { stderr?: string }).stderr
    return stderr?.trim() || error.message
  }
  return String(error)
}

async function gh(args: string[]): Promise<string> {
  if (!REPO) {
    throw new Error("GITHUB_REPOSITORY is not set")
  }
  if (!TOKEN) {
    throw new Error("GITHUB_TOKEN or GH_TOKEN is not set")
  }

  const { stdout } = await run("gh", [...args, "--repo", REPO], {
    env: { ...process.env, GH_TOKEN: TOKEN },
    maxBuffer: 10 * 1024 * 1024,
  })
  return stdout.trim()
}

export default tool({
  description: "Return the unified diff of a pull request, as computed by GitHub.",
  args: {
    number: tool.schema.number().int().describe("Pull request number"),
  },
  async execute(args) {
    try {
      const diff = await gh(["pr", "diff", String(args.number)])
      return diff || "no diff"
    } catch (error) {
      return `github-pr-diff failed: ${message(error)}`
    }
  },
})
