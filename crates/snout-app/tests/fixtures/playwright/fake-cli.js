// A stand-in for `@playwright/test`'s cli.js in SnoutCode's tests: `test [--list]
// --reporter=<file>,list …` loads the reporter given (SnoutCode's own) and plays a small
// run through it, as Playwright calls a reporter: four tests in two projects, one failing
// with a trace and a screenshot comparison, one flaky. The attachments are the files the
// test put beside this one (`trace.zip`, `home-*.png`).

'use strict';

const path = require('path');
const fs = require('fs');

const args = process.argv.slice(2);

// `codegen --output=<file> … <url>`: the recorder rewrites its file as steps come, and runs
// until stopped (its browser stays open). What it was given goes beside the file.
if (args[0] === 'codegen') {
  const out = args.find((a) => a.startsWith('--output=')).slice('--output='.length);
  fs.writeFileSync(out + '.args.json', JSON.stringify(args));
  const url = args.slice(1).filter((a) => !a.startsWith('-')).pop() || 'about:blank';
  const steps = [
    `  await page.goto('${url}/');`,
    "  await page.getByRole('link', { name: 'Shop' }).click();",
    "  await page.getByLabel('Email').fill('qa@shop.test');",
  ];
  let n = 0;
  const write = () => {
    n += 1;
    const body = steps.slice(0, n).join('\n');
    fs.writeFileSync(out, "import { test, expect } from '@playwright/test';\n\ntest('test', async ({ page }) => {\n" + body + '\n});');
    if (n < steps.length) setTimeout(write, 300);
  };
  write();
  process.on('SIGTERM', () => process.exit(0));
  setInterval(() => {}, 1000);
  return;
}

// `test --ui --ui-port=0`: UI Mode serves until stopped.
if (args.includes('--ui')) {
  process.stdout.write('Listening on http://127.0.0.1:52123\n');
  process.on('SIGTERM', () => process.exit(0));
  setInterval(() => {}, 1000);
  return;
}
const reporterArg = args.find((a) => a.startsWith('--reporter='));
const Reporter = require(reporterArg.slice('--reporter='.length).split(',')[0]);
const reporter = new Reporter();
const list = args.includes('--list');
const here = __dirname;
const file = path.join(process.cwd(), 'tests', 'shop.spec.ts');

function test(id, project, title, line) {
  return {
    id,
    title,
    location: { file, line, column: 3 },
    titlePath: () => ['', project, 'shop.spec.ts', 'shop', title],
    parent: { project: () => ({ name: project }) },
    tags: [],
    expectedStatus: 'passed',
    retries: 1,
    outcome() {
      return this._outcome;
    },
  };
}

const tests = [
  test('a1', 'chromium', 'signs in', 3),
  test('a2', 'firefox', 'signs in', 3),
  test('b1', 'chromium', 'pays by card', 7),
  test('b2', 'firefox', 'pays by card', 7),
];
let only = args.find((a) => a.startsWith('--project='));
const shown = only ? tests.filter((t) => t.parent.project().name === only.slice(10)) : tests;

reporter.onBegin(
  {
    version: '1.64.1',
    rootDir: process.cwd(),
    workers: 2,
    projects: [
      {
        name: 'chromium',
        use: { browserName: 'chromium', baseURL: 'https://shop.test', testIdAttribute: 'data-qa', storageState: '.auth/user.json' },
      },
      { name: 'firefox', use: { browserName: 'firefox' } },
    ],
  },
  { allTests: () => shown },
);

function play(t, retry, worker, status, outcome, extra) {
  const result = Object.assign(
    { retry, parallelIndex: worker, status, duration: 1200, errors: [], attachments: [], steps: [] },
    extra || {},
  );
  reporter.onTestBegin(t, result);
  reporter.onStepBegin(t, result, { category: 'pw:api', title: "page.goto('/')" });
  reporter.onStdOut('hello from ' + t.title + '\n', t, result);
  result.steps = [
    { title: "page.goto('/')", category: 'pw:api', duration: 300, location: { line: t.location.line + 1 }, steps: [] },
    {
      title: 'Expect "toHaveScreenshot"',
      category: 'expect',
      duration: 500,
      error: status === 'failed' ? { message: 'Screenshot differs' } : undefined,
      location: { line: t.location.line + 2 },
      steps: [],
    },
  ];
  t._outcome = outcome;
  process.stdout.write('  ' + (status === 'passed' ? '✓' : '✘') + ' [' + t.parent.project().name + '] › ' + t.title + '\n');
  reporter.onTestEnd(t, result);
}

// Under the fake js-debug: what the run was started with goes beside this file, and the
// run waits until the debugger has let go of its worker.
if (process.env.FAKE_DEBUG_HOLD && !list) {
  fs.writeFileSync(path.join(here, 'debugged.json'), JSON.stringify({ NODE_OPTIONS: process.env.NODE_OPTIONS, args }));
  const sleep = new Int32Array(new SharedArrayBuffer(4));
  const until = Date.now() + 20000;
  while (!fs.existsSync(process.env.FAKE_DEBUG_HOLD) && Date.now() < until) Atomics.wait(sleep, 0, 0, 50);
}

if (!list) {
  for (const t of shown) {
    if (t.id === 'b1') {
      play(t, 0, 0, 'failed', 'unexpected', {
        errors: [
          {
            message: 'Error: \u001b[31mexpect(page).toHaveScreenshot(expected)\u001b[39m failed\n\n  555 pixels are different.',
            stack: 'Error: …\n    at ' + file + ':9:22',
            snippet: '   8 |   await page.getByRole("button").click();\n>  9 |   await expect(page).toHaveScreenshot("home.png");\n     |                      ^',
            location: { file, line: 9, column: 22 },
          },
        ],
        attachments: [
          { name: 'home-expected.png', contentType: 'image/png', path: path.join(here, 'home-expected.png') },
          { name: 'home-actual.png', contentType: 'image/png', path: path.join(here, 'home-actual.png') },
          { name: 'home-diff.png', contentType: 'image/png', path: path.join(here, 'home-diff.png') },
          { name: 'trace', contentType: 'application/zip', path: path.join(here, 'trace.zip') },
          { name: 'video', contentType: 'video/webm', path: path.join(here, 'video.webm') },
        ],
      });
    } else if (t.id === 'a2') {
      play(t, 0, 1, 'failed', 'unexpected', { errors: [{ message: 'Timed out' }] });
      play(t, 1, 1, 'passed', 'flaky');
    } else {
      play(t, 0, t.id === 'a1' ? 0 : 1, 'passed', 'expected');
    }
  }
}
reporter.onEnd({ status: list ? 'passed' : 'failed', duration: 4321 });
process.exit(list ? 0 : 1);
