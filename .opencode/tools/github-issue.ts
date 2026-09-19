import { execFile } from "node:child_process"
import { promisify } from "node:util"

import { tool } from "@opencode-ai/plugin"

const run = promisify(execFile)

const REPO = process.env.GITHUB_REPOSITORY ?? ""
const TOKEN = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN ?? ""

let labelCache: Set<string> | undefined

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

async function existingLabels(): Promise<Set<string>> {
  if (labelCache) {
    return labelCache
  }
  const out = await gh(["label", "list", "--limit", "200", "--json", "name"])
  const parsed = JSON.parse(out || "[]") as { name: string }[]
  labelCache = new Set(parsed.map((label) => label.name))
  return labelCache
}

export const search = tool({
  description:
    "List the open GitHub issues of the current repository, including their body, so the caller can discard " +
    "pre-existing findings that are already tracked.",
  args: {},
  async execute() {
    try {
      const out = await gh([
        "issue",
        "list",
        "--state",
        "open",
        "--limit",
        "100",
        "--json",
        "number,title,body,url",
      ])
      return out || "[]"
    } catch (error) {
      return `github-issue_search failed: ${message(error)}`
    }
  },
})

export const create = tool({
  description:
    "Create a GitHub issue in the current repository. Labels that do not exist are skipped. Returns the new " +
    "issue number and URL.",
  args: {
    title: tool.schema.string().describe("Issue title"),
    body: tool.schema.string().describe("Issue body, including the AI-generated disclosure"),
    labels: tool.schema
      .array(tool.schema.string())
      .optional()
      .describe("Labels to apply; labels that do not exist are skipped"),
  },
  async execute(args) {
    try {
      const requested = args.labels ?? []
      const existing = requested.length > 0 ? await existingLabels() : new Set<string>()
      const labels = requested.filter((label) => existing.has(label))

      const command = ["issue", "create", "--title", args.title, "--body", args.body]
      for (const label of labels) {
        command.push("--label", label)
      }

      const url = await gh(command)
      return url || "created"
    } catch (error) {
      return `github-issue_create failed: ${message(error)}`
    }
  },
})
