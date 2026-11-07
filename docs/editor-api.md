# The editor API for agents

An agent running in SnoutCode's console can work the editor: open a file at a line or with
text selected, read your selection and your open tabs, read the problems of a file, check
whether a file has unsaved changes, and save it. It can also find out where it stands in the
window, and bring a window such as Git or Problems to the front for you. Ask Claude Code "open
the order service at the place method" and the file opens in SnoutCode, the method selected.

It works without an MCP server, so the editor's tools cost almost nothing in a session that
does not use them. The chain is **Agent Skill → UTCP → API**:

1. **An Agent Skill** tells the agent how to reach the editor. The console's `claude` loads it
   for its session only. Codex, opencode and Amp get one MCP tool instead (see [Other agents](#other-agents)).
2. **A UTCP manual** describes each call: its name, what it does, its inputs, and the HTTP
   request that makes it.
3. **A plain HTTP API** in SnoutCode answers the calls.

## Why not MCP: the tokens

An MCP server's tools are listed to the model with their full descriptions and JSON schemas,
so they take room in the context of every request of the session, whether the agent uses them
or not. A skill shows the model one line, and the rest is read only when the agent needs it.

For the API's twelve tools, measured on what SnoutCode sends:

| | Size | When it is in the context |
|---|---|---|
| The twelve tools as MCP definitions | about 4,900 characters, roughly 1,200 tokens | every request of the session |
| The skill's description | about 290 characters, roughly 70 tokens | every request of the session |
| The skill's instructions, with the `openFile` call | about 1,500 characters, roughly 370 tokens | once, when the agent first uses the editor |
| The short manual, all twelve tools | about 2,200 characters, roughly 550 tokens | only when the agent needs a tool other than `openFile` |
| One domain's manual, the editor's | about 3,400 characters, roughly 840 tokens | only when the agent needs that domain |
| The MCP tool, for agents without the skill | about 670 characters, roughly 170 tokens | every request of the session |

So a session that never touches the editor pays less than a tenth of what the MCP definitions
would cost, on every request. Opening a file, the common case, needs the skill alone; the
manual stays out of the context unless the agent wants something else. Each tool added to
the API later grows the manual, not every request.

The answers cost the same either way: a call returns the same text as the MCP tool of that
name would.

Claude Code still connects to SnoutCode as its IDE (see [Coding agents](agents.md#claude-code-and-the-editor)):
that connection is Claude Code's own plumbing (the changes it proposes for you to review, your
selection in its prompt), not tools for the model, and stays as it is.

## How Claude Code uses it

You do nothing: start Claude Code in the console and ask. Behind it, SnoutCode starts `claude`
with:

- `--plugin-dir`, a plugin of its own holding the **snoutcode** skill and a small script,
  `snout-api`. Nothing is installed into your Claude Code setup, and the plugin is gone when
  the window closes.
- `--settings`, the settings it already passes for its hooks, which also let `snout-api` run
  without asking for permission.
- Two variables: `SNOUT_HOOK_PORT`, the API's port, and `SNOUT_API_HEADER`, the file that
  holds the token's header line.

When you ask Claude to open or show a file, it reads the skill and runs, for example:

```sh
snout-api openFile '{"filePath":"/home/me/shop/src/main/java/shop/OrderService.java","startText":"public Order place("}'
```

The file opens in the editor with that text selected, and Claude hears back
`Opened file and selected text "public Order place("`.

## The API

SnoutCode listens on 127.0.0.1 only, on a port the system picks, and checks a token on every
request. The token is in a file only you can read, so it never shows on a command line or in
the agent's transcript.

| | |
|---|---|
| The short manual | `GET http://127.0.0.1:$SNOUT_HOOK_PORT/manual`, every tool without its arguments; `?domain=editor` for a domain's tools with theirs |
| The UTCP manual | `GET http://127.0.0.1:$SNOUT_HOOK_PORT/utcp` |
| A call | `POST http://127.0.0.1:$SNOUT_HOOK_PORT/api/<tool>` |
| Arguments | a JSON body, or query parameters (`?filePath=…&startLine=12`), or both (the body wins) |
| The token | the header line in the file `$SNOUT_API_HEADER`; with curl, `-H @"$SNOUT_API_HEADER"` |
| The answer | the tool's JSON, or its text; an error has a status (400, 404, 422) and `{"error": "…"}` |

The manual tags each tool with what it does: `read` looks, `show` changes only what is on screen, `change` writes or starts something.

The tools:

| Tool | What it does | Arguments |
|---|---|---|
| `getAppState` | Where the session stands: the project (none when it runs in the home folder), its branch, the open files, the runs, the problems, the windows showing and the connection | none |
| `showToolWindow` | Brings a window to the front: Project, Git, Run, Debug, Build, Output, Terminal, Problems, Console, Gradle, Maven, Spring, Tests, Agent Changes, Structure, Hierarchy, Notifications or Processes. It stays if it already shows | `name` |
| `openFile` | Opens a file, and selects lines or text in it | `filePath`; `startLine`, `endLine` (from 1); or `startText`, `endText`, `selectToEndOfLine`; `makeFrontmost`, `preview` |
| `getCurrentSelection` | The selection in the editor in front | none |
| `getLatestSelection` | The last selection made, in any tab | none |
| `getOpenEditors` | The open tabs: path, language, unsaved or not, selection | none |
| `getWorkspaceFolders` | The project's folders | none |
| `getDiagnostics` | The errors and warnings of a file, or of every open file | `uri` (optional) |
| `checkDocumentDirty` | Whether a file has unsaved changes | `filePath` |
| `saveDocument` | Saves a file | `filePath` |
| `getCommitChange` | While a [draft](git.md#draft-the-message-with-your-agent) waits: the change the commit would record, and how to write its message | none |
| `setCommitMessage` | While a draft waits: puts the message in the commit box. An error once it was stopped. With `snout-api`, the message may also come as plain text on standard input | `message` |

With curl, from the console:

```sh
curl -s -X POST -H @"$SNOUT_API_HEADER" -H 'content-type: application/json' \
  --data '{"filePath":"/home/me/shop/build.gradle.kts","startLine":12}' \
  "http://127.0.0.1:$SNOUT_HOOK_PORT/api/openFile"
```

## Other agents

Codex, opencode and Amp get the same tools through one MCP tool, `snoutcode`, for their session
only. Nothing is written into your project or your settings:

| Agent | How it gets the tool |
|---|---|
| Codex | `-c mcp_servers.snoutcode...` arguments. The token comes from the variable `SNOUT_API_TOKEN`. This server's calls do not ask for approval in Codex, because what changes something is asked in SnoutCode's window |
| opencode | The session's `OPENCODE_CONFIG_CONTENT` variable |
| Amp | `--mcp-config`, naming a file in the session's folder that only you can read |

Why one MCP tool, when the reason for the skill is to avoid MCP's cost? A sandbox can cut an agent's
commands off from the port (Codex's does, by default) but not its MCP client, so the script would
not work there. And the cost was the definitions: one tool costs about what a skill's description
does, however many tools the API has. The agent calls `snoutcode` with `tool: "manual"` to list the
tools, and with `arguments: {"domain": "editor"}` for the arguments of a domain's tools.

An agent of your own gets `SNOUT_HOOK_PORT`, `SNOUT_API_HEADER` and `SNOUT_API`, the script's path.
Tell it about them in its instructions: `"$SNOUT_API" manual` lists the tools. A UTCP client reads
`GET /utcp` and calls the tools from it directly; the manual names the port (`${SNOUT_HOOK_PORT}`)
and the token (`${SNOUT_API_TOKEN}`, the value in the header file) as variables, so nothing secret
is written into it. The script needs `sh` and `curl`; on Windows without Git for Windows, call the
API with `curl.exe`.

Outside SnoutCode's console there is no port or token, and the script says so: the API is for
the agent working in the window.

## On a remote machine

With a [remote project](remote-ssh.md), `claude` runs on the remote machine and so does the
plugin. The API's port there is forwarded back to the window over the same SSH link as the
hooks, so the same calls open files in the editor on your Mac.
