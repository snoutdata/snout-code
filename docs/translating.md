# Translating SnoutCode

SnoutCode's window and menus are in English, French (`fr`), Spanish (`es`) and Brazilian Portuguese
(`pt-BR`). You can help by correcting a translation or by adding a language. No translation has
been proofread by a native speaker yet, so a correction of even one line is useful.

Each language is one text file, a [Fluent](https://projectfluent.org) catalog in
[`crates/snout-app/i18n/`](../crates/snout-app/i18n/). English (`en`) is the source: every message
is in it, and a message missing from another language shows in English until someone translates it.

A catalog is a list of messages. The name on the left stays as it is, and the text on the right is
what you translate:

    settings-apply = Aplicar
    agents-files-changed = { $files } arquivos alterados

## If you do not write code

[Open an issue](https://github.com/snoutdata/snout-code/issues/new) that says what is wrong, what
it should say and where you saw it. A screenshot helps. To ask for a language, open one naming it.

## Correcting a translation

1. Open the catalog of the language on GitHub:
   [French](../crates/snout-app/i18n/fr/snout-app.ftl),
   [Spanish](../crates/snout-app/i18n/es/snout-app.ftl),
   [Portuguese](../crates/snout-app/i18n/pt-BR/snout-app.ftl).
2. Choose the pencil to edit it, change the text, and propose the change as a pull request.

You do not need to build anything. A maintainer runs the checks before it goes in.

## Adding a language

1. Copy `crates/snout-app/i18n/en/` to `crates/snout-app/i18n/<tag>/`, where the tag is the
   language's standard code (`de` for German, say). Translate the text, a part at a time if you
   like: what you have not reached stays in English.
2. In `crates/snout-app/src/i18n.rs`, add the language to `Language`, `Language::ALL`, `tag`,
   `native_name` and `from_requested`.
3. Add the tag to `CFBundleLocalizations` in `scripts/package-mac.sh`.
4. Run the checks. They fail on a message that is missing, on a value that differs from English in
   the `{ $values }` it takes, and on a catalog that does not parse.

       cargo test -p snout-app i18n

If you would rather not touch the code, a pull request with only the catalog is welcome, and a
maintainer will do steps 2 and 3.

## What to keep

- Keep the name on the left and every `{ $value }` as they are. Move a value within the sentence
  whenever your language needs it.
- Say it the way a developer in your language would, not word for word.
- Keep product names, and Git's verbs where developers in your language use them: Pull, Fetch,
  Push, Commit, Merge, Rebase.
- Do not use em dashes, or hyphens as dashes.
- Git's and the build tools' output, your code, and what is sent to a coding agent are not
  translated.

## Plurals

A message that depends on a number chooses by the plural forms of your language. Use the ones your
language has, as listed in the [plural rules](https://www.unicode.org/cldr/charts/latest/supplemental/language_plural_rules.html),
and keep the `*` on the last one:

    welcome-minutes-ago = { $count ->
        [one] há { $count } minuto
       *[other] há { $count } minutos
    }

## Seeing that the words fit

Other languages often take more room than English. To see every window in your language, take the
screenshots, naming the language as it is named in `Language`:

    SNOUT_SHOT_LANGUAGE=French SNOUT_SCREENSHOT=/tmp/shots cargo test -p snout-app --test screenshots

To try it in the app itself, choose the language under Settings, Appearance, Language.

## For developers: writing messages

- In the code, `t!("name", argument = value)` gives a `String` and `ts!("name")` a `&'static str`.
  Both are checked against the English catalog when the app is compiled.
- A new message goes in every catalog, or the checks fail. If you cannot translate it, put the
  English text there.
- A count is passed as a number, so that the catalog can choose the plural.
- What decides how the app behaves stays in English in the code, and is translated where it is
  shown.
