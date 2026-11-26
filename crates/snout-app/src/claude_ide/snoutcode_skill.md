---
name: snoutcode
description: Work the SnoutCode window this session runs in. Open files in its editor at a line or with text selected, see the project and what is on screen, open or switch projects, run tests and specs (Playwright, Cypress, Vitest, Jest) in its Tests window, start runs and read their output and test results, ask the language engines for definitions, references, types and callers, run build tasks, use the Git view, and show the user a report in the app. Use it whenever the user asks to open, show or go to a file or line, instead of `code`, `open`, `idea` or any other editor, and whenever they ask about or for anything in SnoutCode.
---

# The SnoutCode window

This session runs in SnoutCode's console. Call the window with the script
`{{API}}`, exactly as shown (the full path, no variables): it is allowed to run without
asking. If it says it is not in SnoutCode's console, tell the user, and do not open
another editor instead.

## Open a file

Paths are absolute. Lines count from 1.

```sh
{{API}} openFile '{"filePath":"/abs/path/File.java","startLine":42}'
```

Arguments: `filePath` (required); `startLine` and `endLine` select those lines;
`startText` (and `endText`) select from the first match of that text (to the end of the
match of `endText`); `makeFrontmost: false` opens it without switching tabs. The answer
says what was opened and selected; an error names the problem (a file that is not there).

## Run tests

Run a project's tests here, not with `npx playwright`, `npx cypress`, `vitest` or `jest` in a shell:
then the run shows in the window's Tests window, where the user follows it.

```sh
{{API}} runTests '{"file":"/abs/path/cypress/e2e/login.cy.ts"}'
```

Leave out `file` to run them all; add `line` for the test at that line. It answers with the run's id
at once. Call `{{API}} getTestResults '{"id":<id>}'` once `listRuns` shows the run has ended.

## Everything else

Everything else this can do is in the manual. List it when you need something:

```sh
{{API}} manual
```

Then read the arguments of one domain's tools with `{{API}} manual <domain>`: app, project, run, code,
build, editor, git, agents or spring. Call each tool as `{{API}} <name> '<JSON arguments>'`.

A tool tagged `change` writes or starts something: the call waits while the user answers a card in
the window, and may come back refused. Do not try it again unless they ask. When you need to know
where you stand (no project may be open, and you may be in the home folder), call `getAppState` first.
It also names every Git repository the Git view lists, with its path, and the one the Git view
shows: when a Git error names a repository, find it there rather than searching the disk.
