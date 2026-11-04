// SnoutCode's locator picker: opens the project's browser where the tests start (the URL,
// the saved session, the test id attribute) and, each time an element is clicked, prints
// its locator as Playwright would write it, after the same marker the reporter uses. It runs
// until the browser is closed or the app stops it. SnoutCode writes this file into its data
// folder and runs it on the project's Node, in the project's folder, with what to open as
// JSON in its first argument. It uses the project's own Playwright.

'use strict';

const MARK = '\u001esnout:';

function emit(event) {
  try {
    process.stdout.write(MARK + JSON.stringify(event) + '\n');
  } catch (_) {
    // A closed stdout ends nothing here.
  }
}

async function main() {
  const opts = JSON.parse(process.argv[2] || '{}');
  const pw = require(require.resolve('@playwright/test', { paths: [process.cwd()] }));
  const type = pw[opts.browser] || pw.chromium;
  const browser = await type.launch({ headless: !!opts.headless, channel: opts.channel || undefined });
  browser.on('disconnected', () => process.exit(0));
  if (opts.testIdAttribute) pw.selectors.setTestIdAttribute(opts.testIdAttribute);
  const context = await browser.newContext({
    storageState: opts.storage || undefined,
    viewport: opts.viewport || undefined,
  });
  let page = await context.newPage();
  context.on('page', (p) => {
    page = p;
  });
  if (typeof page.pickLocator !== 'function') {
    emit({ e: 'error', message: 'Pick Locator needs a newer Playwright (page.pickLocator), this project has ' + (pw.version || 'an older one') + '.' });
    await browser.close();
    return;
  }
  if (opts.url) {
    await page.goto(opts.url).catch((e) => emit({ e: 'error', message: String(e.message || e).split('\n')[0] }));
  }
  emit({ e: 'ready' });
  for (;;) {
    const on = page;
    try {
      const locator = await on.pickLocator();
      emit({ e: 'picked', locator: locator.toString() });
    } catch (e) {
      if (!browser.isConnected()) return;
      // The page closed or navigated away while picking: pick on the one in front.
      if (on.isClosed()) page = context.pages().slice(-1)[0] || (await context.newPage());
    }
  }
}

main().catch((e) => {
  emit({ e: 'error', message: String((e && e.message) || e).split('\n')[0] });
  process.exit(1);
});
