# SnoutCode's interface in English: the source catalog. Every message is here; the other
# languages translate these, and a message they leave out shows in English.
#
# Ids are grouped by where they show. Product names (Java, Gradle, Maven, Spring, Git, GitHub,
# Claude, SnoutCode) stay as they are in every language. No em dashes.

## Languages

# The choice that follows the system, with the language that gives.
language-system = System ({ $language })

## Light or dark

theme-system = System
theme-light = Light
theme-dark = Dark

## Settings

settings-nothing-matches = Nothing matches
settings-group-build-tools = Build Tools
settings-group-frameworks = Frameworks
settings-group-tools = Tools
settings-page-appearance = Appearance
settings-page-keymap = Keymap
settings-page-editor = Editor
settings-page-version-control = Version Control
settings-page-terminal = Terminal
settings-page-agents = Agents
settings-blurb-appearance = The language, colours, light or dark, and the typefaces of the window and the editor.
settings-blurb-keymap = The keys of every command. Add keys, take them away, or go back to the defaults.
settings-blurb-editor = How files indent, wrap and show as you type.
settings-blurb-version-control = How SnoutCode keeps your projects' Git up to date.
settings-blurb-gradle = How long SnoutCode's Gradle daemons stay without a build.
settings-blurb-spring = The profiles the Spring window evaluates the beans under.
settings-blurb-terminal = How the terminals look and what they run, and the terminal app to open folders in.
settings-blurb-agents = The coding agents SnoutCode follows, and the one its console runs.

settings-section-language = Language
settings-language = Interface language
settings-section-theme = Theme
settings-colors = Colors
settings-light-or-dark = Light or dark
settings-dark-only = { $name } has no light side: it stays dark, whatever this says.
settings-section-interface = Interface
settings-font = Font
settings-size = Size
settings-section-editor = Editor
settings-section-quitting = Quitting
settings-confirm-quit = Confirm before quitting
settings-confirm-quit-note = SnoutCode always asks when quitting would lose unsaved changes or stop something running.
settings-section-updates = Updates
settings-update-check = Check for updates automatically
settings-update-check-note = A new version downloads in the background and installs when you restart or quit. Help has Check for Updates at any time.

## The menu bar

menu-run = Run
menu-snoutcode = SnoutCode
menu-services = Services
menu-file = File
menu-open-recent = Open Recent
menu-edit = Edit
menu-view = View
menu-git = Git
menu-tool-window-actions = Tool Window Actions
menu-appearance = Appearance
menu-code = Code
menu-navigate = Navigate
menu-refactor = Refactor
menu-build = Build
menu-tools = Tools
menu-tool-settings = Tool Settings
menu-window = Window
menu-help = Help

## Commands: the menu items and the command palette

cmd-new-project = New Project…
cmd-open-project = Open Project…
cmd-close-project = Close Project
cmd-connect-to-ssh-host-or-wsl = Connect to SSH Host or WSL…
cmd-connect-to-ssh-host = Connect to SSH Host…
cmd-close-remote-connection = Close Remote Connection
cmd-clear-recent-projects = Clear Recent Projects
cmd-search-everywhere = Search Everywhere…
cmd-settings = Settings…
cmd-open-settings-json = Open Settings (JSON)
cmd-open-project-settings-json = Open Project Settings (JSON)
cmd-go-to-file = Go to File…
cmd-recent-files = Recent Files…
cmd-new-file = New File…
cmd-go-to-class = Go to Class…
cmd-go-to-line = Go to Line…
cmd-find-in-files = Find in Files…
cmd-replace-in-files = Replace in Files…
cmd-undo-replace-in-files = Undo Replace in Files
cmd-find-in-file = Find in File
cmd-replace-in-file = Replace in File
cmd-save = Save
cmd-close-tab = Close Tab
cmd-split-editor-right = Split Editor Right
cmd-toggle-comment = Toggle Comment
cmd-duplicate-line = Duplicate Line
cmd-move-line-up = Move Line Up
cmd-move-line-down = Move Line Down
cmd-toggle-soft-wrap = Toggle Soft Wrap
cmd-toggle-parameter-name-hints = Toggle Parameter Name Hints
cmd-go-to-definition = Go to Definition
cmd-go-to-declaration-or-usages = Go to Declaration or Usages
cmd-go-to-implementations = Go to Implementations
cmd-find-references = Find References
cmd-rename-symbol = Rename Symbol…
cmd-extract-method = Extract Method
cmd-extract-variable = Extract Variable
cmd-extract-constant = Extract Constant
cmd-extract-field = Extract Field
cmd-inline = Inline
cmd-show-context-actions = Show Context Actions…
cmd-apply-top-fix = Apply Top Fix
cmd-format-document = Format Document
cmd-format-selection = Format Selection
cmd-organize-imports = Organize Imports
cmd-show-parameter-hints = Show Parameter Hints
cmd-trigger-completion = Trigger Completion
cmd-go-to-symbol-in-workspace = Go to Symbol in Workspace…
cmd-go-to-symbol-in-file = Go to Symbol in File…
cmd-file-structure = File Structure…
cmd-structure = Structure
cmd-type-hierarchy = Type Hierarchy
cmd-call-hierarchy = Call Hierarchy
cmd-hierarchy = Hierarchy
cmd-show-problems = Show Problems
cmd-run = Run
cmd-debug = Debug
cmd-run-context-configuration = Run Context Configuration
cmd-debug-context-configuration = Debug Context Configuration
cmd-stop = Stop
cmd-rerun = Rerun
cmd-rerun-failed-tests = Rerun Failed Tests
cmd-build-project = Build Project
cmd-rebuild-project = Rebuild Project
cmd-run-gradle-task = Run Gradle Task…
cmd-run-maven-goal = Run Maven Goal…
cmd-show-the-build-window = Show the Build Window
cmd-show-the-gradle-window = Show the Gradle Window
cmd-show-the-maven-window = Show the Maven Window
cmd-terminal = Terminal
cmd-output = Output
cmd-settings-keymap = Settings: Keymap
cmd-settings-terminal = Settings: Terminal
cmd-settings-coding-agents = Settings: Coding Agents
cmd-settings-gradle-and-maven = Settings: Gradle and Maven
cmd-edit-configurations = Edit Configurations…
cmd-edit-environments = Edit Environments…
cmd-show-the-tests-window = Show the Tests Window
cmd-record-at-cursor = Record at Cursor
cmd-pick-locator = Pick Locator
cmd-stop-recording = Stop Recording
cmd-open-ui-mode = Open UI Mode
cmd-toggle-breakpoint = Toggle Breakpoint
cmd-toggle-breakpoint-enabled = Toggle Breakpoint Enabled
cmd-edit-breakpoint-condition = Edit Breakpoint Condition…
cmd-view-breakpoints = View Breakpoints…
cmd-evaluate-expression = Evaluate Expression…
cmd-run-to-cursor = Run to Cursor
cmd-step-over = Step Over
cmd-step-into = Step Into
cmd-step-out = Step Out
cmd-pause-program = Pause Program
cmd-resume-program = Resume Program
cmd-clear-output = Clear Output
cmd-sync-gradle-and-maven-projects = Sync Gradle and Maven Projects
cmd-project-structure = Project Settings…
cmd-refresh-gradle-tasks = Refresh Gradle Tasks
cmd-collapse-all-gradle-tasks = Collapse All Gradle Tasks
cmd-reload-maven-project = Reload Maven Project
cmd-read-the-maven-goals-again = Read the Maven Goals Again
cmd-collapse-all-maven-goals = Collapse All Maven Goals
cmd-spring-beans-and-endpoints = Spring Beans and Endpoints
cmd-read-the-spring-model-again = Read the Spring Model Again
cmd-go-to-spring-bean-or-usages = Go to Spring Bean or Usages…
cmd-spring-active-profiles = Spring Active Profiles…
cmd-collapse-the-spring-window = Collapse the Spring Window
cmd-reload-all-from-disk = Reload All from Disk
cmd-select-opened-file = Select Opened File
cmd-collapse-the-project-tree = Collapse the Project Tree
cmd-hide-the-side-panel = Hide the Side Panel
cmd-sort-structure-by-name = Sort Structure by Name
cmd-show-processes = Show Processes
cmd-show-the-agent-log = Show the Agent Log
cmd-stop-idle-processes = Stop Idle Processes
cmd-restart-the-java-engine = Restart the Java Engine
cmd-send-selection-to-claude = Send Selection to Claude
cmd-accept-claudes-change = Accept Claude's Change
cmd-reject-claudes-change = Reject Claude's Change
cmd-next-claude-change = Next Claude Change
cmd-previous-claude-change = Previous Claude Change
cmd-accept-claude-change-at-caret = Accept Claude Change at Caret
cmd-revert-claude-change-at-caret = Revert Claude Change at Caret
cmd-accept-all-claude-changes = Accept All Claude Changes
cmd-revert-all-claude-changes = Revert All Claude Changes
cmd-show-agent-changes = Show Agent Changes
cmd-annotate-with-agents = Annotate with Agents
cmd-annotate-with-git-blame = Annotate with Git Blame
cmd-show-history = Show History
cmd-show-history-of-file = Show History of File
cmd-undo = Undo
cmd-redo = Redo
cmd-zoom-in = Zoom In
cmd-zoom-out = Zoom Out
cmd-actual-size = Actual Size
cmd-show-project = Show Project
cmd-show-git = Show Git
cmd-toggle-agent-console = Toggle Agent Console
cmd-find-repositories-in-folder = Find Repositories in Folder…
cmd-clone-from-github = Clone from GitHub…
cmd-new-group = New Group…
cmd-refresh = Refresh
cmd-pull = Pull
cmd-fetch = Fetch
cmd-push = Push
cmd-commit = Commit
cmd-draft-commit-message = Draft Commit Message
cmd-stage = Stage
cmd-unstage = Unstage
cmd-discard-changes = Discard Changes…
cmd-check-out = Check Out…
cmd-new-branch = New Branch…
cmd-side-by-side-diff = Side by Side Diff
cmd-unified-diff = Unified Diff
cmd-compact-diff = Compact Diff
cmd-theme-match-the-system = Theme: Match the System
cmd-theme-dark = Theme: Dark
cmd-theme-light = Theme: Light
cmd-open-on-github = Open on GitHub
cmd-open-in-external-terminal = Open in External Terminal
cmd-about-snoutcode = About SnoutCode
cmd-check-for-updates = Check for Updates…
cmd-release-notes = Release Notes
cmd-quit-snoutcode = Quit SnoutCode
cmd-clear-recent = Clear Recent
cmd-exit = Exit
cmd-quit = Quit
cmd-zoom = Zoom
cmd-maximize = Maximize
cmd-command-palette = Command Palette…
cmd-cut = Cut
cmd-copy = Copy
cmd-paste = Paste
cmd-select-all = Select All
cmd-find = Find
cmd-replace = Replace
cmd-project = Project
cmd-git = Git
cmd-agent-console = Agent Console
cmd-soft-wrap = Soft Wrap
cmd-parameter-name-hints = Parameter Name Hints
cmd-processes = Processes
cmd-agent-log = Agent Log
cmd-build-window = Build Window
cmd-gradle = Gradle
cmd-maven = Maven
cmd-move-to-trash = Move to Trash…
cmd-agent-changes = Agent Changes
cmd-keymap = Keymap…
cmd-terminal-more = Terminal…
cmd-coding-agents = Coding Agents…
cmd-gradle-and-maven = Gradle and Maven…
cmd-minimize = Minimize
cmd-snoutcode-on-github = SnoutCode on GitHub
cmd-report-an-issue = Report an Issue
cmd-snoutdata-website = SnoutData Website
# The command palette: open a recent project, by its folder name.
cmd-open-recent-project = Open Recent: { $name }

## The platform's file manager

machine-mac = this Mac
machine-windows = this PC
machine-linux = this computer
reveal-mac = Reveal in Finder
reveal-windows = Show in Explorer
reveal-linux = Show in Files

## settings

settings-search-settings = Search settings
settings-found-on-path-and-the-usual-places = Found on PATH and the usual places
settings-its-programs-name = Its program's name
settings-your-login-shell-shell = Your login shell ($SHELL)
settings-no-arguments = No arguments
settings-name-value-then-return = NAME=value, then Return
settings-search-commands-or-shortcuts = Search commands or shortcuts
settings-overridden-by-this-project = Overridden by this project
settings-set-for-this-project = Set for this project
settings-reset-to-your-setting = Reset to your setting
settings-your-setting = Your setting
settings-user = User
settings-your-settings-json = Your settings.json
settings-for-only-kept-in-in-the-project-what = For { $root } only, kept in { $file } in the project. What it sets overrides your settings.
settings-your-settings-for-every-project-a-project-can = Your settings, for every project. A project can set some of them for itself, in Project Settings (⌘;).
settings-in-every-build = In every build
settings-reflow-kitty-keys-links = Reflow, kitty keys, links
settings-unavailable = Unavailable
settings-the-same-in-every-project = The same in every project
settings-settings-are-yours-a-project-cannot-set-them = { $title } settings are yours: a project cannot set them for itself.
settings-show-your-settings = Show Your Settings
settings-indentation = Indentation
settings-tab-size = Tab size
settings-for-a-file-whose-own-indentation-cannot-be = For a file whose own indentation cannot be told.
settings-display = Display
settings-soft-wrap-long-lines = Soft wrap long lines
settings-show-whitespace = Show whitespace
settings-show-parameter-names-of-literal-arguments-java = Show parameter names of literal arguments (Java)
settings-hover = Hover
settings-show-hover = Show hover
settings-delay = Delay
settings-navigation = Navigation
settings-click-with = Click with
settings-preview = Preview
settings-fetch-every = Fetch every
settings-the-projects-are-fetched-in-the-background-to = The projects are fetched in the background, to count what there is to pull.
settings-annotations = Annotations
settings-show-who-wrote-each-class-and-method-ada = Show who wrote each class and method (“Ada Lovelace +2”)
settings-show-the-current-lines-author-and-commit-after = Show the current line’s author and commit after its end
settings-good-to-know = Good to know
settings-a-background-fetch-leaves-your-branches-as-they = A background fetch leaves your branches as they are: it only counts the commits there are to pull.
settings-click-the-branch-in-the-title-bar-for = Click the branch in the title bar for New Branch, Fetch, Pull and Push, and every branch to check out.
settings-2-shows-the-git-view-in-the-message = ⌘2 shows the Git view. In the message field, ⌘↩ commits.
settings-each-builds-own-your-settings = Each build's own (for its Java level)
settings-every-build = Every build
settings-jdk-per-project = JDK per project
settings-no-gradle-build-is-open = No Gradle build is open.
settings-automatic-the-builds-java-level = Automatic (the build's Java level)
settings-the-java-server-restarts-on-the-chosen-jdk = The Java server restarts on the chosen JDK the next time it starts.
settings-gradle-daemons = Gradle daemons
settings-stop-idle-daemons-after = Stop idle daemons after
settings-double-click-a-task-in-the-gradle-tool = Double click a task in the Gradle tool window to run it. Its output shows in the Build window.
settings-the-processes-window-lists-the-gradle-daemons-running = The Processes window lists the Gradle daemons running, and stops the idle ones on request.
settings-project-structure-downloads-jdks-and-sets-the-projects = Project Settings (⌘;) downloads JDKs and sets the project's own.
settings-the-leaf-on-the-right-stripe-opens-the = The leaf on the right stripe opens the Spring window: the beans and the endpoints SnoutCode found.
settings-spring-active-profiles-p-changes-the-profiles-from = Spring Active Profiles (⌃⌥P) changes the profiles from anywhere.
settings-a-bean-the-active-profiles-leave-out-is = A bean the active profiles leave out is dimmed, and no injection takes it.
settings-active-profiles = Active profiles
settings-open-a-project-to-choose-its-profiles = Open a project to choose its profiles.
settings-any-no-profile-chosen = Any (no profile chosen)
settings-those-the-application-files-make-active = Those the application files make active
settings-these-profiles = These profiles
settings-evaluate-beans-under = Evaluate beans under
settings-profiles = Profiles
settings-choose = Choose
settings-looking-for-it = Looking for it…
settings-that-file-is-not-there = That file is not there.
settings-not-found-install-it-from-the-console-or = Not found. Install it from the console, or choose the program.
settings-browse = Browse…
settings-sees-the-editor-its-hooks-report-each-edit = Sees the editor; its hooks report each edit and prompt
settings-its-hooks-report-each-edit-and-prompt = Its hooks report each edit and prompt
settings-a-plugin-of-snoutcodes-reports-each-edit-and = A plugin of SnoutCode's reports each edit and prompt
settings-files-seen-on-disk-while-it-works-in = Files seen on disk while it works in the console
settings-its-colour-caret-and-marks = Its colour, caret and marks
settings-the-console-runs = The console runs
settings-programs = Programs
settings-your-agent = Your agent
settings-its-name = Its name
settings-any-command-line-agent-the-console-runs-it = Any command line agent: the console runs it in the project with /bin/sh -c.
settings-j-shows-or-hides-the-console-the-session = ⌘J shows or hides the console. The session keeps running while it is hidden.
settings-k-sends-the-selection-or-the-whole-file = ⌥⌘K sends the selection, or the whole file, to the agent.
settings-in-a-proposed-changes-tab-accepts-it-and = In a proposed change's tab, ⌘↩ accepts it and Esc rejects it.
settings-the-agent-changes-window-keeps-every-edit-an = The Agent Changes window keeps every edit an agent made, by prompt, to accept or revert.
settings-ghostty-is-unavailable = Ghostty is unavailable
settings-terminal-engine = Terminal engine
settings-use-the-editors = Use the editor's
settings-the-editors-size = The editor's size
settings-blink = Blink
settings-look = Look
settings-line-height = Line height
settings-cursor = Cursor
settings-remove-this-variable = Remove this variable
settings-shell = Shell
settings-program = Program
settings-empty-the-login-shell-of-your-account-on = Empty: the login shell of your account on the remote machine.
settings-empty-your-login-shell-on-a-remote-machine = Empty: your login shell. On a remote machine, your shell there.
settings-arguments = Arguments
settings-separated-by-spaces-l-starts-a-login-shell = Separated by spaces. -l starts a login shell.
settings-environment = Environment
settings-add = Add
settings-added-to-the-terminals-and-the-claude-code = Added to the terminals and the Claude Code console you start after a change.
settings-behaviour = Behaviour
settings-scrollback = Scrollback
settings-lines-for-terminals-started-after-a-change = lines, for terminals started after a change
settings-copy-text-as-soon-as-it-is-selected = Copy text as soon as it is selected
settings-use-option-as-meta-f-moves-a-word = Use Option as Meta (⌥F moves a word, not ƒ)
settings-external-terminal = External terminal
settings-open-in = Open in
settings-command = Command
settings-running-now = Running now
settings-a-terminal-running-another-engine-keeps-it-until = A terminal running another engine keeps it until it starts again (New, in its header).
settings-press-the-keys-or-esc = Press the keys, or Esc
settings-none = None
settings-was-the-shortcut-of-here-it-now-runs = { $keys } was the shortcut of { $join }. Here it now runs this command.
settings-remove-it-there = Remove It There
settings-no-command-matches = No command matches.
settings-reset-all = Reset All
settings-adds-a-shortcut-press-its-keys-changes-apply = + adds a shortcut: press its keys. Changes apply with Apply or OK.
settings-settings = Settings
settings-edit-as-json = Edit as JSON
settings-open-these-settings-file-in-a-tab-with = Open these settings' file in a tab, with completion and checks
settings-cancel = Cancel
settings-apply = Apply
settings-cannot-be-read = { $file } cannot be read
settings-the-values-it-had-before-stay-in-use = The values it had before stay in use until it is fixed. { $e }
settings-in-shop = { $shell } in shop
settings-engine = { $shown } engine
settings-found = Found: { $path }
settings-editors = Editor's ({ $editor_font })
settings-not-installed = { $chosen } (not installed)
settings-in = { $kind } in { $root }
settings-reset = Reset
settings-also-bound-to = Also bound to { $join }
settings-no-jdk = No JDK was found on { $machine }.
settings-after-a-delay = After a delay
settings-only-while-is-held = Only while ⌘ is held
settings-block = Block
settings-bar = Bar
settings-underline = Underline
settings-hover-hint = For problems and their fixes, and what a symbol is. ⌘ always shows it at once, with the link to the declaration.
settings-click-hint = Hold it to underline a name as a link, and click to go to its declaration (on the declaration, its usages). ⌥⌘-click goes to the implementations.
settings-blame-hint = From Git’s blame of the file. The line’s author shows once the caret rests for the hover’s delay (Editor ▸ Hover). Annotate with Git Blame is in the gutter’s right-click menu and the Git menu.
settings-daemons-hint = SnoutCode’s own Gradle daemons stop after this long without a build. A new value holds for the daemons started after the Java engine starts again.
settings-profiles-hint = The Spring window evaluates the beans under these profiles and dims those they leave out. Its profile menu changes the same setting.
settings-agent-colours-hint = Each agent writes in its own colour: a caret with its name while it types, a mark in the gutter beside the lines it changed, and a dot on the tabs and files it changed until you review them.
settings-programs-hint = Leave them empty and SnoutCode finds each program, on your PATH and where installers put it. Browse to choose one yourself.
settings-terminal-scope-hint = The terminals’ look, engine and keys, and the external terminal, are yours: they are the same in every project.
settings-engine-hint = The engine reads what runs in the terminals and the Claude Code console. A change applies to the terminals you start after it; open terminals keep the engine they started with.
settings-ghostty-preview-hint = Ghostty also draws the deprecated call struck through, its curly warning line, and the test report’s link.
settings-custom-terminal-hint = Run with /bin/sh. { "{dir}" } is the folder; on a remote machine, { "{ssh}" } is the ssh command that opens a shell there.
settings-external-terminal-hint = Open in External Terminal, in the File menu and on a file’s or folder’s menu, opens it at the folder. On a remote project it opens an ssh session there.
settings-unit-minutes = minutes

## dialogs

dialogs-discard-body = { $count ->
    [one] This puts this file back as it is in HEAD. Untracked files are deleted.
   *[other] This puts these { $count } files back as they are in HEAD. Untracked files are deleted.
    }
dialogs-trash-body = { $count ->
    [one] This file goes to the Trash. Tracked files then show as deleted, ready to commit.
   *[other] These { $count } files go to the Trash. Tracked files then show as deleted, ready to commit.
    }
dialogs-delete-body = { $count ->
    [one] This file is deleted from disk. Tracked files then show as deleted, ready to commit.
   *[other] These { $count } files are deleted from disk. Tracked files then show as deleted, ready to commit.
    }
dialogs-unsaved-files = { $count ->
    [one] You have unsaved changes in a file.
   *[other] You have unsaved changes in { $count } files.
    }
dialogs-version = Version { $cargo_pkg_version }
dialogs-git-stopped-and-said-why = Git stopped and said why.
dialogs-you-have-uncommitted-changes-what-should-happen-to = You have uncommitted changes. What should happen to them?
dialogs-a-line-of-work-of-its-own-from = A line of work of its own, from where you choose.
dialogs-its-commits-stay-wherever-else-they-are-reachable = Its commits stay wherever else they are reachable.
dialogs-it-goes-for-everyone-who-uses-the-remote = It goes for everyone who uses the remote. Local branches stay as they are.
dialogs-copy-a-remote-repository-to-this-computer-and = Copy a remote repository to this computer and open it.
dialogs-is-waiting-in-the-console-for-your-answer = { $agent } is waiting in the console for your answer.
dialogs-the-window-closes-and-the-java-engine-stops = The window closes, and the Java engine stops with it.
dialogs-what-runs-now-stops-when-snoutcode-quits = What runs now stops when SnoutCode quits.
dialogs-clone-repository = Clone repository
dialogs-local-branches = Local branches
dialogs-remote-branches = Remote branches
dialogs-tags = Tags
dialogs-unsaved = Unsaved
dialogs-these-stop = These stop:
dialogs-dont-ask-again-settings-appearance-confirm-before-quitti = Don't ask again (Settings, Appearance: Confirm before quitting)
dialogs-discard-and-quit = Discard and Quit
dialogs-save-all-and-quit = Save All and Quit
dialogs-an-ide-for-java-gradle-and-spring-boot = An IDE for Java, Gradle and Spring Boot, with Git and Claude Code built in.
dialogs-made-by-snoutdata-mit-license = Made by SnoutData. MIT License.
dialogs-this-cannot-be-undone = This cannot be undone.
dialogs-discard = Discard
dialogs-you-can-still-get-them-back-from-the = You can still get them back from the Trash.
dialogs-move-to-trash = Move to Trash
dialogs-delete = Delete
dialogs-stash-check-out-re-apply = Stash, check out, re-apply
dialogs-put-the-changes-aside-switch-then-bring-them = Put the changes aside, switch, then bring them back. Works even if they touch files that differ on the target.
dialogs-check-out-keep-changes = Check out, keep changes
dialogs-carry-the-changes-across-as-they-are-git = Carry the changes across as they are. Git refuses if they would be overwritten.
dialogs-this-leaves-head-detached-create-a-branch-if = This leaves HEAD detached. Create a branch if you want to keep new commits.
dialogs-and-have-each-moved-on-and-this-repositorys = { $branch } and { $upstream } have each moved on, and this repository's settings do not say how to combine them.
dialogs-merge = Merge
dialogs-join-both-histories-with-a-merge-commit-nothing = Join both histories with a merge commit. Nothing is rewritten.
dialogs-rebase = Rebase
dialogs-replay-your-commits-on-top-of-the-remote = Replay your commits on top of the remote ones, for a straight history.
dialogs-and-have-no-commit-in-common-so-they = { $branch } and { $upstream } have no commit in common, so they cannot be merged. This usually means the remote repository was recreated or its history was rewritten.
dialogs-resetting-makes-match-exactly-your-current-commits-stay = Resetting makes { $branch } match { $upstream } exactly. Your current commits stay on a backup branch, so nothing is lost.
dialogs-commit-or-stash-your-uncommitted-changes-first-resetting = Commit or stash your uncommitted changes first; resetting would discard them.
dialogs-reset-to = Reset to { $upstream }
dialogs-name = Name
dialogs-starting-at = Starting at
dialogs-check-out-the-new-branch = Check out the new branch
dialogs-create = Create
dialogs-renaming = Renaming { $old }
dialogs-goes-into-the-new-group = { $repo } goes into the new group.
dialogs-groups-keep-related-repositories-together-drag-repositor = Groups keep related repositories together. Drag repositories onto one.
dialogs-rename = Rename
dialogs-goes-away = goes away.
dialogs-delete-even-if-it-is-not-fully-merged = Delete even if it is not fully merged
dialogs-also-delete-on-the-remote = Also delete { $up } on the remote
dialogs-removing-it-from-the-remote-affects-everyone-who = Removing it from the remote affects everyone who uses it.
dialogs-delete-branch = Delete branch
dialogs-repository-url = Repository URL
dialogs-into-folder = Into folder
dialogs-lands-in = Lands in
dialogs-clone = Clone
dialogs-no-matching-branch-tag-or-commit = No matching branch, tag, or commit
dialogs-uncommitted-changes = Uncommitted changes:
dialogs-keep-them = Keep them
dialogs-stash-and-re-apply = Stash and re-apply
dialogs-check-out = Check Out
dialogs-dont-allow = Don't Allow
dialogs-allow-for-this-session = Allow for This Session
dialogs-allow = Allow
dialogs-close = Close
dialogs-a-branch-a-tag-or-any-commit = A branch, a tag, or any commit.
dialogs-something-went-wrong = Something went wrong
dialogs-discard-changes = Discard changes?
dialogs-check-out-2 = Check out { $t }?
dialogs-check-out-3 = Check out
dialogs-new-branch = New branch
dialogs-delete-branch-2 = Delete branch?
dialogs-delete-branch-from = Delete branch from { $remote }?
dialogs-move-to-the-trash = Move to the Trash?
dialogs-delete-files = Delete files?
dialogs-both-sides-have-new-commits = Both sides have new commits
dialogs-no-shared-history = No shared history
dialogs-git-could-not-sign-in-to = Git could not sign in to { $host }
dialogs-git-could-not-sign-in = Git could not sign in
dialogs-sign-in-helper-missing = The credential helper set for this repository needs { $program }, which is not installed here.
dialogs-sign-in-helper-broken = The credential helper set for this repository cannot run on this computer.
dialogs-sign-in-rejected = The server refused the user name and token Git sent.
dialogs-sign-in-none = Git has no user name or token for this server.
dialogs-sign-in-no-access = The server says the repository is not there, or this account cannot see it.
dialogs-sign-in-ssh-key = The server accepted none of this computer’s SSH keys.
dialogs-sign-in-host-key = SSH does not know this server’s host key yet, or the key changed.
dialogs-sign-in-fix-helper = This happens when a repository was copied from another computer with its settings, or the sign in saved here was lost. Install the program, or remove the helper and sign in again with credentials of your own.
dialogs-sign-in-fix-rejected = The saved token may have expired, been revoked, or belong to another account. Sign in again, or make a new token and save it.
dialogs-sign-in-fix-none = Sign in once with a credential helper that saves it, or switch the remote to SSH.
dialogs-sign-in-fix-no-access = Check the remote’s address, and that the account Git signs in as can see the repository. With several accounts, the saved one may be the wrong one.
dialogs-sign-in-fix-ssh-key = Add this computer’s public key to your account on the server, or load your key into the SSH agent.
dialogs-sign-in-fix-host-key = Connect once in a terminal, check the fingerprint the server shows, then accept it. A key that changed can mean the server was replaced, or that someone is in between.
dialogs-sign-in-agent = { $agent } gets what Git said, finds the cause with you and walks you through the fix.
dialogs-fix-with = Fix with { $agent }
dialogs-open-terminal = Open Terminal
dialogs-rename-group = Rename group
dialogs-new-group = New group
dialogs-quit-snoutcode = Quit SnoutCode?
dialogs-allow-to = Allow { $agent } to { $what }?
dialogs-its-upstream = its upstream
dialogs-delete-from = Delete from { $remote }
dialogs-allow-for-this-session-lets-it-use-again = Allow for This Session lets it use { $tool } again without asking.

## welcome

welcome-minutes-ago = { $count ->
    [one] { $count } minute ago
   *[other] { $count } minutes ago
    }
welcome-hours-ago = { $count ->
    [one] { $count } hour ago
   *[other] { $count } hours ago
    }
welcome-days-ago = { $count ->
    [one] { $count } day ago
   *[other] { $count } days ago
    }
welcome-weeks-ago = { $count ->
    [one] { $count } week ago
   *[other] { $count } weeks ago
    }
welcome-months-ago = { $count ->
    [one] { $count } month ago
   *[other] { $count } months ago
    }
welcome-just-now = just now
welcome-yesterday = yesterday
welcome-search-recent-projects = Search recent projects
welcome-welcome = Welcome
welcome-go-to-file = Go to File
welcome-recent-files = Recent Files
welcome-search-everywhere = Search Everywhere
welcome-find-in-files = Find in Files
welcome-new-file = New File
welcome-no-files-open = No files open
welcome-open-one-from-the-project-tree-or = Open one from the Project tree, or:
welcome-recent-files-2 = Recent files
welcome-clone-repository = Clone Repository…
welcome-from-a-git-url = From a Git URL
welcome-java-or-spring-boot-app = Java or Spring Boot app
welcome-no-recent-project-matches = No recent project matches.
welcome-no-recent-projects-yet-open-a-folder-to = No recent projects yet. Open a folder to start.
welcome-recent-projects = Recent Projects
welcome-a-folder-on = A folder on { $host }
welcome-folder-on-machine = A folder on { $machine }

## tabs

tabs-unpin-tab = Unpin Tab
tabs-pin-tab = Pin Tab
tabs-close-others = Close Others
tabs-close-tabs-to-the-left = Close Tabs to the Left
tabs-close-tabs-to-the-right = Close Tabs to the Right
tabs-close-all = Close All
tabs-close-all-but-pinned = Close All but Pinned
tabs-keep-open = Keep Open
tabs-reload-from-disk = Reload from Disk
tabs-copy-path = Copy Path
tabs-clear = Clear
tabs-sync-the-gradle-project = Sync the Gradle Project
tabs-read-the-tasks-again = Read the Tasks Again
tabs-collapse-all = Collapse All
tabs-reload-the-maven-project = Reload the Maven Project
tabs-read-the-goals-again = Read the Goals Again
tabs-stop-idle-processes-idle-gradle-daemons-leftovers-and = Stop Idle Processes: idle Gradle daemons, leftovers, and the engines of projects not open
tabs-options = Options
tabs-hide = Hide

## projects

projects-filter = Filter
projects-open-local-repository = Open local repository…
projects-find-repositories-in-folder = Find repositories in folder…
projects-no-matches = No matches
projects-rename = Rename…
projects-delete-group = Delete group
projects-new-group = New group…
projects-drag-repositories-here = Drag repositories here
projects-add-a-repository-or-a-group = Add a repository or a group
projects-repositories = Repositories
projects-no-repositories-yet-use-or-o = No repositories yet. Use + or ⌘O.
projects-copy-path = Copy path
projects-open-as-project = Open as Project
projects-move-to-group = Move to group
projects-no-group = No group
projects-remove-from-list = Remove from list

## status

status-snoutcodes-processes-running-of-memory-click-to-show = SnoutCode's processes: { $count } running, { $memory } of memory. Click to show them.
status-in-the-background = In the background
status-the-editor-works-meanwhile = The editor works meanwhile.
status-reconnect = Reconnect
status-changes-the-agents-made-that-wait-for-your = Changes the agents made that wait for your review
status-review = Review
status-restart = Restart
status-read-only = Read only
status-java-support-unavailable = Java support unavailable
status-chars-lines = { $chars } chars, { $lines } lines
status-selected = { $chars } selected
status-and-more = and { $count } more

## search

search-go-to-file = Go to file
search-line-or-line-column = Line, or line:column
search-type-a-command = Type a command
search-run-a-gradle-task-or-maven-goal-its = Run a Gradle task or Maven goal: its name or path
search-spring-profiles-pick-one-to-turn-it-on = Spring profiles: pick one to turn it on or off
search-go-to-symbol = Go to symbol
search-filter-the-files-symbols = Filter the file's symbols
search-go-to-class-in-the-project-its-libraries = Go to class, in the project, its libraries and the JDK
search-new-file-its-path-in-the-project-like = New file: its path in the project, like src/main/java/App.java
search-no-file-matches = No file matches
search-no-command-matches = No command matches
search-no-task-matches = No task matches
search-no-profile-matches = No profile matches
search-type-a-name = Type a name
search-no-symbol-matches = No symbol matches
search-type-a-class-name-or-its-capitals = Type a class name, or its capitals
search-no-class-matches = No class matches
search-no-recent-file-matches = No recent file matches
search-return-makes-the-file-and-opens-it = Return makes the file and opens it
search-type-a-line-number-or-line-column = Type a line number, or line:column
search-go-to-line-column = Go to line { $line }, column { $column }
search-try-fewer-letters-or-another-word = Try fewer letters, or another word.

## find

find-matches = { $count ->
    [one] { $count } match
   *[other] { $count } matches
    }
find-files = { $count ->
    [one] { $count } file
   *[other] { $count } files
    }
find-matches-in-files = { $matches } in { $files }
find-replaced = Replaced { $matches } in { $files }
find-put-back = Put back { $files }
find-to-replace = , { $ticked } to replace
find-first-only = {" "}(the first ones only)
find-skipped = ; { $files } changed since and left alone
find-undo-hint = . Edit ▸ Undo Replace in Files puts it back.
find-not-a-regular-expression = Not a regular expression: { $error }
find-the-regular-expression-is-too-large = The regular expression is too large
find-module = Module
find-directory = Directory
find-open-files = Open Files
find-text-to-find = Text to find
find-replace-with = Replace with
find-file-mask-java-test-java = File mask: *.java, !*Test.java
find-folder-from-the-projects = Folder, from the project's
find-match-case = Match Case
find-whole-words = Whole Words
find-regular-expression-1-in-the-replacement-names-a = Regular Expression ($1 in the replacement names a group)
find-hide-replace = Hide Replace
find-replace-in-files-r = Replace in Files (⇧⌘R)
find-choose = Choose…
find-find-in-the-projects-files = Find in the project's files
find-type-a-word-a-phrase-or-a-regular = Type a word, a phrase or a regular expression above.
find-nothing-found = Nothing found
find-try-fewer-letters-another-scope-or-a-wider = Try fewer letters, another scope, or a wider file mask.
find-searching = Searching…
find-replace-selected = Replace Selected
find-replace-all = Replace All
find-open = Open
find-replace-in-files = Replace in Files
find-no-replacement-to-undo = No replacement to undo
find-not-a-file-mask = Not a file mask: { $kind }
find-module-2 = Module: { $name }
find-of = { $ticked } of { $count }

## files

files-no-project-open = No project open
files-open-a-project-to-see-its-files = Open a project to see its files.
files-the-connection-is-lost = The connection is lost
files-node-package = Node package: { $path }
files-the-files-on-come-back-when-it-reconnects = The files on { $host } come back when it reconnects.

## project

project-no-classes-to-show = No classes to show
project-reading-the-build-model = Reading the build model
project-external-libraries = External Libraries
project-the-build-model-could-not-be-read = The build model could not be read: { $e }
project-uses = uses { $join }
project-showing-the-last-model-that-synced = { $reason }{"\u000A"}Showing the last model that synced.
project-compiled-by = Compiled by { $join }
project-did-not-sync = Did not sync: { $reason }{"\u000A"}{"\u000A"}{ $note }
project-did-not-sync-2 = Did not sync: { $reason }
project-excluded = Excluded: { $path }
project-and-more = { $builds } and { $n } more

## model

model-build-files = Build Files
model-maven-files = Maven Files
model-gradle-scripts = Gradle Scripts
model-detached-at = detached at { $count }
model-detached-head = detached HEAD
model-does-not-exist = { $path } does not exist
model-ready = Ready
model-no-upstream = {"  "}(no upstream)
model-opened = Opened { $asked }
model-snoutcode-could-not-connect-to = SnoutCode could not connect to { $connection }. { $why }
model-showing-in-git = Showing { $root } in Git
model-closed = Closed { $closed }
model-could-not-save-settings = Could not save settings: { $e }
model-your-settings-json-has-an-error-so-the = Your settings.json has an error, so the settings in use stay: { $e }
model-your-settings-are-read-again = Your settings are read again
model-could-not-save-the-projects-settings = Could not save the project's settings: { $e }
model-finished = { $label } finished
model-cloned-into = Cloned into { $root }
model-failed = { $label } failed
model-failed-2 = { $label } failed:{"\u000A"}{ $e }
model-no-git-repositories-within-levels-of = No Git repositories within { $SCAN_DEPTH } levels of{"\u000A"}{ $root }
model-found-repositories-in-new = Found { $count } repositories in { $root } ({ $added } new)
model-asking-the-agent-to-draft-the-commit-message = Asking the agent to draft the commit message…
model-the-agent-has-not-sent-a-commit-message = The agent has not sent a commit message yet. Its answer is in the console.
model-asks-you-something-in-the-console-answer-it = { $name } asks you something in the console. Answer it there, and the draft goes on.
model-is-writing-the-commit-message = { $name } is writing the commit message…
model-the-agent-finished-without-writing-a-commit-message = The agent finished without writing a commit message. Its answer is in the console.
model-no-commit-message-yet = No commit message yet
model-drafted-the-commit-message = { $name } drafted the commit message
model-refreshed = Refreshed
model-open-project = Open project
model-initialized-repository-in = Initialized repository in { $root }
model-find-repositories-in-folder = Find repositories in folder
model-searching-for-repositories = Searching for repositories
model-staged = { $count ->
    [one] Staged 1 file
   *[other] Staged { $count } files
    }
model-discarded = { $count ->
    [one] Discarded 1 file
   *[other] Discarded { $count } files
    }
model-trashed = { $count ->
    [one] Moved 1 file to the Trash
   *[other] Moved { $count } files to the Trash
    }
model-deleted = { $count ->
    [one] Deleted 1 file
   *[other] Deleted { $count } files
    }
model-unstaged = Unstaged
model-added-to-gitignore = Added { $pattern } to .gitignore
model-added-rules-to-gitignore = Added { $n } rules to .gitignore
model-committed = Committed { $summary }
model-stopped-drafting = Stopped drafting
model-checked-out = Checked out { $target }
model-local-changes-conflicted = Local changes conflicted
model-created-branch = Created branch { $name }
model-deleted-branch = Deleted branch { $name }
model-delete = Delete { $r }
model-pull-merge = Pull (merge)
model-pull-rebase = Pull (rebase)
model-reset-to = Reset { $head } to { $upstream }
model-your-previous-commits-are-kept-on-the-branch = Your previous commits are kept on the branch { $backup }. Delete it once you no longer need them.
model-now-matches = { $head } now matches { $upstream }
model-commit = Commit { $short_id } { $summary }
model-not-in-the-repository = { $path } is not in the repository the Git view shows
model-sources = Sources
model-test-sources = Test sources
model-test-resources = Test resources
model-generated-sources = Generated sources
model-the-build-did-not-sync = The build did not sync
model-sync = Sync: { $root }
model-the-gradle-sync-failed = The Gradle sync failed

## branch

branch-search-branches = Search branches
branch-local = Local
branch-remote = Remote

## panels

panels-files = Files
panels-no-output-yet = No output yet
panels-what-git-and-the-tools-say-shows-here = What Git and the tools say shows here as they work.
panels-open-git = Open Git

## title

title-no-project = No Project
title-current-file = Current File
title-yours = Yours
title-reading-the-configurations = Reading the configurations…
title-configurations-appear-once-the-tests-are-listed = Configurations appear once the tests are listed
title-configurations-appear-once-the-java-engine-is-ready = Configurations appear once the Java engine is ready
title-stop-f2 = Stop (⌘F2)
title-search-everywhere-a = Search Everywhere (⇧⌘A)
title-settings = Settings (⌘,)
title-the-current-file = the Current File
title-rerun-r = Rerun { $what } (⌃R)
title-run-r = Run { $what } (⌃R)
title-debug-d = Debug { $what } (⌃D)

## stripes

stripes-left = Left
stripes-right = Right
stripes-bottom = Bottom
stripes-move-to = Move to
stripes-build = Build
stripes-problems-m = Problems (⇧⌘M)
stripes-agent-console-j = Agent Console (⌘J)
stripes-spring = Spring
stripes-notifications = Notifications
stripes-structure-7 = Structure (⌘7)
stripes-hierarchy-h-h = Hierarchy (⌃H, ⌃⌥H)
stripes-agent-view = Agent View
stripes-tests = Tests
stripes-project-1 = Project (⌘1)
stripes-git-2 = Git (⌘2)

## git

git-state = State
git-relative-directory = Relative Directory
git-file-filter = File filter
git-message-author-sha-or-ref = Message, author, SHA or ref
git-commit-message = Commit message
git-message = Message
git-author = Author
git-date = Date
git-push = Push ({ $n })
git-hide = Hide { $short }
git-console-in-this-repository-j = { $agent } console in this repository (⌘J)
git-no-github-remote = No GitHub remote
git-open-local-repository-o = Open local repository (⌘O)
git-git-push-sets-upstream-on-first-push = git push (sets upstream on first push)
git-move-to-the-trash = Move to the Trash (⌘⌫)
git-branch-tag-or-commit-o = Branch, tag or commit (⇧⌘O)
git-new-branch = New Branch
git-no-repository-open = No repository open
git-local-branches = Local Branches
git-tags = Tags ({ $count })
git-branches = Branches
git-check-out-o = Check out… (⇧⌘O)
git-double-click-to-check-out = { $b }{ $u }{"\u000A"}{ $target } { $subject }{"\u000A"}{"\u000A"}Double-click to check out
git-double-click-to-check-out-detached = { $target } { $subject }{"\u000A"}{ $date }{"\u000A"}{"\u000A"}Double-click to check out (detached)
git-delete = Delete…
git-delete-from = Delete from { $remote }…
git-check-out-as-local = Check out as local “{ $local_name }”
git-delete-branch = Delete branch…
git-delete-from-the-remote = Delete from the remote…
git-create-branch-from-here = Create branch from here…
git-copy-name = Copy name
git-check-out-detached = Check out (detached)
git-create-branch-from-tag = Create branch from tag…
git-tag = tag { $name }
git-nothing-to-commit-on = Nothing to commit on { $head }
git-nothing-to-commit = Nothing to commit
git-show-all-files = Show all files
git-in = in { $dir }/
git-the-working-tree-is-clean = The working tree is clean
git-discard = Discard…
git-add-to-gitignore = Add to .gitignore
git-see-console = See console
git-writing = Writing…
git-writes-the-message-from-building-on-anything-you = { $agent } writes the message from { $what }, building on anything you typed. If it is not running, it starts in the console.
git-draft = Draft
git-amend = Amend
git-amend-last-commit = Amend last commit
git-commits = Commits { $what }  ⌘↩
git-files-selected = { $count } files selected
git-select-a-file-to-see-its-changes = Select a file to see its changes
git-changes = Changes
git-nothing-staged = Nothing staged
git-stage-files-to-see-what-the-next-commit = Stage files to see what the next commit holds.
git-journal = Journal
git-click-a-commit-to-see-what-it-changed = Click a commit to see what it changed
git-history-of = History of { $name }
git-history-of-tooltip = The commits that changed { $path }, renames followed. Click to see the whole Journal again.
git-no-commits-changed-it = No commit changed this file
git-it-is-not-committed-yet = It is new, or was never committed on this branch.
git-no-commits-yet = No commits yet
git-stage-some-files-and-make-the-first-one = Stage some files and make the first one
git-show-changes = Show changes
git-check-out-this-commit-detached = Check out this commit (detached)
git-create-branch-here = Create branch here…
git-copy-sha = Copy SHA
git-copy-message = Copy message
git-working-tree = Working tree
git-back-esc = Back (Esc)
git-the-commit-could-not-be-read = The commit could not be read
git-from = { $directory }  (from { $orig })
git-this-commit-changes-no-files = This commit changes no files
git-committer = Committer
git-committed = Committed
git-parents = Parents
git-parent = Parent
git-none-this-is-the-first-commit = none, this is the first commit
git-select-a-file = Select a file
git-nothing = (nothing)
git-this-commit = This commit
git-git-could-not-read-this-folder = Git could not read this folder
git-the-project-works-without-it-but-its-changes = The project works without it, but its changes and history cannot show.
git-this-folder-has-no-git-repository = This folder has no Git repository
git-the-project-works-without-git-start-a-repository = The project works without Git. Start a repository to track its changes and history.
git-try-again = Try Again
git-initialize-a-repository-here = Initialize a Repository Here
git-find-repositories-inside = Find Repositories Inside
git-open-a-project-to-see-its-changes-branches = Open a project to see its changes, branches and history.
git-open-project-o = Open Project…   ⌘O
git-unified = Unified
git-side-by-side = Side by Side
git-compact-hide-unchanged-lines-away-from-changes = Compact: hide unchanged lines away from changes
git-compact = Compact
git-hide-unchanged-lines-away-from-changes = Hide unchanged lines away from changes
git-no-diff = No diff
git-binary-file = Binary file
git-its-versions-differ-as-bytes = Its versions differ, as bytes.
git-too-large-to-show = Too large to show
git-the-file-is-too-large-to-diff-here = The file is too large to diff here.
git-no-differences = No differences
git-and-are-the-same = { $left } and { $right } are the same.
git-changed = { $count ->
    [one] { $count } changed
   *[other] { $count } changed
    }
git-changed-staged = { $count } changed, { $staged_count } staged

## console

console-console = Console
console-the-installer-is-still-at-work-end-stops = The installer is still at work; End stops it
console-ended = { $name } ended
console-ended-resume = { $name } ended; Continue resumes it
console-your-own-agent-has-no-command-yet-give = Your own agent has no command yet. Give it one in Settings, Agents, then click Draft again.
console-find-in-terminal = Find in terminal
console-of = { $i } of { $n }
console-previous-match-return = Previous match (Return)
console-next-match-shift-return = Next match (Shift+Return)
console-close-esc = Close (Esc)
console-open-a-project-to-run-a-shell-in = Open a project to run a shell in it.
console-not-running = Not running
console-starting = Starting
console-connected = Connected
console-not-connected = Not connected
console-running = Running
console-working = Working
console-installing = Installing
console-ended-2 = Ended
console-is-not-running = { $agent } is not running
console-is-starting = { $agent } is starting
console-sees-the-editor-its-selection-open-files-and = { $agent } sees the editor: its selection, open files and problems; it proposes changes there for you to accept or reject, and its edits show as it makes them
console-runs-but-has-not-connected-to-the-editor = { $agent } runs, but has not connected to the editor yet
console-runs-the-files-it-changes-show-as-it = { $agent } runs; the files it changes show as it works
console-is-at-work-on-your-prompt = { $agent } is at work on your prompt
console-is-being-installed = { $agent } is being installed
console-has-ended = { $agent } has ended
console-the-shell = The shell
console-follow = Follow
console-following-the-editor-goes-to-the-files-the = Following: the editor goes to the files the agent changes, as it types
console-not-following-the-editor-stays-where-you-are = Not following: the editor stays where you are; dots mark what the agent changed
console-start-again-with-a-new-conversation = Start again with a new conversation
console-start-a-new-shell = Start a new shell
console-new = New
console-continue = Continue
console-start-again-on-the-most-recent-conversation-in = Start again on the most recent conversation in this folder
console-end = End
console-end-this-session = End this session
console-hide-j-the-session-keeps-running = Hide (⌘J); the session keeps running
console-starting-the-shell = Starting the shell…
console-starting-2 = Starting { $agent }…
console-the-shell-is-not-running = The shell is not running
console-start-a-shell-in-the-project = Start a shell in the project.
console-start-shell = Start Shell
console-start-claude = Start Claude
console-start-codex = Start Codex
console-start-opencode = Start opencode
console-start-amp = Start Amp
console-start-agent = Start Agent
console-it-sees-the-editor-and-its-edits-type = It sees the editor, and its edits type themselves in, in its colour, for you to review.
console-the-files-it-changes-type-themselves-in-in = The files it changes type themselves in, in its colour, for you to review.
console-continue-last-session = Continue last session
console-install-here = Install runs its installer here in the console. If it is installed somewhere else, set its path in Settings, Agents.
console-install-here-node = Install runs its installer here in the console. It needs Node.js. If it is installed somewhere else, set its path in Settings, Agents.
console-is-not-installed = { $agent } is not installed
console-set-its-path-in-settings-agents = Set its path in Settings, Agents.
console-your-own-agent-has-no-command-yet = Your own agent has no command yet
console-name-the-command-line-agent-to-run-here = Name the command line agent to run here, and the name to show for it, in Settings, Agents.
console-agent-settings = Agent Settings
console-set-its-path = Set Its Path
console-s-developers-recommend-running-it-in-wsl = { $name }'s developers recommend running it in WSL.
console-looking-for-in-wsl = Looking for { $name } in WSL…
console-open-a-folder-in = Open a Folder in { $d }
console-is-installed-in = { $name } is installed in { $and_value }
console-open-a-folder-in-and-the-console-runs = { $makers } Open a folder in { $has } and the console runs { $name } there.
console-this-pc-has-no-wsl-distribution-yet-wsl = { $makers } This PC has no WSL distribution yet: `wsl --install` in PowerShell sets one up. To run { $name } on Windows itself instead, install it with the line below, or set its path.
console-on-windows-itself = On Windows itself
console-install-runs-its-installer-in-then-open-a = { $makers } Install runs its installer in { $first }; then open a folder there and the console runs { $name } in it. On Windows itself, it installs with the second line.
console-install-in = Install in { $d }
console-in-wsl = In WSL
console-the-installer = The installer
console-try-again = Try again
console-start-a-new-session = Start a new session
console-open-link = Open Link
console-copy-link = Copy Link
console-install-claude-code = Install Claude Code
console-install-codex = Install Codex
console-install-opencode = Install opencode
console-install-amp = Install Amp
console-install = Install
console-starts-with-the-next-session = { $agent } starts with the next session
console-draft-stopped-the-console-started-a-new-session = Draft stopped: the console started a new session. Click Draft to ask { $name }.
console-could-not-start = Could not start { $program }: { $e }
console-is-not-installed-yet-install-it-in-the = { $agent } is not installed yet. Install it in the console, then click Draft again.
console-is-not-running-2 = { $agent } is not running.
console-set-up = { $a } (set up)
console-install-2 = { $a } (install)
console-open-a-project-to-run-in-it = Open a project to run { $agent } in it.

## workspace

workspace-could-not-make = Could not make { $path }: { $e }
workspace-do-you-want-to-save-the-changes-you = Do you want to save the changes you made to { $one }?
workspace-do-you-want-to-save-the-changes-you-2 = Do you want to save the changes you made to { $count } files?
workspace-your-changes-are-lost-if-you-dont-save = Your changes are lost if you don't save them.
workspace-dont-save = Don't Save
workspace-open-a-project-to-see-its-structure = Project Settings is for a project: this folder holds no Gradle, Maven or Node build
workspace-settings-on-machine = Your settings.json is on { $machine }: open it from a window on { $machine }
workspace-these-settings-are-not-kept-in-a-file = These settings are not kept in a file
workspace-open-a-project-to-edit-its-settings = Open a project to edit its settings
workspace-the-projects-settings-are-read-again = The project's settings are read again
workspace-has-an-error-so-its-last-good-values = { $file } has an error, so its last good values stay: { $e }
workspace-reading-the-projects-folders = Reading the project's folders
workspace-reading-the-open-file-and-the-projects-folders = Reading the open file and the project's folders
workspace-reading-of-open-files-and-the-projects-folders = Reading { $n } of { $total } open files and the project's folders
workspace-nothing-changed-on-disk = Nothing changed on disk
workspace-1-file-changed-on-disk = 1 file changed on disk
workspace-files-changed-on-disk = { $n } files changed on disk
workspace-drop-a-folder-to-open-it = Drop a folder to open it

## tools

tools-problems = Problems
tools-references = References
tools-show-output = Show Output
tools-no-notifications = No notifications
tools-errors-from-git-and-the-tools-show-here = Errors from Git and the tools show here.

## state

state-modified = Modified
state-added = Added
state-deleted = Deleted
state-renamed = Renamed
state-copied = Copied
state-type-changed = Type changed
state-untracked = Untracked
state-conflicted = Conflicted
state-staged = Staged
state-partly-staged = Partly staged

## splash

splash-starting-snoutcode = Starting SnoutCode

## updates

updates-snoutcode-update = SnoutCode update
updates-updated-to-v = Updated to v{ $version }
updates-dismiss = Dismiss
updates-later = Later
updates-download = Download
updates-restart-to-install-the-update = Restart to install the update
updates-restart-to-update = Restart to update

## background

background-reading-the-gradle-model = Reading the Gradle model
background-compiling-scala = Compiling Scala
background-starting-java = Starting Java
background-indexing = Indexing
background-restarting-java = Restarting Java
background-waiting-for-snout-engine = Waiting for snout-engine
background-attempt-of-5 = {" "}(attempt { $n } of 5)
background-the-java-engine-stopped-unexpectedly-and-is-starting = The Java engine stopped unexpectedly and is starting again{ $again }: { $doing }
background-java-stopped-starting-again = Java stopped, starting again
background-building-snout-engine = Building snout-engine

## quit

quit-debugging = Debugging
quit-gradle-build = Gradle build
quit-compiling-the-scala-classes = Compiling the Scala classes
quit-claude-code-editing = Claude Code, editing { $name }
quit-the-connection-to-and-what-the-project-runs = The connection to { $host }, and what the project runs there
quit-a-file-could-not-be-saved-so-snoutcode = A file could not be saved, so SnoutCode did not quit

## terminal

terminal-custom-command = Custom command
terminal-command-prompt = Command Prompt
terminal-the-custom-external-terminal-has-no-command-set = The custom external terminal has no command: set one in Settings ▸ Tools ▸ Terminal.
terminal-could-not-run-the-external-terminal-command = Could not run the external terminal command: { $e }
terminal-could-not-open-the-external-terminal = Could not open the external terminal: { $e }
terminal-could-not-open-the-external-terminal-2 = Could not open the external terminal: { $trim }

## debug

debug-breakpoints-go-in-a-java-kotlin-scala-typescript = Breakpoints go in a Java, Kotlin, Scala, TypeScript or JavaScript file
debug-there-is-no-breakpoint-on-this-line-f8 = There is no breakpoint on this line: ⌘F8 sets one
debug-no-spring-bean-injection-or-endpoint-on-this = No Spring bean, injection or endpoint on this line
debug-run-to-cursor-works-while-the-debugger-is = Run to Cursor works while the debugger is stopped
debug-put-the-caret-on-a-line-of-code = Put the caret on a line of code
debug-nothing-to-pause-no-program-is-running-in = Nothing to pause: no program is running in the debugger
debug-nothing-ran-yet-to-run-again = Nothing ran yet to run again
debug-no-failed-tests-to-run-again = No failed tests to run again
debug-the-maven-goals-are-being-read-try-again = The Maven goals are being read; try again in a moment
debug-no-maven-goals-the-project-has-no-maven = No Maven goals: the project has no Maven build, or it is being read
debug-the-gradle-tasks-are-being-read-try-again = The Gradle tasks are being read; try again in a moment
debug-no-gradle-tasks-yet-the-projects-build-is = No Gradle tasks yet: the project's build is being read
debug-on = On
debug-any-profile-every-bean-may-be-there = Any profile: every bean may be there
debug-as-the-application-files-say = As the application files say ({ $join })
debug-the-maven-build-is-being-read-try-again = The Maven build is being read; try again in a moment
debug-stop-only-when-this-is-true-as-i = Stop only when this is true, as `i > 3`
debug-if = if { $c }
debug-remove = Remove
debug-no-breakpoints = No breakpoints
debug-click-left-of-a-line-number-or-press = Click left of a line number, or press ⌘F8 in an editor, to set one.
debug-condition-of = Condition of { $path }:{ $line }
debug-breakpoints-count = { $count ->
    [one] { $count } breakpoint
   *[other] { $count } breakpoints
    }
debug-breakpoints-disabled = { $breakpoints }, { $off } disabled
debug-untick-one-to-keep-it-without-stopping-there = Untick one to keep it without stopping there. A double click shows its line.
debug-remove-all = Remove All
debug-go-to-line = Go to Line
debug-an-expression-as-total-2 = An expression, as `total * 2`
debug-evaluate-works-while-the-program-is-stopped-at = Evaluate works while the program is stopped at a breakpoint: debug it (⌃D) and set one (⌘F8).
debug-press-to-evaluate-it-in-the-frame-the = Press ⏎ to evaluate it in the frame the debugger stopped in.
debug-add-it-to-the-watches-to-open-its = Add it to the watches (⌘⏎) to open its fields in the Debug window.
debug-evaluate-expression = Evaluate Expression
debug-java-in-the-frame-the-debugger-stopped-in = Java, in the frame the debugger stopped in.
debug-add-to-watches = Add to Watches
debug-evaluate = Evaluate
debug-could-not-connect-the-debugger = Could not connect the debugger: { $e }
debug-debugger-attached = Debugger attached
debug-the-debugger-could-not-attach = The debugger could not attach: { $e }
debug-nothing-to-reload = Nothing to reload
debug-reloaded = Reloaded { $classes }
debug-reloaded-classes = Reloaded { $n } classes
debug-could-not-reload-the-changed-classes = Could not reload the changed classes: { $e }

## lang

lang-symbols-come-from-the-java-server-which-starts = Symbols come from the Java server, which starts with a Java file
lang-file-structure-shows-the-symbols-of-a-java = File Structure shows the symbols of a Java file
lang-classes-come-from-the-java-engine-which-starts = Classes come from the Java engine, which starts with a Gradle project
lang-hierarchies-come-from-the-java-engine-in-a = Hierarchies come from the Java engine, in a Java file
lang-downloading-jdk = Downloading JDK { $major }… { $pct }%
lang-downloading-jdk-2 = Downloading JDK { $major }…
lang-jdk-could-not-be-downloaded = JDK { $major } could not be downloaded: { $error }
lang-this-project-needs-jdk-download-it = This project needs JDK { $major }. Download it?
lang-jdk-installed-too-new = This project needs JDK { $major }: its Gradle runs on Java { $max } at most, and the JDKs found here are { $installed }. Download JDK { $major }?
lang-jdk-installed-too-old = This project needs JDK { $major }, and the JDKs found here are older ({ $installed }). Download it?
lang-use-another-jdk = Use Another JDK…
lang-not-now = Not now
lang-downloading-node = Downloading Node { $prompt }… { $pct }%
lang-downloading-node-2 = Downloading Node { $prompt }…
lang-node-could-not-be-downloaded = Node { $prompt } could not be downloaded: { $error }
lang-this-project-needs-node-which-is-not-on = This project needs Node { $prompt }, which is not on this machine. Download it for SnoutCode?
lang-installing-the-packages-of-the-build-window-shows = Installing the packages of { $dir }… The Build window shows how it goes.
lang-installing-the-projects-packages-the-build-window-shows = Installing the project's packages… The Build window shows how it goes.
lang-installing-playwrights-browsers-the-build-window-shows-h = Installing Playwright's browsers… The Build window shows how it goes.
lang-the-packages-of-are-not-installed-so-it = The packages of { $dir } are not installed, so it cannot run yet.
lang-this-playwright-projects-packages-are-not-installed-so = This Playwright project's packages are not installed, so its tests cannot run yet.
lang-this-projects-packages-are-not-installed-so-it = This project's packages are not installed, so it cannot run yet.
lang-install-packages = Install packages ({ $command })
lang-playwright-needs-browsers-that-are-not-downloaded = Playwright { $version } needs browsers that are not downloaded: { $join }.
lang-install-browsers = Install browsers
lang-pom-changed = A pom.xml changed. Reload the Maven project?
lang-build-files-changed = Build files changed. Sync?
lang-gradle-files-changed = Gradle files changed. Sync?
lang-the-sync-failed = The sync failed: { $e }
lang-reload = Reload
lang-sync = Sync
lang-library-classes-open-once-the-java-engine-is = Library classes open once the Java engine is ready
lang-the-java-engine-does-not-know = The Java engine does not know { $qualified }

## record

record-works-in-a-playwright-project = { $what } works in a Playwright project
record-open-a-spec-file-and-put-the-caret = { $what }: open a spec file and put the caret where it goes
record-a-recording-is-going-stop-it-first = A recording is going: stop it first
record-it-did-not-start = It did not start
record-the-recorder-stopped-the-build-window-says-why = The recorder stopped: the Build window says why
record-the-recording-ended-with-no-steps = The recording ended with no steps
record-recorded-1-step-into = Recorded 1 step into { $unwrap_or_default }
record-recorded-steps-into = Recorded { $n } steps into { $unwrap_or_default }
record-picking-is-on-click-an-element-in-the = Picking is on: click an element in the browser
record-pick-locator-stopped = Pick Locator stopped
record-picked = Picked { $locator }
record-ui-mode-works-in-a-playwright-project = UI Mode works in a Playwright project
record-ui-mode-opens-in-your-browser = UI Mode opens in your browser

## remote

remote-could-not-connect-to = Could not connect to { $name }: { $why }
remote-trust-this-host = Trust This Host?
remote-key-passphrase = Key Passphrase
remote-no = No
remote-yes = Yes
remote-password = Password:
remote-user-host-a-host-of-your-ssh-config = user@host, a Host of your ssh config, or WSL: Ubuntu
remote-user-host-or-a-host-of-your-ssh = user@host, or a Host of your ssh config
remote-recent = Recent
remote-from-your-ssh-config = From your ssh config
remote-connecting-to = Connecting to { $h }…
remote-connecting-to-2 = Connecting to { $host }…
remote-the-first-time-takes-a-little-longer-snoutcode = The first time takes a little longer: SnoutCode installs its server there.
remote-no-hosts-in-your-ssh-config-type-one = No hosts in your ssh config: type one.
remote-connect-to-ssh-host-or-wsl = Connect to SSH Host or WSL
remote-connect-to-ssh-host = Connect to SSH Host
remote-snoutcode-installs-its-server-on-the-host-the = SnoutCode installs its server on the host the first time (Linux, x86_64 or aarch64).
remote-host = Host
remote-connect = Connect
remote-on = { $path } on { $host }
remote-connected-to = Connected to { $name }
remote-reconnecting-to = Reconnecting to { $name }…
remote-reconnecting-to-attempt = Reconnecting to { $name }… (attempt { $attempt })
remote-the-connection-to-is-lost = The connection to { $name } is lost
remote-disconnected-from = Disconnected from { $name }
remote-the-linux-server-builds-are-missing-run-scripts = The Linux server builds are missing: run scripts/build-server-linux.sh
remote-connecting-to-3 = Connecting to { $name }
remote-connecting = Connecting
remote-installing-snout-server = Installing snout-server
remote-java-engine-on = Java engine on { $name }
remote-looking-for-the-engine = Looking for the engine
remote-could-not-install-it = Could not install it: { $e }
remote-open-folder-on = Open a Folder on { $name }
remote-no-folders-here = No folders here
remote-show-hidden-folders = Show hidden folders

## run

run-is-not-among-the-projects-sources = { $class } is not among the project's sources
run-the-file-in-front-has-no-tests-main = The file in front has no tests, main class or script to run
run-nothing-to-run-here-put-the-caret-in = Nothing to run here: put the caret in a main class, a test or a script
run-root = { $name } (root)
run-reading-the-gradle-tasks = Reading the Gradle tasks
run-the-builds-are-being-configured = The builds are being configured.
run-the-tasks-could-not-be-read = The tasks could not be read
run-no-tasks-yet = No tasks yet
run-the-tasks-appear-once-the-java-engine-has = The tasks appear once the Java engine has imported the project.
run-no-gradle-tasks = No Gradle tasks
run-this-project-has-no-gradle-build = This project has no Gradle build.
run-maven-wrapper = Maven { $version } (wrapper)
run-lifecycle = Lifecycle
run-plugins = Plugins
run-delete-the-build-output-target = Delete the build output (target)
run-check-the-project-is-correct-and-complete = Check the project is correct and complete
run-compile-the-main-sources = Compile the main sources
run-run-the-unit-tests = Run the unit tests
run-package-the-compiled-code-a-jar-or-war = Package the compiled code (a jar or war)
run-run-the-checks-on-the-package-integration-tests = Run the checks on the package, integration tests among them
run-put-the-package-in-the-local-repository = Put the package in the local repository
run-make-the-projects-site = Make the project's site
run-copy-the-package-to-the-remote-repository = Copy the package to the remote repository
run-reading-the-maven-goals = Reading the Maven goals
run-the-plugins-goals-are-being-read = The plugins' goals are being read.
run-the-goals-could-not-be-read = The goals could not be read
run-no-goals-yet = No goals yet
run-the-goals-appear-once-the-java-engine-has = The goals appear once the Java engine has imported the project.
run-no-maven-builds = No Maven builds
run-this-project-has-no-pom-xml = This project has no pom.xml.
run-offline = Offline
run-run-maven-offline-o-only-the-local-repository = Run Maven offline (-o): only the local repository
run-skip-tests = Skip Tests
run-run-goals-without-the-tests-dskiptests = Run goals without the tests (-DskipTests)
run-active-in-the-pom = active in the POM
run-failed = { $label } (failed)
run-paused-running = Paused, running { $time }
run-running = Running { $time }
run-finished-in = Finished in { $time }
run-failed-after = Failed after { $time }
run-stopped-after = Stopped after { $time }
run-tests-count = { $count ->
    [one] { $parts }, { $count } test
   *[other] { $parts }, { $count } tests
    }
run-the-java-engine-is-not-ready-yet = The Java engine is not ready yet
run-the-java-engine-stopped-and-this-run-with = The Java engine stopped, and this run with it.
run-could-not-save-the-output-to = Could not save the output to { $path }: { $e }
run-open-a-project-to-run-it = Open a project to run it.
run-has-no-gradle-or-maven-build-runs-tests = { $project } has no Gradle or Maven build: runs, tests and the debugger need one.
run-java-support-is-off = Java support is off.
run-the-java-engine-is-not-running = The Java engine is not running: { $why }
run-starting = Starting { $c }
run-running-2 = Running { $t }
run-running-3 = Running { $config }
run-testing = Testing { $label }
run-running-4 = Running { $task }
run-running-mvn-in = Running mvn { $goals } in { $module }
run-attaching-to-the-node-listening-on-port = Attaching to the Node listening on port { $port }
run-waiting-for-the-java-engine-the-run-starts = Waiting for the Java engine ({ $doing }): the run starts as soon as it is ready.
run-working-directory = Working directory: { $cwd }
run-the-java-engine-is-ready = The Java engine is ready.
run-working-directory-2 = Working directory: { $dir }
run-finished = Finished
run-failed-3 = Failed
run-stopped = Stopped
run-with-exit-code = {" "}with exit code { $code }
run-the-scala-compile-failed-see-problems = The Scala compile failed: see Problems
run-compiling-the-changed-scala = Compiling the changed Scala
run-reading-the-tasks = Reading the tasks
run-the-engine-does-not-know-this-files-module = The engine does not know this file's module yet
run-downloading-the-javascript-debugger-js-debug = Downloading the JavaScript debugger (js-debug { $version })
run-the-debugger-could-not-start-the-program = The debugger could not start the program: { $e }
run-detached = Detached
run-could-not-connect-the-debugger-to-a-node = Could not connect the debugger to a Node process: { $e }
run-npm-is-missing-beside-node = npm is missing beside Node
run-is-not-installed-the-projects-lockfile-is-s = { $name } is not installed (the project's lockfile is { $name }'s)
run-attach-to-port = Attach to port { $port }
run-the-configuration-names-no-script = The configuration names no script
run-the-configuration-names-no-file-to-run = The configuration names no file to run
run-is-not-there = { $file } is not there
run-open-a-project-first = Open a project first
run-no-node-fits-this-project-download-it-from = No Node fits this project: download it from the bar above the editor first.
run-did-not-start = { $program } did not start: { $e }
run-process-finished-with-exit-code = Process finished with exit code { $c }
run-process-ended-by-a-signal = Process ended by a signal
run-all-tests = All tests
run-tests-in = Tests in { $p }
run-open-a-playwright-project-first = Open a Playwright project first
run-passed = { $passed } passed
run-skipped = { $skipped } skipped
run-failed-2 = { $failed } failed
tests-watch-label = Watch

## structure

structure-included-build = included build
structure-compile = Compile
structure-runtime = Runtime
structure-test = Test
structure-annotation-processor = Annotation processor
structure-what-the-main-code-compiles-against-compile-provided = What the main code compiles against (compile, provided and system scopes).
structure-what-the-main-code-runs-with-compile-and = What the main code runs with (compile and runtime scopes).
structure-what-the-tests-compile-and-run-with-every = What the tests compile and run with (every scope).
structure-what-the-main-code-compiles-against-compileclasspath = What the main code compiles against (compileClasspath).
structure-what-the-main-code-runs-with-runtimeclasspath = What the main code runs with (runtimeClasspath).
structure-what-the-tests-compile-and-run-with-the = What the tests compile and run with (the test source set's runtimeClasspath).
structure-the-annotation-processors-javac-runs-on-the-main = The annotation processors javac runs on the main code.
structure-project = project { $path }
structure-the-gradle-model-could-not-be-read = The Gradle model could not be read: { $e }
structure-the-gradle-model-is-not-json = The Gradle model is not JSON: { $e }
structure-the-gradle-model-is-of-an-older-format = The Gradle model is of an older format: sync again
structure-edit-in-build-file = Edit in build file
structure-sync-gradle = Sync Gradle
structure-the-pages-fill-in-as-soon-as-the = The pages fill in as soon as the last sync's model is read.
structure-no-gradle-model-yet = No Gradle model yet
structure-this-project-has-not-been-synced-as-a = This project has not been synced as a Gradle build. Sync it to see its modules and dependencies.
structure-the-gradle-model-could-not-be-read-2 = The Gradle model could not be read
structure-java = Java
structure-looking-for-jdks = Looking for JDKs…
structure-not-set-the-jdks-own = Not set (the JDK's own)
structure-java-to-per-module = Java { $lo } to { $hi }, per module
structure-language-level = Language level
structure-gradle-and-maven = Gradle and Maven
structure-unknown = Unknown
structure-did-not-sync = Did not sync
structure-maven-version = Maven version
structure-gradle-version = Gradle version
structure-maven-jvm = Maven JVM
structure-gradle-jvm = Gradle JVM
structure-not-known-until-the-build-syncs = Not known until the build syncs
structure-the-jdk-maven-runs-on-and-compiles-with = The JDK Maven runs on and compiles with, chosen for the build's Java level: the JDK above when one is set.
structure-the-jvm-the-last-sync-ran-gradle-on = The JVM the last sync ran Gradle on: the JDK above when one is set.
structure-included-build-2 = Included build
structure-spring-boot = Spring Boot
structure-build-file = Build file
structure-not-set = Not set
structure-toolchain = toolchain { $t }
structure-target-compatibility = Target compatibility
structure-this-project-compiles-no-java-it-has-no = This project compiles no Java: it has no source sets.
structure-java-bytecode = Java { $a }, bytecode { $b }
structure-content-roots = Content roots
structure-not-there-yet = not there yet
structure-output-folders = Output folders
structure-resources = Resources
structure-classes = Classes
structure-no-module-compiles-java = No module compiles Java
structure-dependencies-show-for-the-projects-that-apply-a = Dependencies show for the projects that apply a Java plugin.
structure-conflicts-only = Conflicts only
structure-no-scope-dependencies = No dependencies: { $scope }
structure-declares-nothing-for-this-scope = { $m } declares nothing for this scope. { $m_2 }
structure-declared-in-all = { $declared } declared, { $count } in all.
structure-version-conflicts = { $count ->
    [one] { $count } version conflict
   *[other] { $count } version conflicts
    }
structure-unresolved = { $unresolved_count } unresolved
structure-no-version-lost-a-conflict-here = No version lost a conflict here.
structure-try-fewer-letters-of-the-group-name-or = Try fewer letters of the group, name or version.
structure-on = on { $h }
structure-on-machine = on { $machine }
structure-download-jdk = Download JDK
structure-found-where-jdks-are-installed-and-added-by = Found where JDKs are installed { $machine }, and added by you.
structure-add-from-folder = Add from Folder…
structure-downloading-jdk = Downloading JDK { $major }
structure-no-jdk-found = No JDK found
structure-no-jdk-is-installed-where-jdks-usually-are = No JDK is installed { $machine } where JDKs usually are. Download one, or add the folder of one.
structure-downloaded = Downloaded
structure-detected = Detected
structure-this-projects = This project's
structure-remove-from-the-list = Remove from the list
structure-no-jdk-there-now = No JDK there now
structure-removing-an-added-jdk-takes-it-off-the = Removing an added JDK takes it off the list; its files stay where they are.
structure-selected-jdk = Selected JDK
structure-version = Version
structure-home = Home
structure-kept-in = Kept in
structure-your-settings = Your settings
structure-found-by-snoutcode = Found by SnoutCode
structure-this-project = This project
structure-runs-on-it = Runs on it
structure-use-for-this-project = Use for This Project
structure-not-detected = Not detected
structure-spring-settings = Spring Settings
structure-none-with-the-spring-boot-plugin = None with the Spring Boot plugin
structure-in-the-spring-model = { $beans } in the Spring model
structure-read-once-the-java-engine-is-ready = Read once the Java engine is ready
structure-profiles-known = Profiles known
structure-configuration-files = Configuration files
structure-no-application-properties-or-application-yml-in-the = No application.properties or application.yml in the resources
structure-no-spring-here = No Spring here
structure-no-module-applies-the-spring-boot-plugin-and = No module applies the Spring Boot plugin, and the Spring model has no beans.
structure-not-found = Not found
structure-claude-code-settings = Claude Code Settings
structure-claude-was-not-found-on-path-or-the = claude was not found on PATH or the usual places
structure-ide-connection = IDE connection
structure-no-session-connected-a-claude-started-in-the = No session connected. A claude started in the console connects by itself.
structure-1-session-connected-it-sees-the-open-files = 1 session connected: it sees the open files, selection and problems
structure-sessions-connected = { $n } sessions connected
structure-the-project-as-a-whole-the-jdk-it = The project as a whole: the JDK it runs on is yours to pick, the rest is Maven's.
structure-every-maven-module-of-every-build-with-its = Every Maven module of every build, with its source sets, roots and outputs.
structure-what-each-module-declares-and-what-that-brings = What each module declares and what that brings in, as Maven resolved it.
structure-the-project-as-a-whole-the-jdk-it-2 = The project as a whole: the JDK it runs on is yours to pick, the rest is Gradle's.
structure-every-gradle-project-of-every-build-with-its = Every Gradle project of every build, with its source sets, roots and outputs.
structure-what-each-module-declares-and-what-that-brings-2 = What each module declares and what that brings in, as Gradle resolved it.
structure-the-jdks-snoutcode-can-run-gradle-and-the = The JDKs SnoutCode can run Gradle and the Java engine on.
structure-frameworks-and-tools-as-they-stand-read-only = Frameworks and tools, as they stand. Read only.
structure-unresolved-2 = Unresolved
structure-show-in-modules = Show { $name } in Modules
structure-show-in-build-file = Show in build file
structure-show-the-declaration-that-brings-it-in = Show the declaration that brings it in
structure-show-its-dependencies = Show its dependencies
structure-a-newer-version-asked-for-elsewhere-won = a newer version asked for elsewhere won
structure-dependency-management-chose-it = dependency management chose it
structure-it-is-forced = it is forced
structure-a-constraint-asks-for-it = a constraint asks for it
structure-another-request-won = another request won
structure-was-asked-for-here-is-used = { $asked } was asked for here; { $version } is used: { $why }
structure-conflict = Conflict
structure-won-over = won over { $join }
structure-show-in-version-catalog = Show in version catalog
structure-modules-from-sync = { $count ->
    [one] { $name } · { $count } module from the last { $tool } sync
   *[other] { $name } · { $count } modules from the last { $tool } sync
    }
structure-project-structure = Project Settings
structure-syncing = Syncing…
structure-reload-maven = Reload Maven
structure-read-the-maven-builds-again = Read the Maven builds again
structure-read-the-gradle-builds-again = Read the Gradle builds again
structure-read-the-builds-again = Read the builds again
structure-facets = Facets
structure-project-settings = Project Settings
structure-platform-settings = Platform Settings
structure-search = Search
structure-filter-dependencies = Filter by group, name or version
structure-jdk-listed-already = JDK { $version } is listed already
structure-jdk-added = JDK { $version } added. Apply keeps it.
structure-no-jdk-in = No JDK in { $dir }: pick the folder holding the bin folder with Java in it
structure-add-jdk = Add JDK
structure-jdk-removed = Removed from the list. Apply keeps the change.
structure-jdk-downloaded = JDK { $version } downloaded to { $home }
structure-download-failed = The download failed: { $e }
structure-build-file-not-found = The build file could not be found
structure-syncing-note = Syncing the Gradle model. The pages follow when it is done.

## newproject

newproject-gradle-or-maven = Gradle or Maven
newproject-from-start-spring-io = From start.spring.io
newproject-end-to-end-tests-in-typescript = End-to-end tests in TypeScript
newproject-project-name = Project name
newproject-folder = Folder
newproject-search-dependencies = Search dependencies
newproject-waiting-for-start-spring-io = Waiting for start.spring.io…
newproject-start-spring-io-could-not-be-reached = start.spring.io could not be reached ({ $e }).
newproject-the-project-goes-in = The project goes in { $target }
newproject-asking-start-spring-io-what-it-offers = Asking start.spring.io what it offers…
newproject-application-url = Application URL
newproject-the-address-the-tests-open-a-base-url = The address the tests open. A BASE_URL environment variable replaces it when it is set.
newproject-create-a-git-repository = Create a Git repository
newproject-build-tool = Build tool
newproject-coordinates = Coordinates
newproject-none-chosen-yet = None chosen yet.
newproject-no-dependency-matches = No dependency matches.
newproject-dependencies = Dependencies
newproject-spring-boot-java-choose-what-the-application-uses = Spring Boot { $version }, Java { $java }. Choose what the application uses; more can be added to the build later.
newproject-getting-the-project-from-start-spring-io = Getting the project from start.spring.io…
newproject-creating-the-project = Creating the project…
newproject-new-project = New Project
newproject-a-java-or-spring-boot-project-built-with = A Java or Spring Boot project, built with Gradle or Maven.
newproject-create-without-the-network = Create Without the Network
newproject-previous = Previous
newproject-next = Next
newproject-already-exists-and-is-not-empty = { $dir } already exists and is not empty.
newproject-give-the-project-a-name = Give the project a name.
newproject-the-name-cannot-start-with-a-dot-or = The name cannot start with a dot or hold / : or \.
newproject-choose-where-the-project-goes = Choose where the project goes.
newproject-the-group-is-a-dotted-name-such-as = The group is a dotted name, such as com.example.
newproject-the-artifact-holds-letters-digits-dots-dashes-and = The artifact holds letters, digits, dots, dashes and underscores.
newproject-the-package-is-a-dotted-java-name-such = The package is a dotted Java name, such as com.example.demo.
newproject-the-applications-address-starts-with-http-or-https = The application's address starts with http:// or https://.

## runwin

runwin-evaluate-an-expression = Evaluate an expression
runwin-clear-all = Clear All
runwin-copy-all = Copy All
runwin-fix-with = Fix with { $agent }
runwin-save-output-to-file = Save Output to File…
runwin-rerun-r = Rerun (⌘R)
runwin-rerun-failed-tests-r = Rerun Failed Tests (⌥⌘R)
runwin-test-results = Test Results
runwin-passed = Passed
runwin-skipped = Skipped
runwin-nothing-printed = { $what }; nothing printed
runwin-expected = Expected
runwin-actual = Actual
runwin-errors-warnings = { $e } errors, { $w } warnings
runwin-up-to-date = up to date
runwin-from-cache = from cache
runwin-no-source = no source
runwin-debugger = Debugger
runwin-resume-f9 = Resume (F9)
runwin-pause-program-f9 = Pause Program (⌥⌘F9)
runwin-step-over-f8 = Step Over (F8)
runwin-step-into-f7 = Step Into (F7)
runwin-step-out-f8 = Step Out (⇧F8)
runwin-the-program-ended = The program ended.
runwin-waiting-for-the-program-to-start = Waiting for the program to start…
runwin-the-program-is-running-breakpoints-stop-it-here = The program is running. Breakpoints stop it here.
runwin-add-to-watches = Add to Watches (⇧↩)
runwin-nothing-running = Nothing running
runwin-run-a-configuration-from-the-title-bar-or = Run a configuration from the title bar, or a test from the gutter.
runwin-no-debug-session = No debug session
runwin-debug-a-configuration-from-the-title-bar-or = Debug a configuration from the title bar, or a test from the gutter.
runwin-no-build-output = No build output
runwin-build-the-project-or-run-one-of-its = Build the project, or run one of its Gradle tasks or Maven goals.
runwin-run-task = Run Task…

## spring

spring-any-profile = Any profile
spring-from-the-files = { $config_active } (from the files)
spring-text = { $name } ({ $from })
spring-consumes = {"\u000A"}Consumes { $join }
spring-produces = {"\u000A"}Produces { $join }
spring-bean-not-active-with-the-active-profiles = Bean '{ $b }' ({ $stereotype_label }): not active with the active profiles
spring-not-injected-anywhere = not injected anywhere
spring-injected-in-1-place = injected in 1 place
spring-injected-in-places = injected in { $n } places
spring-bean = Bean '{ $b }' ({ $stereotype_label }), { $used }
spring-text-2 = { $what }: { $value }
spring-bean-2 = { $what }: bean '{ $one }'
spring-no-bean = { $what }: no bean
spring-beans = { $what }: { $count } beans
spring-all-sources = All sources
spring-default-package = (default package)
spring-search-beans-by-name-or-type = Search beans by name or type
spring-search-paths-and-handlers = Search paths and handlers
spring-all-stereotypes = All stereotypes
spring-no-profile = No profile
spring-profile = Profile { $p }
spring-inactive = { $inactive } inactive
spring-by-package = By package
spring-by-stereotype = By stereotype
spring-no-beans = No beans
spring-the-project-declares-no-spring-bean-yet = The project declares no Spring bean yet.
spring-no-bean-matches = No bean matches
spring-try-fewer-letters-or-another-stereotype-or-source = Try fewer letters, or another stereotype or source.
spring-beans-from-libraries = { $count } beans, { $library } from libraries
spring-not-injected-anywhere-2 = Not injected anywhere
spring-injected-in-1-place-2 = Injected in 1 place
spring-injected-in-places-2 = Injected in { $n } places
spring-profiles = {"\u000A"}Profiles: { $join }
spring-not-active-note = {"\u000A"}Not active: the active profiles leave it out
spring-primary = Primary
spring-of-beans = { $shown } of { $bean_count } beans
spring-method-all = All
spring-method = Method
spring-path = Path
spring-handler = Handler
spring-no-endpoints = No endpoints
spring-no-controller-of-the-project-maps-a-request = No controller of the project maps a request yet.
spring-no-endpoint-matches = No endpoint matches
spring-try-fewer-letters-of-the-path-or-the = Try fewer letters of the path or the method.
spring-of-endpoints = { $shown } of { $total } endpoints
spring-as-runs = As '{ $c }' runs ({ $join })
spring-read-the-spring-model = Read the Spring Model
spring-reading-the-spring-model = Reading the Spring model
spring-the-beans-and-endpoints-are-being-read = The beans and endpoints are being read.
spring-no-spring-model-yet = No Spring model yet
spring-beans-and-endpoints-appear-once-the-java-engine = Beans and endpoints appear once the Java engine has imported the project.
spring-beans-2 = Beans { $bean_count }
spring-endpoints = Endpoints { $count }
spring-reading = Reading…
spring-data-repository = Data repository

## config

config-arguments-the-program-gets = Arguments the program gets
config-9229-empty-run-the-file = 9229 (empty: run the file)
config-unnamed = Unnamed { $next }
config-no-configuration-add-one-with = No configuration. Add one with +.
config-choose-a-module = Choose a module
config-kind = Kind
config-main-class = Main class
config-goals = Goals
config-script = Script
config-file = File
config-task = Task
config-gradle-arguments = Gradle arguments
config-maven-arguments = Maven arguments
config-playwright-arguments = Playwright arguments
config-script-arguments = Script arguments
config-program-arguments = Program arguments
config-node-options = Node options
config-vm-options = VM options
config-environment-variables = Environment variables
config-attach-to-port = Attach to port
config-from-the-build-with-your-changes-remove-puts = From the build, with your changes. Remove puts it back as the build has it.
config-from-the-build-a-change-you-make-stands = From the build. A change you make stands in for it.
config-your-own-configuration = Your own configuration.
config-run-configurations = Run Configurations
config-what-run-and-debug-start-an-application-a = What Run and Debug start: an application, a Gradle task, tests, a script.
config-add-configuration = Add Configuration
config-remove-or-put-back-as-the-build-has = Remove, or Put Back as the Build Has It
config-application = Application
config-package-script = Package script
config-scripts = Scripts

## tests

tests-filter-tests = Filter tests
tests-not-run-yet = Not run yet
tests-did-not-start = Did not start
tests-workers = { $count ->
    [one] { $projects } · { $count } worker
   *[other] { $projects } · { $count } workers
    }
tests-about-left = { $elapsed } · about { $left } left
tests-live-run = Live run
tests-history = History
tests-run-with-the-browsers-showing-one-test-at = Run with the browsers showing, one test at a time
tests-rerun-failed = Rerun failed
tests-run-all = Run all
tests-no-environment = No environment
tests-environment = Environment: { $label }, { $d }
tests-environment-2 = Environment: { $label }
tests-no-environment-the-configurations-own = No environment (the configuration's own)
tests-all = All { $total }
tests-failed = Failed { $failed }
tests-flaky = Flaky { $flaky }
tests-slow-over-4-s = Slow, over 4 s
tests-last-10-runs = Last 10 runs
tests-flaky-2 = Flaky
tests-finding-the-tests = Finding the tests…
tests-no-test-matches-the-filter = No test matches the filter.
tests-no-tests-yet-run-them-or-add-a = No tests yet. Run them, or add a spec file.
tests-failed-in = Failed in { $project }
tests-flaky-in = Flaky in { $project }
tests-in = { $other } in { $project }
tests-retries = { $count ->
    [one]  · { $count } retry
   *[other]  · { $count } retries
    }
tests-screenshot-comparison = Screenshot comparison
tests-difference = Difference
tests-accept-new-screenshot = Accept new screenshot
tests-runs-the-test-again-keeping-its-screenshots-as = Runs the test again, keeping its screenshots as they are now
tests-steps = Steps
tests-from-the-trace = From the trace
tests-console-error = console error
tests-actions-no-failed-requests-or-console-errors = { $count } actions; no failed requests or console errors.
tests-the-trace-could-not-be-read = The trace could not be read: { $why }
tests-reading-the-trace = Reading the trace…
tests-open-full-trace = Open full trace
tests-video = Video
tests-run-this-test = Run this test
tests-debug-this-test = Debug this test
tests-debug-in-inspector = Debug in Inspector
tests-show-in-editor = Show in editor
tests-copy-error = Copy error
tests-when = When
tests-what = What
tests-branch = Branch
tests-time = Time
tests-new-since-the-run-before = New since the run before
tests-runs-of-this-project = Runs of this project
tests-kept-on-this-machine-outside-the-repository = Kept on this machine, outside the repository
tests-no-runs-kept-yet = No runs kept yet.
tests-min-ago = { $secs } min ago
tests-h-ago = { $secs } h ago
tests-days-ago = { $days } days ago
tests-the-playwright-test-runner-is-not-installed = The Playwright test runner is not installed.
tests-no-node-fits-this-project = No Node fits this project.
tests-trace-viewer = Trace viewer
tests-no-tests-found = No tests found
tests-open-a-playwright-project-or-a-project-whose = Open a Playwright project, or a project whose packages test with Vitest or Jest, to run its tests here.
tests-in-2 = { $runner } in { $rel }
tests-is-not-installed-install-the-packages-first = { $runner } is not installed: install the packages first.
tests-the-snoutcode-reporter-could-not-be-written = The SnoutCode reporter could not be written: { $e }
tests-tests = { $count } tests
tests-failed-tests = Failed tests
tests-the-playwright-test-runner-is-not-installed-install = The Playwright test runner is not installed: install the packages first.
tests-playwright-listed-no-tests = Playwright listed no tests ({ $status })
tests-recording = Recording
tests-ui-mode = UI Mode
tests-the-snoutcode-locator-picker-could-not-be-written = The SnoutCode locator picker could not be written: { $e }
tests-new-since-the-last-run = {" "}({ $fresh } new since the last run)
tests-passed-failed = { $title }: { $passed } passed, { $failed } failed{ $new }{ $new_2 }
tests-count-passed = { $count ->
    [one] { $count } passed
   *[other] { $count } passed
    }
tests-count-failed = { $count ->
    [one] { $count } failed
   *[other] { $count } failed
    }
tests-count-flaky = { $count ->
    [one] { $count } flaky
   *[other] { $count } flaky
    }
tests-count-skipped = { $count ->
    [one] { $count } skipped
   *[other] { $count } skipped
    }
tests-flaky-3 = , { $flaky } flaky

## env

env-environment = Environment { $count }
env-no-environment-add-one-with = No environment. Add one with +.
env-variables = Variables
env-in-use = in use
env-where-the-tests-point-the-applications-url-and = Where the tests point: the application's URL and the variables they read.
env-add-environment = Add Environment
env-remove-environment = Remove Environment

## pwsetup

pwsetup-ffmpeg-for-videos = FFmpeg (for videos)
pwsetup-playwright-not-installed = Playwright { $v }, not installed
pwsetup-no-node-fits = no Node fits
pwsetup-install-packages = Install packages
pwsetup-install-playwright-browsers = Install Playwright browsers
pwsetup-no-node-fits-this-project-download-it-from = No Node fits this project: download it from the bar above first.
pwsetup-the-install-did-not-finish-see-the-build = The install did not finish: see the Build window.

## pwreport

pwreport-queued = Queued
pwreport-flaky-passed-on-a-retry = Flaky, passed on a retry
pwreport-interrupted = Interrupted

## processes

processes-snoutcode-the-window-the-editors-and-git = SnoutCode: the window, the editors and Git
processes-idle-for-min-stops-after = Idle for { $m } min, stops after { $limit }
processes-idle-stops-after-min = Idle, stops after { $limit } min
processes-java-engine-for-reads-the-gradle-or-maven = Java engine for { $project }: reads the Gradle or Maven build, runs and tests the code
processes-java-language-server-for-completion-errors-navigation = Java language server for { $project }: completion, errors, navigation
processes-maven-for-runs = Maven for { $project }: runs { $title }
processes-maven-for-runs-goals = Maven for { $project }: runs goals
processes-tests-of-the-test-jvm-maven-started = Tests of { $title }: the test JVM Maven started
processes-tests-for-the-test-jvm-maven-started = Tests for { $project }: the test JVM Maven started
processes-your-program-started-by-maven = { $title }: your program, started by Maven
processes-program-maven-started-for = Program Maven started for { $project }
processes-your-program-being-debugged = { $title }: your program, being debugged
processes-your-program-started-with-run = { $title }: your program, started with Run
processes-program-started-by-the-java-engine-for = Program started by the Java engine for { $project }
processes-gradle-daemon-for-syncs-the-build-runs-tasks = Gradle daemon { $version } for { $project }: syncs the build, runs tasks and tests
processes-tests-of-the-test-jvm-gradle-started = Tests of { $title }: the test JVM Gradle started
processes-tests-for-the-test-jvm-gradle-started = Tests for { $project }: the test JVM Gradle started
processes-gradle-worker-for-compiles-or-processes-in-a = Gradle worker for { $project }: compiles or processes in a JVM of its own
processes-your-program-started-by-gradle = { $title }: your program, started by Gradle
processes-program-gradle-started-for = Program Gradle started for { $project }
processes-in = {" "}in { $p }
processes-java-engine-for-stopping-with-its-language-server = Java engine for { $folder }: stopping, with its language server and Gradle daemons
processes-terminal-your-shell = Terminal: your { $name } shell{ $within }
processes-claude-code-the-assistant-in-the-console = Claude Code: the assistant in the console
processes-started-by-snoutcode = { $name }: started by SnoutCode
processes-java-engine-for-left-over-from-an-earlier = Java engine for { $folder }, left over from an earlier session
processes-java-language-server-left-over-from-an-earlier = Java language server, left over from an earlier session
processes-gradle-daemon-left-over-from-an-earlier-session = Gradle daemon { $version }, left over from an earlier session
processes-nothing-idle-to-stop = Nothing idle to stop.
processes-stopped-1-idle-process = Stopped 1 idle process.
processes-stopped-idle-processes = Stopped { $n } idle processes.
processes-state-building = Building
processes-state-stopping = Stopping
processes-state-left-over = Left over
processes-looking-at-the-processes = Looking at the processes
processes-snoutcodes-own-processes-show-here-the-java-engine = SnoutCode's own processes show here: the Java engine, Gradle daemons, Maven, runs and terminals.
processes-process = Process
processes-memory = Memory
processes-uptime = Uptime
processes-project = { $purpose }{"\u000A"}Project: { $project }
processes-busy-at-work-now-a-filled-dot = Busy: at work now (a filled dot)
processes-waiting-alive-doing-nothing-now-a-hollow-dot = Waiting: alive, doing nothing now (a hollow dot)
processes-summary = { $count ->
    [one] { $count } process, { $memory } of memory, { $cpu }% CPU
   *[other] { $count } processes, { $memory } of memory, { $cpu }% CPU
    }
processes-idle = { $idle } idle

## editor

editor-tab = Tab, { $width }
editor-spaces = { $width } spaces
editor-could-not-read = Could not read { $path }: { $e }
editor-could-not-save = Could not save { $path }: { $e }
editor-this-file-changed-on-disk-and-this-tab = This file changed on disk, and this tab has unsaved changes.
editor-reload-from-disk = Reload from disk
editor-keep-my-changes = Keep my changes
editor-run = Run '{ $name }'
editor-debug = Debug '{ $name }'
editor-do-you-want-to-save-the-changes-you = Do you want to save the changes you made to { $name }?
editor-a-librarys-class-file-has-no-git-history = A library's class file has no Git history.
editor-not-a-git-project-no-git-repository-holds = Not a Git project: no Git repository holds this file.
editor-not-committed-yet = Not committed yet
editor-uncommitted = Uncommitted
editor-reading-history = Reading history…
editor-close-annotations = Close Annotations
editor-breakpoint-condition = Breakpoint Condition…
editor-show-diff = Show Diff
editor-show-in-git-log = Show in Git Log
editor-copy-hash = Copy Hash
editor-copied = Copied { $count }
editor-annotate-previous-revision = Annotate Previous Revision
editor-the-file-as-it-was-before-this-commit = The file as it was before this commit
editor-this-commit-brought-the-file-in = This commit brought the file in
editor-you-changed-this-line-since-the-last-commit = You changed this line since the last commit. Commit it and the column says so.
editor-authors-of = Authors of { $decl }
editor-committed-lines = { $total } committed lines
editor-last = { $email } · last { $now }
editor-1-line = 1 line
editor-lines = { $lines } lines
editor-before = before { $short_id }
editor-now = Now
editor-back = Back
editor-back-to-now = Back to Now
editor-you-not-committed-yet = You, not committed yet
editor-1-line-removed = 1 line removed
editor-lines-removed = { $n } lines removed
editor-writer-editing-line = { $writer } is editing line { $line } { $arrow }
editor-writer-edited-line = { $writer } edited line { $line } { $arrow }
editor-claude-code-is-writing-a-change = Claude Code is writing a change…
editor-claude-code-proposes-a-new-file = Claude Code proposes a new file
editor-claude-code-proposes-a-change = Claude Code proposes a change
editor-more = { $waiting } more
editor-reject = Reject
editor-accept = Accept
editor-claudes-changes-all-reviewed = Claude's changes, all reviewed
editor-claude-changed-this-file-1-change-to-review = Claude changed this file: 1 change to review
editor-claude-changed-this-file-changes-to-review = Claude changed this file: { $n } changes to review
editor-next = ⌃⌥⇧↓ next
editor-revert-file = Revert File
editor-accept-file = Accept File
editor-hide-the-inline-changes = Hide the inline changes
editor-changed-by-at = Changed by { $short } at { $t }
editor-changed-by = Changed by { $short }
editor-lines-taken-out-here = Lines taken out here
editor-line = Line { $start }
editor-lines-2 = Lines { $start }–{ $end }
editor-reviewed = Reviewed
editor-revert = Revert
editor-ask-to-fix = Ask { $agent } to fix
editor-context-actions = Context Actions
editor-looking-for-fixes = Looking for fixes…
editor-the-symbol = the symbol
editor-rename-to-enter-renames-everywhere = Rename to (Enter renames everywhere)
editor-go-to-declaration-needs-the-java-engine-for = Go to Declaration needs the Java engine for this file.
editor-no-declaration-found-here = No declaration found here.
editor-usages-of = Usages of { $word }
editor-no-usages-of-found = No usages of { $word } found.
editor-implementations-of = Implementations of { $word }
editor-no-implementations-of-found = No implementations of { $word } found.
editor-this-needs-the-java-engine-for-this-file = This needs the Java engine for this file.
editor-found = { $count } found
editor-group-in-the-local-repository = group in the local repository
editor-artifact-in-the-local-repository = artifact in the local repository
editor-version-in-the-local-repository = version in the local repository
editor-ask-to-fix-2 = Ask { $short } to fix
editor-more-actions = More actions…
editor-the-file-changed-meanwhile-try-fix-all-again = The file changed meanwhile: try Fix all again
editor-none-of-these-problems-could-be-fixed-this = None of these problems could be fixed this way
editor-fixed-of-problems-the-rest-need-a-look = Fixed { $n } of { $count } problems; the rest need a look
editor-no-problem-at-the-caret = No problem at the caret
editor-no-quick-fix-for-this-problem-for-more = No quick fix for this problem: { $more } for more actions
editor-no-actions-here = No actions here
editor-recording-but-the-recorded-steps-were-edited-here = Recording, but the recorded steps were edited here: the rest stays in the recorder's window.
editor-no-steps-yet = no steps yet
editor-1-step = 1 step
editor-steps = { $n } steps
editor-recording-into-this-file-act-in-the-browser = Recording into this file: { $n }. Act in the browser, then Stop or close it.
editor-picking-locators-click-an-element-in-the-browser = Picking locators: click an element in the browser to put its locator at the caret.
editor-picking-locators-picked-click-another-or-stop = Picking locators: { $picked } picked. Click another, or Stop.
editor-cannot-extract-a-method-here-select-whole-statements = Cannot extract a method here: select whole statements, or an expression
editor-cannot-extract-a-variable-here-select-an-expression = Cannot extract a variable here: select an expression
editor-cannot-extract-a-constant-here-select-an-expression = Cannot extract a constant here: select an expression of constants, such as a literal
editor-cannot-extract-a-field-here-select-an-expression = Cannot extract a field here: select an expression
editor-nothing-to-inline-here-put-the-caret-on = Nothing to inline here: put the caret on a local variable, a method or a constant
editor-a-library-class-is-read-only = A library class is read only
editor-replace-all-occurrences = Replace all occurrences
editor-replace-this-occurrence-only = Replace this occurrence only
editor-breakpoint-on-line = Breakpoint on line { $line }
editor-filter-by-class-member-or-file = Filter by class, member or file
editor-show-in-endpoints = Show in Endpoints
editor-nothing-matches = Nothing matches.
editor-of = { $matching } of { $total }
editor-bean = Bean '{ $b }'
editor-bean-is-injected-into = Bean '{ $b }' is injected into
editor-takes = '{ $name }' takes { $value }
editor-no-bean-for = No bean for '{ $name }'
editor-press-for-quick-fixes = { $why }. Press ⌘. for quick fixes.
editor-could-take-any-of = '{ $name }' could take any of
editor-takes-2 = '{ $name }' takes
editor-takes-the-bean = '{ $name }' takes the bean
editor-fix-all-in-file = Fix all ‘{ $title }’ in file
editor-supertypes = Supertypes
editor-subtypes = Subtypes
editor-callers = Callers
editor-callees = Callees
editor-put-the-caret-on-a-type-or-a = Put the caret on a type or a method, then ⌃H or ⌃⌥H.
editor-put-the-caret-on-a-class-or-an = Put the caret on a class or an interface for its hierarchy.
editor-put-the-caret-on-a-method-for-its = Put the caret on a method for its calls.
editor-type-hierarchy-of = Type hierarchy of
editor-calls-of = Calls of
editor-reading-the-hierarchy = Reading the hierarchy
editor-the-java-engine-is-looking-it-up = The Java engine is looking it up.
editor-no-hierarchy-yet = No hierarchy yet
editor-no-hierarchy = No hierarchy
editor-needs-jdk = Needs JDK { $v }
editor-importing-gradle-model = Importing Gradle model
editor-restarting = Restarting
editor-syncing-the-build-model = Syncing the build model
editor-generating-sources = Generating sources
editor-importing = Importing
editor-of-modules-ready = { $ready } of { $n } modules ready
editor-java = Java: { $summary }
editor-java-2 = Java: { $what }
editor-java-3 = Java: { $text }
editor-the-java-engine-is-not-built-run-snout = The Java engine is not built (run snout.sh)
editor-the-java-engine-did-not-start = The Java engine did not start: { $e }
editor-the-java-engine-keeps-stopping = The Java engine keeps stopping
editor-references-to = References to { $name }
editor-find-references-f12-lists-them-here = Find References (⇧F12) lists them here
editor-results = { $count ->
    [one] { $count } result
   *[other] { $count } results
    }
editor-title-results = { $title }: { $results }
editor-no-usages-found = No usages found
editor-no-usages-yet = No usages yet
editor-find-usages-on-a-symbol-lists-every-place = Find Usages on a symbol lists every place it is used.
editor-find-usages-at-the-caret = Find Usages at the Caret
editor-filter-problems = Filter problems
editor-show = Show { $what }
editor-expand-all = Expand all
editor-collapse-all = Collapse all
editor-no-problems = No problems
editor-nothing-to-fix-in-the-files-the-java = Nothing to fix in the files the Java engine checked.
editor-no-problems-match = No problems match
editor-clear-the-filter-or-turn-on-more-of = Clear the filter, or turn on more of errors, warnings and information above.
editor-filter-by-name = Filter by name
editor-sorted-by-name-click-for-file-order = Sorted by Name (click for File Order)
editor-sort-by-name = Sort by Name
editor-no-structure = No structure
editor-reading-the-structure = Reading the structure
editor-try-fewer-letters-of-the-name = Try fewer letters of the name.
editor-nothing-declared = Nothing declared
editor-the-file-declares-no-class-field-or-method = The file declares no class, field or method.
editor-package-private = package private
editor-finding-node = Finding Node
editor-needs-node = Needs Node { $what }
editor-installing-the-language-server = Installing the language server
editor-typescript = TypeScript: { $what }
editor-or-newer = { $m } or newer
editor-the-typescript-language-server-did-not-start = The TypeScript language server did not start: { $e }
editor-the-typescript-language-server-failed = The TypeScript language server failed: { $e }
editor-the-typescript-language-server-keeps-stopping = The TypeScript language server keeps stopping
editor-open-a-java-file = Open a Java file to see its structure.
editor-java-engine-reading = The Java engine is reading the file.
editor-problems-title = Problems
editor-downloading-java-for-the-engine = Downloading Java { $ENGINE_JAVA } for the engine
editor-copying-the-java-engine = Copying the Java engine
editor-java-engine-install-failed = The Java engine could not be installed: { $why }
editor-ts-install-failed = The TypeScript language server could not be installed: { $why }

## blame

blame-month-1 = January
blame-month-2 = February
blame-month-3 = March
blame-month-4 = April
blame-month-5 = May
blame-month-6 = June
blame-month-7 = July
blame-month-8 = August
blame-month-9 = September
blame-month-10 = October
blame-month-11 = November
blame-month-12 = December
blame-long-date = { $month } { $day }, { $year } at { $time }

## agents

agents-turn = Turn { $id }
agents-files = { $files } files
agents-1-file = 1 file
agents-revert-what-this-prompt-changed = Revert what this prompt changed
agents-accept-what-this-prompt-changed = Accept what this prompt changed
agents-editing = Editing
agents-waiting-for-your-review = Waiting for your review
agents-claude-asks-to-make-this-change = Claude asks to make this change
agents-waiting-behind-another-change-to-this-file = Waiting behind another change to this file
agents-1-file-changed = 1 file changed
agents-files-changed = { $files } files changed
agents-revert-all = Revert All
agents-put-back-every-file-as-it-was-before = Put back every file as it was before Claude changed it
agents-accept-all = Accept All
agents-keep-every-change-the-files-leave-this-list = Keep every change; the files leave this list
agents-seen-on-disk-while-claude-code-was-connected = Seen on disk while Claude Code was connected; its hooks did not report it
agents-revert-the-file = Revert the file
agents-accept-the-file = Accept the file
agents-lines-removed = { $count } lines removed
agents-revert-this-change = Revert this change
agents-accept-this-change = Accept this change
agents-no-changes-from-agents-yet = No changes from agents yet
agents-the-files-an-agent-changes-show-here-as = The files an agent changes show here as it works, each change to accept or revert.
agents-open-the-agent-console = Open the Agent Console
agents-a-new-file-the-agent-created-it-reverting = A new file: the agent created it; reverting deletes it
agents-sent-line-to = Sent { $name } line { $a } to { $agent }
agents-sent-lines-to = Sent { $name } lines { $a }–{ $b } to { $agent }
agents-sent-to = Sent { $name } to { $agent }
agents-asked-to-fix-line = Asked { $short } to fix { $name } line { $a }
agents-asked-to-fix-lines = Asked { $short } to fix { $name } lines { $a }–{ $b }
agents-snoutcode-cannot-reach-the-agent-in-this-window = SnoutCode cannot reach the agent in this window. Close the project and open it again.
agents-asked-for-the-commit-message = Asked { $name } for the commit message
agents-asked-to-fix-the-sign-in = Asked { $name } to fix Git’s sign in
agents-asked-to-fix-the-run = Asked { $name } to fix { $title }
agents-claude-is-waiting-for-your-review = Claude is waiting for your review
agents-is-editing = { $short } is editing
agents-waiting = Waiting
agents-done = Done
agents-no-calls-yet = No calls yet
agents-what-an-agent-in-the-console-asks-of = What an agent in the console asks of SnoutCode shows here.
agents-open-a-file-to-insert-into = Open a file to insert into
agents-insert = Insert
agents-run-in-terminal = Run in Terminal
agents-nothing-shown-yet = Nothing shown yet
agents-what-an-agent-in-the-console-chooses-to = What an agent in the console chooses to show you appears here, with buttons on its code.
agents-from = { $title }, from { $agent }
agents-from-2 = From { $agent }
agents-all = Agents
agents-two = { $a } and { $b }
agents-n = { $n } agents
agents-changed-files = { $count ->
    [one] { $who } changed 1 file
   *[other] { $who } changed { $count } files
    }
agents-seen = seen
agents-new = new
project-shared = shared

## keys

keys-page-up = Page Up
keys-page-down = Page Down
keys-backspace = Backspace

## json

json-true-or-false = true or false
json-a-whole-number = a whole number
json-a-number = a number
json-a-string = a string
json-a-list = a list
json-an-object = an object
json-from-to = { $only } from { $min } to { $max }
json-any-value = any value
json-or = { $join } or { $last }
json-default = {"\u000A"}{"\u000A"}Default: `{ $shown }`
json-unexpected-text-after-the-settings = Unexpected text after the settings
json-the-settings = The settings
json-takes = { $what } takes { $s }
json-is-at-least = { $what } is at least { $min }
json-is-at-most = { $what } is at most { $max }
json-unknown-setting-snoutcode-ignores-it = Unknown setting `{ $key }`: SnoutCode ignores it
json-needs = { $what } needs `{ $req }`
json-a-value-is-missing-here = A value is missing here
json-a-key-in-double-quotes-is-expected-here = A key in double quotes is expected here
json-a-comma-or-a-closing-brace-is-expected = A comma or a closing brace is expected here
json-a-comma-or-a-closing-bracket-is-expected = A comma or a closing bracket is expected here
json-this = this
json-is-not-a-json-value-strings-take-double = { $shown } is not a JSON value (strings take double quotes)
json-this-string-cannot-be-read = This string cannot be read: { $e }
json-this-string-has-no-closing-quote = This string has no closing quote
json-colon-expected = A colon is expected after "{ $key }"

## core

core-head-to-worktree = HEAD ↔ Working tree
core-index-to-worktree = Index ↔ Working tree
core-head-to-index = HEAD ↔ Index
core-index = Index
core-scope-message-only = message only
core-scope-nothing = nothing to commit
core-what-staged = the staged changes
core-what-changed-file = the changed file
core-what-all-changed = all { $n } changed files
core-what-selected-file = the selected file
core-what-selected = the { $n } selected files
core-what-message = the new message for the last commit
core-what-nothing = nothing
core-state-staged = { $state } (staged)
core-state-partly-staged = { $state } (partly staged)
core-state-unchanged = Unchanged
core-branch-dash = A branch name cannot start with a dash.
core-branch-dots = A branch name cannot contain “..” or “@{"{"}”.
core-branch-chars = A branch name cannot contain ~ ^ : ? * [ or \.
core-branch-ends = A branch name cannot start with “/” or end with “/”, “.” or “.lock”.
core-group-taken = There is already a group with this name
core-years-ago = { $count ->
    [one] { $count } year ago
   *[other] { $count } years ago
    }
core-engine-built-in = Built-in
core-ghostty-unavailable = This build was made without Ghostty. Release builds include it.
core-gap-unchanged = { $count ->
    [one] ⋯ { $count } unchanged line
   *[other] ⋯ { $count } unchanged lines
    }
core-update-checking = Checking for updates…
core-update-found = Update found (v{ $version }), downloading…
core-update-preparing-now = Preparing update…
core-update-downloading-now = Downloading update…
core-update-ready = Update ready: v{ $version }
core-update-from-source = This build was made from source and does not update itself. Build it again from the latest source to update.
core-update-up-to-date = You’re up to date (v{ $version })
core-update-available = SnoutCode v{ $version } is available.
core-update-cannot-replace = SnoutCode v{ $version } is available, and cannot install itself from where it runs. Move it to your applications, or download it.
core-update-did-not-land = SnoutCode v{ $version } did not install. Download it to install it by hand.
core-update-preparing = Preparing update
core-update-downloading = Downloading update
core-update-offline = Updates could not be checked: this computer is offline. It will try again later.
core-update-feed-down = Updates could not be checked: GitHub is not answering. Nothing is wrong with your installation, and it will try again later.
core-update-rate-limited = Updates could not be checked: GitHub is rate limiting this network. It will try again later.
core-update-no-release = Updates could not be checked: no published release was found to update to.
core-update-signature = An update was downloaded and refused: it did not match the signature it should have. Nothing was installed.
core-update-disk = The update could not be saved: there is not enough disk space.
core-update-unknown = Updates could not be checked.
core-scope-staged = { $n } staged
core-scope-all-changed = all { $n } changed
core-scope-selected = { $n } selected

## icons

icons-plain-text = Plain Text

## menu

menu-menu = Menu

## misc

newproject-location = Location
newproject-offline-fallback = { $why } These are the dependencies SnoutCode knows; the project is made here, and Gradle downloads Spring Boot once there is a network.
structure-modules-row = Modules
structure-jdk-hint = Gradle and the Java engine run on it. Kept in .snout/settings.json; the engine restarts on it the next time it starts.
structure-locked-maven-hint = Values with a lock are Maven’s: Edit in build file opens the pom.xml line that sets them. Once you save it, the bar over the editor offers to reload.
structure-locked-gradle-hint = Values with a lock are the build’s: Edit in build file opens the line that sets them. Once you save it, the bar over the editor offers to sync.
console-installer-not-on-path = The installer finished, but { $agent } is not on the PATH SnoutCode sees. Set its path in Settings, Agents, or restart SnoutCode.
console-no-project-open = No project is open: { $agent } starts in your home folder, where it can make one. Open the project when it is there, to follow its edits.
dialogs-group-example = Work
dialogs-checkout-placeholder = Filter branches and tags, or enter a commit (SHA, HEAD~2, …)
gutter-run-label = Run ‘{ $label }’
gutter-debug-label = Debug ‘{ $label }’
editor-settings-source = SnoutCode settings
terminal-not-installed = { $name } is not installed
terminal-is-mac-app = { $name } is a Mac app
terminal-not-mac-app = { $name } is not a Mac app
terminal-not-windows-app = { $name } is not a Windows app
terminal-default-linux = Default Terminal
model-could-not-open-in = Could not open the project in { $connection }
model-no-repositories-found = No repositories found
env-environments = Environments
env-base-url-hint = Runs get the URL as BASE_URL, which the configuration’s baseURL reads. The file’s variables come first, then those above. Keep passwords in the .env file, which the repository should ignore.
pwsetup-runner-missing = the Playwright test runner is not installed
remote-password-title = Password
run-needs-jdk-note = This build needs JDK { $v }, which isn’t installed. Download it from the bar above the editor, or pick a JDK in Settings.
debug-js-download-failed = The JavaScript debugger could not be downloaded: { $e }
debug-js-did-not-start = The JavaScript debugger did not start: { $e }
run-ts-needs-tsx = Running a TypeScript file needs the tsx package in the project ({ $manager } add -D tsx) or Node 22.6 or later (this project runs on Node { $node }).
settings-open-project-settings = Open Project Settings
settings-each-project-chooses-its-profiles = Each project chooses its active profiles, in Project Settings.
structure-group-structure = Structure
structure-group-settings = Settings for this project
structure-download-jdk-dialog = Download JDK…
structure-looking-up-jdks = Looking up the JDKs this machine can download…
structure-jdks-could-not-be-looked-up = The JDKs could not be looked up: { $e }
structure-java-lts = { $major } (long-term support)
structure-vendor = Vendor
structure-location = Location



