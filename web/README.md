# snoutcode.com

The SnoutCode website: a static React + Vite site with no backend. Every page is prerendered to
HTML at build time and then hydrated, so the site works without JavaScript and reads fully to a
crawler. It is hosted on Cloudflare Pages (project `snout-code`).

```sh
cd web
npm install
npm run dev        # http://localhost:5190
npm run build      # dist/: one HTML file per page, plus the assets
```

## How it is put together

- `src/routes.ts`: every page, with its title and description. `scripts/prerender.mjs` writes one
  HTML file per entry (`index.html`, `download.html`, `privacy.html`, `terms.html`, `404.html`) and `sitemap.xml`.
- `src/pages/`: the pages. `src/components/`: the header, footer, product shots and the art.
- `src/site.ts`: every link to the repository, in one place.
- `src/download.ts`: the download page's guess of the visitor's system, and the latest release's
  files, read from the GitHub API in the browser so the page never needs a redeploy after a
  release. The file names follow `scripts/make-release.sh`; a renamed file is changed in `FILES`.
- `src/styles.css`: the one stylesheet. Colours are tokens defined for both themes at the top;
  light is the default and dark is opt-in from the header.
- `scripts/assets.mjs` copies the screenshots the pages show from `../docs/images` and the icon
  from `../assets/icon` into `public/`, so they have one source each. A new screenshot on a page
  is added to its list. The copies are not committed.

## Translations

The site speaks English, French, Spanish and Portuguese (Brazilian), with the same small i18n as
snoutdata.com and no dependency:

- `src/i18n/index.tsx`: `<I18nProvider>` (wrapped around every page in `App.tsx`), `useT()` for
  `t('dot.path', { vars })`, `useI18n()` for `tList()` (string arrays such as feature bullets) and
  the locale, and `withSlots()` for a link inside a sentence (`"A {link} project."`).
- `src/i18n/{en,fr,es,pt}.json`: the catalogs. `en.json` is the source; every key in it is in the
  other three. A missing key falls back to English, never to a blank.
- `src/components/LangSwitcher.tsx`: the globe in the header. A choice is kept in `localStorage`
  (`lang`); with none, the browser's language decides, then English.
- The language is not in the URL. Every page is prerendered and hydrated in English, then switched
  before the first paint: a script in `index.html` hides the page (`i18n-wait`) for a visitor in
  another language, and the provider shows it once switched. Search engines index the English
  site. If you change how the language is chosen, change `detectLocale()` and that script together.

Adding text: put the English under a key in `en.json`, the same key in the other three, and read
it with `t()`. Data arrays in the `.tsx` keep only structure (ids, links, images); their words are
in the catalogs under the id, and the id is the React key, so switching language never remounts a
revealed card. Translations say the thing the way a developer in that language would, not word for
word. Product names (Java, Gradle, Spring Boot, Git, Remote SSH) stay as they are, and so do the
app's own UI names, since the app is in English: the Tests window, Agent Log, Record at Cursor,
Allow for This Session. The Agent Log and the permission card drawn on the homepage are the app's,
so they stay in English too. The page titles and descriptions (`src/routes.ts`) stay English. The
punctuation rule below holds in every language; French puts a no-break space before `: ; ? !`.

The header fits its longest language, Spanish: below 1100px the switcher shows only the globe,
below 980px the GitHub pill only its mark, and below 900px the links go.

## The art

The drawings in `src/components/Illustrations.tsx` follow the Snout house style: a round ink line
over a few flat fills, all taken from the `--ill-*` tokens so they follow the theme, one green
on the thing a picture is about, and the warm tones only where warmth is the point.

- **The butterfly** is the app icon's (`assets/icon/snout.svg`): its body is a commit chain and its
  veins are branches. On the site it wears the coder's round spectacles where it is drawn large
  enough to show them. The icon stays the logo in the header, the footer and the favicon.
- **Section drawings** (`SectionArt`, a `0 0 360 140` box) sit beside each homepage heading.
- **Card drawings** (`CardArt`, `0 0 200 100`) open the feature and download cards.
- **Scenes** (`0 0 1200 340`) end a page edge to edge and are cropped, never squashed, on a narrow
  screen and, past a 2680px window, at the top: keep anything that matters below y=60 and
  between x=100 and x=1100.

Everything moving in a drawing stops under `prefers-reduced-motion`.

## Copy

User-facing text uses proper punctuation: no em dashes, and no hyphens used as dashes. The site
never names other Git clients.

## Deploying

The repository is public, so it holds no credentials and no account details. A deploy takes them
from the environment:

```sh
CLOUDFLARE_API_TOKEN=<a Pages: Edit token> CLOUDFLARE_ACCOUNT_ID=<the account> npm run deploy
```

`npm run deploy` builds, then uploads `dist/` with a pinned wrangler. It never falls back to a
`wrangler login` session. The link preview image, `public/og-image.png`, is rendered from the
built hero by `npm run og` (it needs `npm i --no-save puppeteer`).
