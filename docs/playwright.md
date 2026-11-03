# Playwright projects

SnoutCode opens a repository of Playwright tests as a project of its own: TypeScript editing, the Node it runs on, and its packages and browsers, without Java. It suits a QA team whose tests live in their own repository and test the application from the outside, through its URL.

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
