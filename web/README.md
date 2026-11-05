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
  HTML file per entry (`index.html`, `download.html`, `404.html`) and `sitemap.xml`.
- `src/pages/`: the pages. `src/components/`: the header, footer, product shots and the art.
- `src/site.ts`: every link to the repository, in one place.
- `src/styles.css`: the one stylesheet. Colours are tokens defined for both themes at the top;
  light is the default and dark is opt-in from the header.
- `scripts/assets.mjs` copies the screenshots the pages show from `../docs/images` and the icon
  from `../assets/icon` into `public/`, so they have one source each. A new screenshot on a page
  is added to its list. The copies are not committed.

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
  screen: keep anything that matters below y=60 and between x=100 and x=1100.

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
