# Playwright projects

SnoutCode opens a repository of Playwright tests as a project of its own: TypeScript editing, the Node it runs on, its packages and browsers, and a Tests window that follows each run, without Java. It suits a QA team whose tests live in their own repository and test the application from the outside, through its URL.

## What counts as one

A folder is a Playwright project when it has a `playwright.config.ts` (or `.js`, `.mjs`, `.cjs`, `.mts`, `.cts`), or a `package.json` that depends on `@playwright/test`. Open it as any project: **File ▸ Open…**, or from the welcome page.

The Project view shows the project's folders and files. A line at its top says which Playwright the project has, its package manager and the Node it runs on, such as `Playwright 1.64.1 · npm · Node 22.11.0`, or `not installed` until its packages are.

A Playwright project never starts the Java engine. Its TypeScript files get completion, navigation, rename and problems from the TypeScript server; see [TypeScript and JavaScript](typescript.md).

## Getting it ready to run

When the project opens, SnoutCode looks at what its tests need, and a bar above the editor says what is missing:

- **Its packages.** When `node_modules` does not hold the test runner yet, the bar offers **Install packages**. SnoutCode runs `npm ci` (or `npm install` when there is no `package-lock.json`). A project with a `pnpm-lock.yaml`, `yarn.lock` or `bun.lock` gets `pnpm install`, `yarn install` or `bun install` instead; that tool must be installed.
- **Its browsers.** Each Playwright version runs particular builds of Chromium, Firefox and WebKit. When those are not downloaded, the bar names them and offers **Install browsers**, which runs `playwright install` with the project's own Playwright. The browsers go where Playwright keeps them (`~/Library/Caches/ms-playwright` on a Mac, `~/.cache/ms-playwright` on Linux, or `PLAYWRIGHT_BROWSERS_PATH` when you set it), so the command line and SnoutCode share them.

Both run in the **Build** window, where their output shows as it comes, with **Stop** to end them. When an install ends, SnoutCode looks again: after the packages it checks the browsers, and the TypeScript server starts again so it sees the packages' types. If an install fails, the bar says so; the Build window shows why.

The commands run on the Node the project asks for (see [Node](typescript.md#node)). When none fits, the Node bar comes first.

**Not now** hides the bar until something else is missing.

On Linux, browsers also need system libraries. Playwright installs them with `sudo npx playwright install-deps`, which you run yourself in the Terminal.

## Writing tests

![Record at Cursor writing the recorder's steps into a test](images/playwright-recording.png)

Three tools help write tests without typing every locator. They start where the tests do: the application's URL from the environment picked (or the configuration's `baseURL`), in the configuration's browser, with its test id attribute (`testIdAttribute`). When a project of the configuration logs in once and keeps the session (`storageState`, saved by a setup project), they start logged in, provided a run has saved that file.

- **Record at Cursor** (**Run ▸ Record at Cursor**, or the editor's right-click menu in a spec) opens Playwright's recorder: a browser, and Playwright's Inspector beside it. What you do in the browser comes back as steps, written into the spec as you go, where the caret was:
  - in a test, a hook or a `test.step`, the steps go there, at its indentation;
  - between tests, they go in a new test (`test('new test', …)`) to rename;
  - in an empty file, the whole spec goes in, with its `import`.

  A `page.goto` to the application's URL is written relative to it (`page.goto('/cart')`), so the test runs against any environment. The recorder's toolbar in the browser adds assertions (visible, text, value). A bar on the tab counts the steps; **Stop**, or closing the browser, ends the recording. If you edit the recorded steps while it goes, it stops writing them and says so.
- **Pick Locator** opens the browser at the same place. Each element you click puts its locator at the caret, as Playwright would write it: `page.getByRole('button', { name: 'Log in' })`, or the locator alone after a `.`. Click another element without moving the caret and it takes the last one's place; move the caret first to keep both. It needs a Playwright that has `page.pickLocator()` (1.64 has it). **Stop** or closing the browser ends it.
- **Open UI Mode** (**Run ▸ Open UI Mode**) starts Playwright's own UI Mode (`playwright test --ui`) with the environment's variables, in your browser: its watch mode, its timeline and its locator tools. Stop it from the **Build** window.

The recorder and the picker run in the **Build** window, where their output shows.

## Running tests

A Playwright project runs its tests with its own Playwright, on the Node it asks for, the way `npx playwright test` would. Start a run from any of these:

- **The Tests window**: **Run all**, **Rerun failed** (Playwright's `--last-failed`, which needs Playwright 1.44 or later) and, for the test picked, **Run this test**.
- **The title bar**: the configurations **All tests** and one per project of the configuration (**Tests in chromium**, …), with Run and [Debug](#debugging-tests).
- **The editor**: each `test(…)` and `test.describe(…)` of a spec file has a run icon in the gutter. ⌃⇧R runs the test the caret is in.

**Edit Configurations…** makes your own: a configuration of the Playwright kind takes Playwright's arguments (`--grep @smoke`, `--repeat-each=5`) and environment variables.

The console in the **Run** window reads as Playwright's `list` reporter. The **Tests** window follows the same run test by test.

**Stop** ends the run and the browsers it started: everything the run started goes with it.

## Debugging tests

![A test stopped at a breakpoint in its spec, with the worker's stack and the variables in the Debug window](images/playwright-debugging.png)

**Debug** on a test (**Debug '…'** on its gutter icon, the title bar's Debug for a configuration, or **Debug this test** in the Tests window) runs it under the JavaScript debugger, which stops at the breakpoints in the code the test runs: the spec, its page objects, helpers and fixtures. Click a line number of a TypeScript or JavaScript file to set one; conditions, disabling and **View Breakpoints…** work as they do for Java (see [Run and debug](run-and-debug.md#breakpoints)).

When the test stops, the editor shows its line with the values of the variables there, and the **Debug** window shows the stack and the variables and watches. Step over, into and out, **Run to Cursor**, **Evaluate Expression** and **Resume** work as for Java. Breakpoints in the TypeScript of a spec stop at the TypeScript line, through the source maps Playwright makes.

A debug run has one worker, shows the browser, and has no time limit (`--headed --workers=1 --timeout=0`), so a test paused at a breakpoint does not time out. The Tests window follows it as it follows any run, and **Stop** ends it.

The debugger is js-debug, Microsoft's JavaScript debugger. SnoutCode downloads it the first time you debug (about 1 MB, from its GitHub releases, checked against its checksum) and runs it on the project's Node. It follows every Node process of the run: Playwright's runner and its worker.

**Debug in Inspector** in the Tests window runs the test in the Playwright Inspector instead (`--debug`), which pauses before each Playwright action and has its own locator picker.

## The Tests window

![The Tests window after a run: the results grid, and a failure's detail](images/playwright-tests-window.png)

The **Tests** window opens on its own when a run starts. Its button is at the bottom of the left stripe, with the number of failed tests on it.

- **The summary bar** shows the run's progress, how many tests passed, failed, were flaky (they passed on a retry) or skipped, the time taken and, while it runs, about how long is left (from the tests' past times). Next to it are the environment, **Watch**, **Rerun failed**, and **Run all** or **Stop**.
- **Watch** runs the next runs with the browsers showing, one test at a time (`--headed --workers=1`).
- **The workers**: while a run goes, each worker has a line with its project, its test and the step it is at, such as `click getByRole('button', { name: 'Pay' })`.
- **The results grid** has a row per test, grouped by file, and a column per project of the configuration (each browser). Each cell has the result and the time. **Last 10 runs** shows the test's past results, and **Flaky** how often it passed only on a retry, or failed and then passed with nothing changed. Filter by **Failed**, **Flaky** or **Slow** (over 4 s), or by text.
- **A test's detail**, when you pick a cell or a row:
  - the error, with the lines around the one that failed;
  - for a screenshot that did not match, the expected, actual and difference images side by side;
  - the steps, with their times;
  - from the test's trace, a filmstrip of the page, the requests that failed and the console's errors;
  - what the test printed.

  Its buttons:
  - **Accept new screenshot** runs the test again with `--update-snapshots`, so its screenshots become the expected ones.
  - **Open full trace** opens Playwright's own trace viewer, with the DOM at each step.
  - **Video** plays the recording in the system's player.
  - **Run this test** runs it again in its project.
  - **Debug this test** runs it again under the debugger, which stops at its breakpoints.
  - **Debug in Inspector** runs it in the Playwright Inspector (`--debug`), to step through it.
  - **Show in editor** opens the test at its line.
  - **Copy error** copies the error and its stack.

  The trace, the screenshots and the video are there when the configuration records them (`trace`, `screenshot`, `video` under `use`). A project made with **New Project** keeps them when a test fails.

When a run of more than a minute ends while SnoutCode is in the background, the Dock icon bounces once and shows the number of failures until you come back.

## Environments

The tests point at an application through its URL, and the URL changes from run to run: your machine, staging, a review app. **Run ▸ Edit Environments…** keeps a list of environments. Each has:

- a name;
- the **Application URL**, which runs get as `BASE_URL` (the configuration of a project made with **New Project** reads it as `baseURL`);
- other **variables**, such as `USER_EMAIL=qa@example.com; LOCALE=en`;
- a **`.env` file** of the project to read first, such as `.env.staging`.

Pick the environment runs go against in the title bar, or in the Tests window's summary bar. **No environment** leaves the configuration as it is.

Environments are kept with the project's settings on this machine, outside the repository, so each tester has their own. Keep passwords in the `.env` file and leave that file out of the repository.

## History

Every run is kept on this machine, outside the repository, with the latest 200 runs of each project. The **History** tab lists them, newest first, with:

- what ran and the environment;
- the branch and commit of the tests;
- the counts and the time;
- **New since the run before**: the tests that failed in that run but not in the one before it.

The grid's **Last 10 runs**, the flaky rates and the time left all come from this history.
