// SnoutCode's plugin for opencode: it tells the SnoutCode window whose console runs this
// opencode about each file edit, before and after, and the prompt of each turn, so the
// editor shows the agent writing. SnoutCode loads it for that session only (through
// OPENCODE_CONFIG_CONTENT); outside SnoutCode's console it does nothing.
import { readFileSync } from "node:fs"

const TOOLS = new Set(["edit", "write", "multiedit", "patch", "apply_patch"])

export const SnoutCode = async ({ directory }) => {
  const port = process.env.SNOUT_HOOK_PORT
  const headerFile = process.env.SNOUT_API_HEADER
  if (!port || !headerFile) return {}
  let header
  const post = async (body) => {
    try {
      if (header === undefined) {
        const line = readFileSync(headerFile, "utf8").trim()
        const at = line.indexOf(":")
        header = at > 0 ? [line.slice(0, at).trim(), line.slice(at + 1).trim()] : null
      }
      if (!header) return
      await fetch(`http://127.0.0.1:${port}/agent`, {
        method: "POST",
        headers: { "content-type": "application/json", [header[0]]: header[1] },
        body: JSON.stringify({ agent: "opencode", cwd: directory, ...body }),
        signal: AbortSignal.timeout(5000),
      })
    } catch {
      // The window is gone or busy: the agent goes on as it would without SnoutCode.
    }
  }
  return {
    "chat.message": async (input, output) => {
      const text = (output?.parts ?? [])
        .filter((p) => p?.type === "text" && !p.synthetic)
        .map((p) => p.text)
        .join("\n")
      if (text) await post({ event: "prompt", session: input?.sessionID, prompt: text })
    },
    "tool.execute.before": async (input, output) => {
      if (TOOLS.has(input?.tool))
        await post({ event: "pre", session: input.sessionID, tool: input.tool, input: output?.args ?? {} })
    },
    "tool.execute.after": async (input) => {
      if (TOOLS.has(input?.tool))
        await post({ event: "post", session: input.sessionID, tool: input.tool, input: input.args ?? {} })
    },
    event: async ({ event }) => {
      if (event?.type === "session.idle") await post({ event: "stop", session: event.properties?.sessionID })
    },
  }
}
