---
name: snout-editor
description: Open files in the SnoutCode editor this session runs in (at a line, or with text selected), and read its open tabs, selection, problems and unsaved files. Use it whenever the user asks to open, show or go to a file or line, instead of `code`, `open`, `idea` or any other editor.
---

# The SnoutCode editor

This session runs in SnoutCode's console. Call its editor with the script
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

The other tools (the open tabs, the selection, the problems of a file, whether a file has
unsaved changes, save it) are described by the UTCP manual. Read it when you need one:

```sh
{{API}} manual
```

Call each one as `{{API}} <name> '<JSON arguments>'`.
