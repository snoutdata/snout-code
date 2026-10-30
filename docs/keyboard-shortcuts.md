# Keyboard shortcuts

Every shortcut SnoutCode has out of the box, by area. You can change any of them in Settings, under Keymap (see [Settings and keymap](settings-and-keymap.md)).

Some keys mean different things in different places. Where a shortcut only works in one place, the **Where** column says so: "Editor" means a code editor has the focus, and "Outside text fields" means the focus is not in a text field, which takes the key first.

## Application and windows

| Shortcut | Command | Where |
|---|---|---|
| ⌘, | Settings… | |
| ⌘; | Project Structure… (see [Project Structure](project-structure.md)) | |
| ⌘Q | Quit SnoutCode (asks first, see [Quitting](getting-started.md#quitting)) | |
| ⌘1 | Show Project | |
| ⌘2 | Show Git | |
| ⌘7 | Show or hide the Structure window | |
| ⌘J | Show or hide the agent console | |
| F5 | Refresh (⌘R is Rerun) | |
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
| ⌘E | Recent Files… | |
| ⌘N | New File… | |
| ⌥⌘O | Go to Class… (⌘O is Open Project) | |
| ⇧⌘F | Find in Files… | |
| ⇧⌘R | Replace in Files… | |
| ⌘↩ | Replace All | In Replace in Files |
| ⌘L | Go to Line… | |
| ⌘T | Go to Symbol in Workspace… | |
| ⇧⌘O | Go to Symbol in File… | Editor |
| ⌘F12 | File Structure… | Editor |
| ⌃H | Type Hierarchy | Editor |
| ⌃⌥H | Call Hierarchy | Editor |
| ↑ ↓ | Pick the previous or next result | In a search box, Find in Files |
| Esc | Close the search box | In a search box, Find in Files |

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
| ⌘B | Go to Declaration or Usages (on a declaration: its usages) | Editor |
| ⌥⌘B | Go to Implementations | Editor |
| ⌘-click | Go to the declaration (on a declaration: its usages) | Editor |
| ⌥⌘-click | Go to the implementations | Editor |
| ⇧F12 | Find References | Editor |
| F2 | Rename Symbol… | Editor |
| Esc | Cancel the rename | In the rename box |
| ⌥⌘M | Extract Method | Editor |
| ⌥⌘V | Extract Variable | Editor |
| ⌥⌘C | Extract Constant | Editor |
| ⌥⇧⌘F | Extract Field (⌥⌘F is Replace in File) | Editor |
| ⌥⌘N | Inline | Editor |
| ⌥↩, ⌘. | Show Context Actions… (fixes, Fix all in file, Ask Claude to fix) | Editor, Problems window |
| ⌥⇧↩ | Apply Top Fix (the best fix of the problem at the caret) | Editor |
| ⌥⇧F | Format Document | Editor |
| ⌥⌘L | Format Selection (the whole file when nothing is selected) | Editor |
| ⌥⇧O | Organize Imports | Editor |
| ⌃⌥P | Spring Active Profiles… | |
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
| ⌘R | Rerun (what ran last) | |
| ⌥⌘R | Rerun Failed Tests | |
| ⌃⌥G | Run Gradle Task… (Gradle tasks and Maven goals) | |
| F8 | Step Over | |
| F7 | Step Into | |
| ⇧F8 | Step Out | |
| ⌥F9 | Run to Cursor | Editor |
| F9 | Resume Program | |
| ⌥⌘F9 | Pause Program | |
| ⌥F8 | Evaluate Expression… | |
| ⌘F8 | Toggle Breakpoint | Editor |
| ⌥⌘F8 | Toggle Breakpoint Enabled | Editor |
| ⌥⇧⌘F8 | Edit Breakpoint Condition… | Editor |
| ⇧⌘F8 | View Breakpoints… | |

See [Run and debug](run-and-debug.md).

## Tool windows

These have no key out of the box; the command palette and **View ▸ Tool Window Actions** run them, and Settings ▸ Keymap can give them one.

| Command | What it does |
|---|---|
| Select Opened File | Shows the file in front in the Project tree |
| Collapse the Project Tree | Folds every folder of the Project tree |
| Hide the Side Panel | Closes the left panel |
| Sort Structure by Name | Sorts the Structure window by name, or back in file order |
| Collapse All Gradle Tasks | Folds the Gradle window |
| Collapse All Maven Goals | Folds the Maven window |
| Read the Maven Goals Again | Lists the Maven plugins' goals again |
| Reload Maven Project | Reads the Maven builds again |
| Collapse the Spring Window | Folds the Spring window |
| Clear Output | Empties the Output tab |

## Git

| Shortcut | Command | Where |
|---|---|---|
| ⇧⌘O | Check Out… | Outside an editor |
| ⌘A | Select all files | Outside text fields |
| ⌘⌫ | Move the selected files to the Trash, after asking | Outside text fields |
| Esc | Leave the commit view | Outside text fields |
| ↑ ↓ | Previous or next commit, while the Journal shows | Outside text fields |
| Esc | Close the popup | In the branch popup |
| ⌥⇧⌘B | Annotate with Git Blame (again: Close Annotations) | In an editor |

See [Git](git.md).

## Coding agents

| Shortcut | Command | Where |
|---|---|---|
| ⌘J | Show or hide the agent console | |
| ⌥⌘K | Send Selection to Claude (the console's agent) | Editor |
| ⌘↩ | Accept Claude's Change | A change Claude Code proposes |
| Esc | Reject Claude's Change | A change Claude Code proposes |
| ⌃⌥⇧↓ | Next Claude Change | Editor |
| ⌃⌥⇧↑ | Previous Claude Change | Editor |
| ⌥⌘Y | Accept Claude Change at Caret | Editor |
| ⌥⌘Z | Revert Claude Change at Caret | Editor |
| ⌥⇧⌘Y | Accept All Claude Changes | |
| ⌥⇧⌘Z | Revert All Claude Changes | |
| (none) | Show Agent Changes, Annotate with Agents | |

See [Coding agents](agents.md).

## Terminal

These keys are SnoutCode's in the Terminal tab and the Claude Code console; the others go to
the program running there.

| Shortcut | Command | Where |
|---|---|---|
| ⌘F | Find in the terminal, scrollback included | Terminal, console |
| ↩ / ⇧↩ | Previous / next match | The terminal's find bar |
| Esc | Close the find bar | The terminal's find bar |
| ⌘-click | Open the link under the pointer | Terminal, console |
| ⌘C / ⌘V | Copy the selection / paste | Terminal, console |
| ⌥ and a key | Meta and the key (Esc, then the key), unless Use Option as Meta is off | Terminal, console |

**Open in External Terminal** (File menu, command palette) has no shortcut out of the box: give
it one on the Keymap page.

See [Terminal](terminal.md).

## Dialogs

| Shortcut | Command | Where |
|---|---|---|
| Esc | Close (Cancel) | Every dialog |
| ⏎ | The main button: Create, Clone, Check Out, OK | Dialogs whose main button keeps your work |
| ⌘⌫ | Discard, Delete, Delete branch | Dialogs that discard or delete, where ⏎ does nothing |

The buttons show their keys: **Cancel** has Esc, the main button ⏎, a button that discards or deletes ⌘⌫.
