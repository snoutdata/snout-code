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
settings-blurb-gradle = The JDK each build runs on, and how long SnoutCode's Gradle daemons stay.
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
cmd-project-structure = Project Structure…
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
settings-your-settings-for-every-project-a-project-can = Your settings, for every project. A project can override some of them.
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
settings-each-builds-own-your-settings = Each build's own (your settings)
settings-every-build = Every build
settings-jdk-per-project = JDK per project
settings-no-gradle-build-is-open = No Gradle build is open.
settings-automatic-the-builds-java-level = Automatic (the build's Java level)
settings-the-java-server-restarts-on-the-chosen-jdk = The Java server restarts on the chosen JDK the next time it starts.
settings-gradle-daemons = Gradle daemons
settings-stop-idle-daemons-after = Stop idle daemons after
settings-double-click-a-task-in-the-gradle-tool = Double click a task in the Gradle tool window to run it. Its output shows in the Build window.
settings-the-processes-window-lists-the-gradle-daemons-running = The Processes window lists the Gradle daemons running, and stops the idle ones on request.
settings-project-structure-downloads-jdks-and-sets-the-projects = Project Structure (⌘;) downloads JDKs and sets the project's own.
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
git-changed-staged = { $count } changed, { $staged_count } staged
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
git-changed = { $count } changed
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

## workspace

workspace-could-not-make = Could not make { $path }: { $e }
workspace-do-you-want-to-save-the-changes-you = Do you want to save the changes you made to { $one }?
workspace-do-you-want-to-save-the-changes-you-2 = Do you want to save the changes you made to { $count } files?
workspace-your-changes-are-lost-if-you-dont-save = Your changes are lost if you don't save them.
workspace-dont-save = Don't Save
workspace-open-a-project-to-see-its-structure = Open a project to see its structure
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

## lang

lang-symbols-come-from-the-java-server-which-starts = Symbols come from the Java server, which starts with a Java file
lang-file-structure-shows-the-symbols-of-a-java = File Structure shows the symbols of a Java file
lang-classes-come-from-the-java-engine-which-starts = Classes come from the Java engine, which starts with a Gradle project
lang-hierarchies-come-from-the-java-engine-in-a = Hierarchies come from the Java engine, in a Java file
lang-downloading-jdk = Downloading JDK { $major }… { $pct }%
lang-downloading-jdk-2 = Downloading JDK { $major }…
lang-jdk-could-not-be-downloaded = JDK { $major } could not be downloaded: { $error }
lang-this-project-needs-jdk-download-it = This project needs JDK { $major }. Download it?
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

## run

run-is-not-among-the-projects-sources = { $class } is not among the project's sources
run-the-file-in-front-has-no-tests-main = The file in front has no tests, main class or script to run
run-nothing-to-run-here-put-the-caret-in = Nothing to run here: put the caret in a main class, a test or a script
