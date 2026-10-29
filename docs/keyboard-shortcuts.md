# Keyboard shortcuts

Every shortcut SnoutCode has out of the box, by area. You can change any of them in Settings, under Keymap (see [Settings and keymap](settings-and-keymap.md)).

Some keys mean different things in different places. Where a shortcut only works in one place, the **Where** column says so: "Editor" means a code editor has the focus, and "Outside text fields" means the focus is not in a text field, which takes the key first.

## Application and windows

| Shortcut | Command | Where |
|---|---|---|
| ⌘, | Settings… | |
| ⌘Q | Quit SnoutCode | |
| ⌘1 | Show Project | |
| ⌘2 | Show Git | |
| ⌘J | Show or hide the Claude Code console | |
| ⌘R or F5 | Refresh | |
| ⌘= or ⌘+ | Zoom In | |
| ⌘- | Zoom Out | |
| ⌘0 | Actual Size | |

## Projects and search

| Shortcut | Command | Where |
|---|---|---|
| ⌘O | Open Project… | |
| ⇧⌘P | Command Palette… | |
| ⇧⌘A | Search Everywhere… | |
| ⌘P | Go to File… | |
| ⇧⌘F | Find in Files… | |
| ⌘L | Go to Line… | |
| ⌘T | Go to Symbol in Workspace… | |
| ⇧⌘O | Go to Symbol in File… | Editor |
| ↑ ↓ | Pick the previous or next result | In a search box |
| Esc | Close the search box | In a search box |

## Editing

| Shortcut | Command | Where |
|---|---|---|
| ⌘S | Save | Editor |
| ⌘W | Close Tab | |
| ⌘\ | Split Editor Right | |
| ⌘Z | Undo | Editor |
| ⇧⌘Z | Redo | Editor |
| ⌘F | Find in File | Editor |
| ⌥⌘F | Replace in File | Editor |
| ⌘/ | Toggle Comment | Editor |
| ⌘D | Duplicate Line | Editor |
| ⌥↑ | Move Line Up | Editor |
| ⌥↓ | Move Line Down | Editor |
| ⌘] | Indent | Editor |
| ⌘[ | Outdent | Editor |
| ⌥⌘↑ | Add a cursor on the line above | Editor |
| ⌥⌘↓ | Add a cursor on the line below | Editor |
| ⌥Z | Toggle Soft Wrap | Editor |

## Java and code

| Shortcut | Command | Where |
|---|---|---|
| ⌃Space | Trigger Completion | Editor |
| ⇧⌘Space | Show Parameter Hints | Editor |
| F12 | Go to Definition | Editor |
| ⇧F12 | Find References | Editor |
| F2 | Rename Symbol… | Editor |
| Esc | Cancel the rename | In the rename box |
| ⌘. | Quick Fix… | Editor |
| ⌥⇧F | Format Document | Editor |
| ⌥⇧O | Organize Imports | Editor |
| ⇧⌘M | Show Problems | |

See [Java](java.md) for what each one does.

## Run and debug

| Shortcut | Command | Where |
|---|---|---|
| ⌃R | Run (the configuration chosen in the title bar) | |
| ⌃D | Debug (the configuration chosen in the title bar) | |
| ⌃⇧R | Run Context Configuration (what the caret is in) | |
| ⌃⇧D | Debug Context Configuration (what the caret is in) | |
| ⌘F2 | Stop | |
| F8 | Step Over | |
| F7 | Step Into | |
| ⇧F8 | Step Out | |
| F9 | Resume Program | |

See [Run and debug](run-and-debug.md).

## Git

| Shortcut | Command | Where |
|---|---|---|
| ⇧⌘O | Check Out… | Outside an editor |
| ⌘A | Select all files | Outside text fields |
| ⌘⌫ | Move the selected files to the Trash, after asking | Outside text fields |
| Esc | Leave the commit view | Outside text fields |
| ↑ ↓ | Previous or next commit, while the Journal shows | Outside text fields |
| Esc | Close the popup | In the branch popup |

See [Git](git.md).

## Claude Code

| Shortcut | Command | Where |
|---|---|---|
| ⌘J | Show or hide the Claude Code console | |
| ⌥⌘K | Send Selection to Claude | Editor |
| ⌘↩ | Accept Claude's Change | A change Claude Code proposes |
| Esc | Reject Claude's Change | A change Claude Code proposes |
| ⌃⌥⇧↓ | Next Claude Change | Editor |
| ⌃⌥⇧↑ | Previous Claude Change | Editor |
| ⌥⌘Y | Accept Claude Change at Caret | Editor |
| ⌥⌘Z | Revert Claude Change at Caret | Editor |
| ⌥⇧⌘Y | Accept All Claude Changes | |
| ⌥⇧⌘Z | Revert All Claude Changes | |

See [Claude Code](claude-code.md).

## Dialogs

| Shortcut | Command | Where |
|---|---|---|
| Esc | Close (Cancel) | Every dialog |
| ⏎ | The main button: Create, Clone, Check Out, OK | Dialogs whose main button keeps your work |
| ⌘⌫ | Discard, Delete, Delete branch | Dialogs that discard or delete, where ⏎ does nothing |

The buttons show their keys: **Cancel** has Esc, the main button ⏎, a button that discards or deletes ⌘⌫.
