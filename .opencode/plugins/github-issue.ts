import { Plugin } from "@opencode/plugin"

import { existingLabels, gh, message } from "../lib/github-gh.ts"

// The namespace keeps the public tool names `github-issue_search` and
// `github-issue_create`, which the reviewer agent and the create-issue skill
// refer to by name.
const NAMESPACE = "github-issue"

export default Plugin.define({
  id: "github-issue",
  async setup(ctx) {
    await ctx.tool.transform((editor) => {
      editor.namespace({
        name: NAMESPACE,
        description: "List the open GitHub issues of the current repository, and create new ones.",
      })

      editor.add({
        name: "search",
        description:
          "List the open GitHub issues of the current repository, including their body, so the caller can discard " +
          "pre-existing findings that are already tracked.",
        input: {
          type: "object",
          properties: {},
          additionalProperties: false,
        },
        options: { namespace: NAMESPACE },
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
            return { content: out || "[]" }
          } catch (error) {
            return { content: `github-issue_search failed: ${message(error)}` }
          }
        },
      })

      editor.add({
        name: "create",
        description:
          "Create a GitHub issue in the current repository. Labels that do not exist are skipped. Returns the new " +
          "issue number and URL.",
        input: {
          type: "object",
          properties: {
            title: { type: "string", description: "Issue title" },
            body: { type: "string", description: "Issue body, including the AI-generated disclosure" },
            labels: {
              type: "array",
              items: { type: "string" },
              description: "Labels to apply; labels that do not exist are skipped",
            },
          },
          required: ["title", "body"],
          additionalProperties: false,
        },
        options: { namespace: NAMESPACE },
        async execute(input) {
          const { title, body, labels } = input as { title: string; body: string; labels?: string[] }

          try {
            const requested = labels ?? []
            const existing = requested.length > 0 ? await existingLabels() : new Set<string>()
            const applied = requested.filter((label) => existing.has(label))

            const command = ["issue", "create", "--title", title, "--body", body]
            for (const label of applied) {
              command.push("--label", label)
            }

            const url = await gh(command)
            return { content: url || "created" }
          } catch (error) {
            return { content: `github-issue_create failed: ${message(error)}` }
          }
        },
      })
    })
  },
})
