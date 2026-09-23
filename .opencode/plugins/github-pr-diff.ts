import { Plugin } from "@opencode/plugin"

import { gh, message } from "../lib/github-gh.ts"

export default Plugin.define({
  id: "github-pr-diff",
  async setup(ctx) {
    await ctx.tool.transform((editor) => {
      editor.add({
        name: "github-pr-diff",
        description: "Return the unified diff of a pull request, as computed by GitHub.",
        input: {
          type: "object",
          properties: {
            number: { type: "integer", description: "Pull request number" },
          },
          required: ["number"],
          additionalProperties: false,
        },
        async execute(input) {
          const { number } = input as { number: number }

          try {
            const diff = await gh(["pr", "diff", String(number)])
            return { content: diff || "no diff" }
          } catch (error) {
            return { content: `github-pr-diff failed: ${message(error)}` }
          }
        },
      })
    })
  },
})
