# Searching

Every search box in SnoutCode reads what you type the same way and puts the results in the same order: Go to File (⌘P), Recent Files (⌘E), Search Everywhere (⇧⌘A), Go to Class (⌥⌘O), Go to Symbol (⌘T, ⇧⌘O), File Structure (⌘F12), the command palette (⇧⌘P), and the filters of lists such as branches, the Git Panel, Settings and the keymap. Find in Files (⇧⌘F) searches text rather than names, and takes the same `*`.

## What you can type

| Type | Finds | For example |
|---|---|---|
| Part of a name | Names that hold it | `controller` finds `TokenController.java` |
| The starts of words | Names whose words start so (camel humps) | `TC`, `tc` and `tokcon` find `TokenController`; `gn` finds `getName`; `hs` finds `HTTPServer` |
| Letters in order | Names that have them in that order, with gaps | `usrsvc` finds `UserService` |
| `*` or `?` | Whatever is there | `token*.ja` finds `TokenController.java`; `*Test` finds `OrderTest` |
| Several words | Names that match every word, in any order | `user test` finds `UserServiceTest.java` |
| A `/` | Files in folders that match | `web/app` finds `web/src/App.tsx`; `w/s/main` finds `web/src/main.tsx`; `web/` lists everything under `web` |
| `:` and a line | The file, opened at that line (and column) | `Order.java:120`, `Order:120:5` |

Small letters match small letters and capitals alike. A capital you type matches only a capital, so `US` looks for the words *U*ser *S*ervice rather than any name beginning with "us". The list filters are more forgiving: there, `JDK` also finds "jdk".

## The order of the results

Results are grouped by how well they match, best first:

1. The name is what you typed. For a file the extension does not count, so `user` is an exact match for `User.java`.
2. The name begins with what you typed.
3. The starts of the name's words match, as `TC` does for `TokenController`.
4. The name holds what you typed somewhere.
5. The name has the letters you typed in order, with gaps.
6. For files only: the folders hold what you typed, but the name does not.

Within each group:

- In Go to File, the files you opened lately come first, latest first. Then the project's own code and tests, then source folders the build takes from outside the project, then what a build or a package manager made (`build/`, `target/`, `node_modules/` and the like).
- Go to Class and Go to Symbol put the project's own classes before those of its libraries and the JDK.
- Then a match whose capitals are as you typed them, then the shorter name.

The letters that matched are highlighted in each result.

With nothing typed, Go to File lists the files you opened lately first, then the rest of the project.

## Wildcards in Find in Files

In a plain search (not a regular expression), a `*` next to a letter, a digit or `_` stands for whatever is on the line there: `get*Name` finds `getName`, `getUserName` and `getFullName`. Code is full of `*`s that mean themselves, so a `*` with no letter or digit beside it is searched as written: `import java.util.*`, `/**` and `a * b` find exactly that. A match is always on one line.

A `?` in Find in Files is always searched as written, since `?.` and `String?` are everyday code. For a single unknown character, use a regular expression.

Replace in Files replaces the whole of each match, including what the wildcard stood for.

## The order of Find in Files

Files with matches are listed in this order:

1. The project's own code, then its tests, then source folders from outside the project, then what a build or a package manager made.
2. Files whose name holds the text, so searching for `OrderService` lists `OrderService.java` first.
3. Files whose matches are the text exactly as typed: not widened by a wildcard, in the same case, and a whole word.
4. Files open in an editor.
5. Files with more matches.

Within a file, the matches stay in line order. When a search stops at 5,000 matches, it has looked at the files most likely to matter first.

## Big projects

Go to File matches on every core of your machine, and so does the first pass of Find in Files, which finds the files that hold the text before reading them. Files that `.gitignore` excludes are left out of both.
