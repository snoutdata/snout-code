// SnoutCode's plugin for Amp: it tells the SnoutCode window whose console runs this Amp
// about each file edit, before and after, and the end of each turn with its prompt, so the
// editor shows the agent writing. SnoutCode puts it here when its console starts Amp. In
// an Amp started anywhere else (no SNOUT_HOOK_PORT) it does nothing at all.
import { copyFileSync, mkdirSync, readFileSync } from "node:fs"
import { dirname, join } from "node:path"

export const description = "SnoutCode: shows Amp's edits live in SnoutCode's editor when Amp runs in its console"

export default function (amp) {
  const port = process.env.SNOUT_HOOK_PORT
  const headerFile = process.env.SNOUT_API_HEADER
  if (!port || !headerFile) return
  // The skill that tells Amp how to work the window, from the session's folder: it names this
  // session's script. An Amp started anywhere else never hears of it.
  const script = process.env.SNOUT_API
  if (script) {
    try {
      const dir = join(amp.system.cacheDirectory, "skills", "snoutcode")
      mkdirSync(dir, { recursive: true })
      copyFileSync(join(dirname(dirname(script)), "skills", "snoutcode", "SKILL.md"), join(dir, "SKILL.md"))
      amp.registerSkill({ path: dir }).catch(() => {})
    } catch {
      // No skill: Amp goes on as it would without SnoutCode.
    }
  }
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
        body: JSON.stringify({ agent: "amp", ...body }),
        signal: AbortSignal.timeout(5000),
      })
    } catch {
      // The window is gone or busy: Amp goes on as it would without SnoutCode.
    }
  }
  // The files a tool call changes, as Amp reads it: its file tools, a patch, a shell
  // command's redirections.
  const files = (event) => {
    try {
      return (amp.helpers.filesModifiedByToolCall(event) ?? []).map((uri) => amp.helpers.filePathFromURI(uri))
    } catch {
      return []
    }
  }
  const report = async (kind, event) => {
    const changed = files(event)
    if (changed.length > 0)
      await post({ event: kind, session: event.thread?.id, tool: event.tool, input: event.input ?? {}, files: changed })
  }
  amp.on("tool.call", async (event) => {
    await report("pre", event)
    return { action: "allow" }
  })
  amp.on("tool.result", async (event) => {
    if (event.status === "done") await report("post", event)
  })
  amp.on("agent.end", async (event) => {
    await post({ event: "end", session: event.thread?.id, prompt: typeof event.message === "string" ? event.message : "" })
  })
}
