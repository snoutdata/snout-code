---
name: snoutcode
description: Work the SnoutCode window this session runs in, to open files in its editor (at a line, or with text selected), see the project, its tabs, selection and problems, and bring its windows forward. Use it whenever the user asks to open, show or go to a file or line, instead of `code`, `open`, `idea` or any other editor.
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

## Everything else

Everything else this can do is in the manual. List it when you need something:

```sh
{{API}} manual
```

Then read the arguments of one domain's tools, such as the editor's, with `{{API}} manual editor`.
Call each tool as `{{API}} <name> '<JSON arguments>'`. A tool tagged `change` writes or starts
something, and the user may be asked first.
