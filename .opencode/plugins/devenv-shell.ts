// Runs shell tool commands inside this repository's devenv environment.
//
// The environment is captured once per devenv root by activating the shell and
// reading `env -0`, then merged into every shell tool call that runs inside a
// discovered root. Commands therefore see the dev toolchain without paying a
// `devenv shell` activation, and its stderr chatter, on every call.
//
// Root discovery walks up from the command's working directory, so worktrees,
// nested devenv projects and sessions that moved elsewhere each resolve to their
// own snapshot.

import { execFile } from "node:child_process"
import { existsSync, statSync } from "node:fs"
import { dirname, isAbsolute, join, resolve } from "node:path"
import { Plugin } from "@opencode/plugin"

// A directory is a devenv root when it carries one of these markers. The local
// overrides are gitignored, so they belong to the staleness stamp as well.
const ROOT_MARKERS = ["devenv.nix", "devenv.local.nix", "devenv.yaml", "devenv.local.yaml"]
const STAMP_FILES = [...ROOT_MARKERS, "devenv.lock"]

// Volatile or OpenCode-owned variables the snapshot must not replace: PWD and
// friends describe the activation process, TERM and OPENCODE_TERMINAL belong to
// the shell tool, and OPENCODE_* values identify the session, not the snapshot.
const SKIP = new Set([
  "PWD",
  "OLDPWD",
  "SHLVL",
  "_",
  "TERM",
  "DEVENV_CMDLINE",
  "OPENCODE_TERMINAL",
  "OPENCODE_SESSION_ID",
  "OPENCODE_CLIENT",
])

type Entry = {
  env: Record<string, string>
  stamp: string
  pending?: Promise<Record<string, string>>
}

const cache = new Map<string, Entry>()
const warned = new Set<string>()

function findRoot(from: string) {
  let dir = resolve(from)
  for (;;) {
    if (ROOT_MARKERS.some((file) => existsSync(join(dir, file)))) return dir
    const parent = dirname(dir)
    if (parent === dir) return undefined
    dir = parent
  }
}

function stamp(root: string) {
  return STAMP_FILES.map((file) => {
    try {
      return String(statSync(join(root, file)).mtimeMs)
    } catch {
      return "0"
    }
  }).join(":")
}

// `env -0` separates records with NUL instead of a newline, so values may
// contain spaces or line breaks without corrupting the parse.
function activate(root: string) {
  return new Promise<Record<string, string>>((accept, reject) => {
    execFile(
      "devenv",
      ["shell", "--", "env", "-0"],
      { cwd: root, timeout: 120_000, maxBuffer: 8 * 1024 * 1024 },
      (error, stdout, stderr) => {
        if (error) {
          reject(new Error(stderr.trim() || error.message))
          return
        }
        const env: Record<string, string> = {}
        for (const record of stdout.split("\0")) {
          const separator = record.indexOf("=")
          if (separator <= 0) continue
          const key = record.slice(0, separator)
          if (!SKIP.has(key)) env[key] = record.slice(separator + 1)
        }
        accept(env)
      },
    )
  })
}

function envFor(root: string) {
  const current = stamp(root)
  const hit = cache.get(root)
  if (hit?.pending) return hit.pending
  if (hit && hit.stamp === current) return Promise.resolve(hit.env)

  const entry: Entry = { env: hit?.env ?? {}, stamp: current }
  entry.pending = activate(root).then(
    (env) => {
      entry.env = env
      entry.pending = undefined
      return env
    },
    (error) => {
      entry.pending = undefined
      throw error
    },
  )
  cache.set(root, entry)
  return entry.pending
}

export default Plugin.define({
  id: "mnemorium.devenv-shell",
  async setup(ctx) {
    // Warm the cache without delaying plugin activation, so the first command in
    // a fresh worktree does not pay the cold activation.
    const warm = findRoot(ctx.location.directory)
    if (warm) void envFor(warm).catch(() => {})

    await ctx.shell.hook("create.before", async (event) => {
      const cwd = isAbsolute(event.cwd) ? event.cwd : resolve(ctx.location.directory, event.cwd)
      const root = findRoot(cwd)
      if (!root) return
      if (event.env.DEVENV_ROOT === root) return

      try {
        Object.assign(event.env, await envFor(root))
      } catch (error) {
        if (warned.has(root)) return
        warned.add(root)
        console.error(`[devenv-shell] could not activate ${root}: ${String(error)}`)
      }
    })

    return () => {
      cache.clear()
      warned.clear()
    }
  },
})
