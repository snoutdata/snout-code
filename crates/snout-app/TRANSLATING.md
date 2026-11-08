# The interface's languages

Each language is one [Fluent](https://projectfluent.org) catalog, `i18n/<tag>/snout-app.ftl`. English
(`en`) is the source: every message is in it, and a message missing elsewhere falls back to it.

## Correcting a translation

Edit the line in that language's catalog. Nothing else changes. `SNOUT_SHOT_LANGUAGE` takes the
screenshots in a language (`French`, `Spanish` or `Portuguese`), to see that the words fit:

    SNOUT_SHOT_LANGUAGE=French SNOUT_SCREENSHOT=/tmp/shots cargo test -p snout-app --test screenshots

## Adding a language

1. Copy `i18n/en/` to `i18n/<tag>/` (`de`, say) and translate the values. Leave the message names and the
   `{ $arguments }` as they are.
2. In `src/i18n.rs`, add the language to `Language`, `Language::ALL`, `tag`, `native_name` and
   `from_requested`.
3. Add the tag to `CFBundleLocalizations` in `scripts/package-mac.sh`.
4. Run `cargo test -p snout-app i18n`. It fails on a message that is missing, an argument that
   differs from English, or a catalog that does not parse.

## Writing messages

- In the code, `t!("name", argument = value)` gives a `String` and `ts!("name")` a `&'static str`.
  Both are checked against the English catalog when the app is compiled.
- A count is passed as a number, so that the catalog can choose the plural:

      files-changed = { $count ->
          [one] { $count } file changed
         *[other] { $count } files changed
      }

- What decides how the app behaves stays in English in the code, and is translated where it is
  shown. Git's and the build tools' output and what is sent to a coding agent are not translated.
- Product names and Git's verbs (Pull, Fetch, Push, Commit, Merge, Rebase) stay as they are.
  No em dashes.
