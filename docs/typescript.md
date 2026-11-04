# TypeScript and JavaScript

How SnoutCode understands TypeScript and JavaScript: completion, navigation, rename and problems. It is made for Node projects (see [Node projects](node.md)), test projects such as a Playwright repository, and the scripts beside a Java project.

## Where it works

The TypeScript features work in `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs` and `.cjs` files. They come from a TypeScript language server that starts when you open one of those files. Files inside `node_modules` open highlighted, without it.

There is one server per TypeScript project: the folder of the nearest `tsconfig.json`, `jsconfig.json` or `package.json` above the file, or the project's folder when there is none. It stops when you open another project in its place, or quit.

A project without Java never starts the Java engine. A project with both, such as a Spring Boot application with an `e2e/` folder of Playwright tests, runs each server for its own files.

The server uses the TypeScript version your project installs (`node_modules/typescript`), so it reports what `tsc` reports. Without one, it uses the TypeScript it comes with.

## Node

The server runs on Node. SnoutCode never needs Node on your `PATH`: it looks for the version your project asks for, in this order:

1. `.nvmrc` or `.node-version`, in the project's folder or one above it.
2. `"volta": { "node": … }` in `package.json`.
3. `"engines": { "node": … }` in `package.json`, such as `>=20`.

It then picks the newest Node that fits from the ones on your machine: the one on your login shell's `PATH`, and those installed with nvm, Volta, fnm, asdf, mise or Homebrew. A project that asks for nothing gets the newest Node found.

When none fits, a bar above the editor says which version the project needs and offers **Download**. SnoutCode then downloads that Node from nodejs.org, checks it against the release's checksums, keeps it in its own data folder, and starts the TypeScript server on it. Your shell, your `PATH` and your version manager are left as they are. **Not now** hides the bar until SnoutCode starts again; the status bar keeps saying **TypeScript: Needs Node** with the version. A Node you install yourself later is found the next time the server starts.

The first time it runs, SnoutCode installs the language server into its own data folder with that Node's npm. This takes a few seconds and needs a network connection. Your project's `node_modules` is never changed.

## What you get

- **Completion** as you type a name or a `.`, with auto imports. Press ⌃Space to open it anywhere.
- **Hover** over a name for its type and its documentation.
- **Go to Definition** (F12) and **Find References** (⇧F12).
- **Rename** a symbol across the project (F2).
- **Quick fixes** (⌥↩), such as adding a missing import.
- **Problems**: type errors and warnings show as squiggles, in the inspections widget at the editor's top right, and in the Problems window.

The status bar shows the server's state for the file in front: **TypeScript: Ready**, **Installing the language server**, **Needs Node 22** and the like.

## Highlighting

TypeScript, TSX, JavaScript and JSON files are highlighted. **Toggle Comment** (⌘/) uses `//` in TypeScript and JavaScript.
