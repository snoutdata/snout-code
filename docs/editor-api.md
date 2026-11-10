# The API for agents

An agent running in SnoutCode's console can work the whole window, not only write files. It can open
a file at a line, find out what is open, switch to the project it just made, start a run and read
its output and test results, ask the Java and TypeScript engines what the code is, run a build task,
use the Git view, and show you a report with buttons on its code. Ask Claude Code "run the tests and
show me what failed" and it does it where you can see it.

It works without an MCP server, so these tools cost almost nothing in a session that does not use
them. The chain is **Agent Skill → UTCP → API**:

1. **An Agent Skill** tells the agent how to reach the window. The console's `claude` loads it
   for its session only. opencode and Amp get the same skill, and Codex one MCP tool instead (see [Other agents](#other-agents)).
2. **A UTCP manual** describes each call: its name, what it does, its inputs, and the HTTP
   request that makes it.
3. **A plain HTTP API** in SnoutCode answers the calls.

## Why not MCP: the tokens

An MCP server's tools are listed to the model with their full descriptions and JSON schemas,
so they take room in the context of every request of the session, whether the agent uses them
or not. A skill shows the model one line, and the rest is read only when the agent needs it.

For the API's 57 tools, measured on what SnoutCode sends:

| | Size | When it is in the context |
|---|---|---|
| The 57 tools as MCP definitions | about 23,000 characters, roughly 5,800 tokens | every request of the session |
| The skill's description | about 560 characters, roughly 140 tokens | every request of the session |
| The one MCP tool, for Codex in place of the skill | about 760 characters, roughly 190 tokens | every request of the session |
| The skill's instructions, with the `openFile` call | about 2,000 characters, roughly 500 tokens | once, when the agent first uses the window |
| The short manual, all 57 tools with no arguments | about 9,400 characters, roughly 2,300 tokens | only when the agent needs a tool other than `openFile` |
| One domain's manual, with its arguments | 800 to 4,400 characters, 200 to 1,100 tokens | only when the agent needs that domain |

So a session that never touches the window pays about a fortieth of what the MCP definitions would
cost, on every request. Opening a file, the common case, needs the skill alone; the manual stays out
of the context unless the agent wants something else. Each tool added to the API later grows the
manual, not every request.

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

The manual tags each tool with what it does: `read` looks, `show` changes only what is on screen,
`change` writes or starts something.

## What an agent needs your answer for

Looking and showing never ask: asking leave to look only teaches people to click through. A tool
tagged `change` waits for you, on a card in the window that says what the agent wants to do:

- **Allow** lets this one call go ahead.
- **Allow for This Session** also stops the asking for that tool until SnoutCode quits.
- **Don't Allow**, or closing the card, tells the agent you said no, and not to try again unless you ask.

Draft's `setCommitMessage` does not ask, because you just clicked Draft. An agent's own commands and
file edits are not these calls: they go by that agent's own permissions, as they would anywhere.

Settings are limited to appearance and editor ones. No tool can read or change what names a program,
a shell or a variable, since that would let an agent run something outside its sandbox the next time
SnoutCode started it.

**View ▸ Agent Log** lists every call an agent made in this session, with its arguments and how it
ended.

## What an agent can show you

`showInApp` puts markdown in the **Agent View** window, on the right, for what no tool covers: a plan,
a report, a comparison. It comes forward for the first push only; after that a new push leaves you
where you are. A fenced code block has **Copy** and **Insert**, which puts it in at the caret of the
editor in front. A `sh` or `bash` block also has **Run in Terminal**. Nothing runs until you click, and
the agent does not hear of what you click. **Clear** empties it, and it lasts until SnoutCode quits.

## The tools

| Tool | Domain | What it does | Arguments |
|---|---|---|---|
| `getAppState` | app | Where the session stands: the project (none when it runs in the home folder), its branch, the Git operation running and the last one that failed (with why Git could not sign in, when that is why), the open files, the runs, the problems, the windows showing and the connection | none |
| `showToolWindow` | app | Brings a window to the front: Project, Git, Run, Debug, Build, Output, Terminal, Problems, Console, Gradle, Maven, Spring, Tests, Agent Changes, Structure, Hierarchy, Notifications, Processes, Agent Log or Agent View | `name` |
| `showInApp` | app | Shows you markdown in the Agent View | `markdown`; `title`, `clear` |
| `showNotification` | app | Tells you something in the status bar and Notifications | `message` |
| `listProcesses` | app | The processes SnoutCode runs | none |
| `stopProcess` | app | Stops one of them | `pid` |
| `getSettings` | app | The appearance and editor settings an agent may change | none |
| `setSetting` | app | Changes one of them | `key`, `value` |
| `getShortcuts` | app | The commands, with their keys as bound now | none |
| `listProjects` | project | The project open, and those opened lately | none |
| `switchProject` | project | Opens a folder as the project | `path` |
| `closeProject` | project | Closes the project | none |
| `showNewProject` | project | Opens the New Project dialog | none |
| `listRunConfigurations` | run | What the project can run | none |
| `listRuns` | run | The runs, with their ids and how each went | none |
| `startRun` | run | Starts a configuration | `configuration`; `debug` |
| `stopRun` | run | Stops a run | `id` |
| `getRunOutput` | run | What a run printed | `id`, `lastLines` |
| `getTestResults` | run | How a run's tests went, with each failure | `id` |
| `runPlaywrightTests` | run | Runs Playwright tests | `file`, `line`, `project` |
| `getPlaywrightTrace` | run | What a test's trace saw: actions, failed requests, console errors | `test`, `id` |
| `recordPlaywrightTest` | run | Starts Playwright's recorder at the caret | none |
| `pickPlaywrightLocator` | run | Lets you pick an element for its locator | none |
| `findSymbol` | code | Classes, methods and fields by name | `query` |
| `getDefinition` | code | Where a symbol is defined | `filePath`, `line`; `text` or `column` |
| `findReferences` | code | Where a symbol is used | the same |
| `getHover` | code | A symbol's type and documentation | the same |
| `getHierarchy` | code | A type's supertypes or subtypes, a method's callers or callees | the same, and `direction` |
| `getProjectProblems` | code | The errors and warnings across the project | none |
| `renameSymbol` | code | Renames a symbol everywhere; the changes stay unsaved | the same, and `newName` |
| `listBuildTasks` | build | Gradle's tasks and Maven's goals | none |
| `runBuildTask` | build | Runs one in the Build window | `task`, or `goals` and `module`; `build` |
| `syncBuild` | build | Reads the build again | none |
| `openFile` | editor | Opens a file, and selects lines or text in it | `filePath`; `startLine`, `endLine` (from 1); or `startText`, `endText`, `selectToEndOfLine`; `makeFrontmost`, `preview` |
| `getCurrentSelection` | editor | The selection in the editor in front | none |
| `getLatestSelection` | editor | The last selection made, in any tab | none |
| `getOpenEditors` | editor | The open tabs: path, language, unsaved or not, selection | none |
| `getWorkspaceFolders` | editor | The project's folders | none |
| `getDiagnostics` | editor | The errors and warnings of a file, or of every open file | `uri` (optional) |
| `checkDocumentDirty` | editor | Whether a file has unsaved changes | `filePath` |
| `saveDocument` | editor | Saves a file | `filePath` |
| `showChanges` | git | Shows the Git view | none |
| `showDiff` | git | Shows a changed file's diff | `path` |
| `showCommit` | git | Shows a commit | `hash` |
| `showBranches` | git | Opens the Check Out dialog | none |
| `stageFiles` | git | Stages files | `paths` |
| `unstageFiles` | git | Unstages files | `paths` |
| `commitChanges` | git | Commits what is staged | `message` |
| `pushCommits` | git | Pushes the current branch | none |
| `fetchRemotes` | git | Fetches from the remotes | none |
| `getCommitChange` | git | While a [draft](git.md#draft-the-message-with-your-agent) waits: the change the commit would record, and how to write its message | none |
| `setCommitMessage` | git | While a draft waits: puts the message in the commit box. An error once it was stopped. With `snout-api`, the message may also come as plain text on standard input | `message` |
| `listAgentChanges` | agents | The files agents changed that await review | none |
| `acceptAgentChange` | agents | Keeps an agent's changes to a file | `filePath` |
| `revertAgentChange` | agents | Puts a file back as it was before them | `filePath` |
| `listBeans` | spring | The Spring beans | `filter` |
| `listEndpoints` | spring | The HTTP endpoints | `filter` |

TypeScript's engine answers only about files open in a tab, and Java's knows a type's hierarchy once
its file is open: the answer says so, and the agent opens it with `openFile` and `makeFrontmost: false`.

With curl, from the console:

```sh
curl -s -X POST -H @"$SNOUT_API_HEADER" -H 'content-type: application/json' \
  --data '{"filePath":"/home/me/shop/build.gradle.kts","startLine":12}' \
  "http://127.0.0.1:$SNOUT_HOOK_PORT/api/openFile"
```

## Other agents

Every agent gets the same tools, for its session only. Nothing is written into your project or your
settings:

| Agent | How it hears of them |
|---|---|
| Claude Code | The skill, through `--plugin-dir` |
| opencode | One line in its instructions, through the session's `OPENCODE_CONFIG_CONTENT` variable, that sends it to the skill |
| Amp | The skill, registered by SnoutCode's Amp plugin, which does nothing outside the console |
| Codex | One MCP tool, `snoutcode`, through `-c mcp_servers.snoutcode...` arguments |

Codex is the exception because its sandbox cuts its commands off from the port by default, so the
script cannot reach the window there, while its MCP client can. Opening the sandbox's network for
the script would open it to everything else too. One MCP definition costs about what a skill's
description does, however many tools the API has. Codex calls `snoutcode` with `tool: "manual"` to
list the tools, and with `arguments: {"domain": "editor"}` for a domain's arguments. Its token comes
from the variable `SNOUT_API_TOKEN`. These calls do not ask for approval in Codex, because what
changes something is asked in SnoutCode's window.

An agent of your own gets `SNOUT_HOOK_PORT`, `SNOUT_API_HEADER` and `SNOUT_API`, the script's path.
Tell it about them in its instructions: `"$SNOUT_API" manual` lists the tools. A UTCP client reads
`GET /utcp` and calls the tools from it directly; the manual names the port (`${SNOUT_HOOK_PORT}`)
and the token (`${SNOUT_API_TOKEN}`, the value in the header file) as variables, so nothing secret
is written into it. The script needs `sh` and `curl`; on Windows without Git for Windows, call the
API with `curl.exe`.

Outside SnoutCode's console there is no port or token, and the script says so: the API is for
the agent working in the window.

## On a remote machine

With a [remote project](remote-ssh.md) or a [project in WSL](windows.md#projects-in-wsl), the agent
runs on the Linux side, so everything it needs is put there: the skill and `snout-api`, opencode's
line, Amp's plugin. The API's port on that machine is forwarded back to the window over the same
link as the hooks, so every agent's calls work the same, and the files it names are that machine's.
