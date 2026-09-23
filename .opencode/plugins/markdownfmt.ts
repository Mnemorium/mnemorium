import { Plugin } from "@opencode/plugin"

import { runTool } from "../lib/devenv-exec.ts"

const ALL_MARKDOWN = ["**/*.md"]

export default Plugin.define({
  id: "markdownfmt",
  async setup(ctx) {
    const projectDir = ctx.location.directory

    await ctx.tool.transform((editor) => {
      editor.add({
        name: "markdownfmt",
        description:
          "Format Markdown with Prettier using the repository's .prettierrc. " +
          "Omit `paths` to cover every Markdown file (**/*.md); set `write` to apply changes, " +
          "otherwise Prettier only checks. " +
          "Runs through `devenv shell` when the project is a devenv environment.",
        input: {
          type: "object",
          properties: {
            paths: {
              type: "array",
              items: { type: "string" },
              description: "Files or globs to format. Omit or leave empty to cover all Markdown.",
            },
            write: {
              type: "boolean",
              description: "Apply formatting (prettier --write). Defaults to check-only (prettier --check).",
            },
          },
          additionalProperties: false,
        },
        async execute(input, context) {
          const { paths, write } = input as { paths?: string[]; write?: boolean }
          const targets = paths && paths.length > 0 ? paths : ALL_MARKDOWN
          const argv: [string, ...string[]] = ["prettier", write ? "--write" : "--check", ...targets]

          const result = await runTool({ projectDir, argv, signal: context.signal })
          if (!result.ok) return { content: `markdownfmt:\n${result.content}` }
          return { content: result.content || "markdownfmt: no changes" }
        },
      })
    })
  },
})
