// A stand-in for `@playwright/test`'s API in SnoutCode's tests, as the locator picker uses
// it: a browser whose page "picks" two elements, the second a while after the first, then
// waits; what the picker opened goes into `picker-opened.json` in the project's folder.

'use strict';

const fs = require('fs');
const path = require('path');

const picks = ["getByRole('button', { name: 'Log in' })", "getByTestId('cart')"];
const opened = {};

function keep() {
  fs.writeFileSync(path.join(process.cwd(), 'picker-opened.json'), JSON.stringify(opened));
}

const page = {
  async goto(url) {
    opened.url = url;
    keep();
  },
  async pickLocator() {
    if (!picks.length) return new Promise(() => {});
    await new Promise((r) => setTimeout(r, picks.length === 2 ? 300 : 1500));
    const locator = picks.shift();
    return { toString: () => locator };
  },
  isClosed: () => false,
};

const context = { newPage: async () => page, on() {}, pages: () => [page] };

function browserType(name) {
  return {
    async launch(options) {
      opened.browser = name;
      opened.headless = options.headless;
      keep();
      return {
        on() {},
        isConnected: () => true,
        close: async () => {},
        async newContext(options) {
          opened.storage = options.storageState || null;
          keep();
          return context;
        },
      };
    },
  };
}

module.exports = {
  chromium: browserType('chromium'),
  firefox: browserType('firefox'),
  webkit: browserType('webkit'),
  selectors: {
    setTestIdAttribute(a) {
      opened.testIdAttribute = a;
      keep();
    },
  },
};
