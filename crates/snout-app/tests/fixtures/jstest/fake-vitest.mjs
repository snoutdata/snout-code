// A stand-in for Vitest's program (`vitest.mjs`) in SnoutCode's tests: `run --reporter=…`
// loads the reporters given that are files (SnoutCode's own) and plays a run through
// Vitest 3's reporter API: one file, `src/cart.test.ts`, with a passing test and a failing
// one in `describe('cart')`. What it was given goes into `vitest-args.json` beside the
// package.

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const args = process.argv.slice(2);
fs.writeFileSync(path.join(process.cwd(), 'vitest-args.json'), JSON.stringify(args));
const files = args.filter((a) => a.startsWith('--reporter=')).map((a) => a.slice('--reporter='.length)).filter((r) => r.includes('/') || r.includes('\\'));
const reporters = [];
for (const f of files) reporters.push(new (await import(pathToFileURL(f).href)).default());
const call = (name, ...a) => reporters.forEach((r) => r[name] && r[name](...a));

const root = process.cwd();
const file = path.join(root, 'src', 'cart.test.ts');
const project = { name: '' };
const config = { root };
const suite = { type: 'suite', name: 'cart' };
const test = (id, name, line, state, errors) => ({
  id,
  name,
  location: config.includeTaskLocation ? { line, column: 3 } : undefined,
  parent: suite,
  project,
  options: { mode: 'run' },
  result: () => ({ state, errors }),
  diagnostic: () => ({ duration: 12, retryCount: 0, flaky: false }),
});
call('onInit', { version: '3.2.7', config, projects: [] });
const tests = [
  test('f_0_0', 'adds items', 4, 'passed', []),
  test('f_0_1', 'totals', 8, 'failed', [{ message: 'expected 40 to be 42', stack: 'AssertionError: expected 40 to be 42', stacks: [{ file, line: 9, column: 20 }] }]),
];
const module = { moduleId: file, children: { allTests: function* () { yield* tests; } }, errors: () => [] };
console.log(' RUN  v3.2.7 ' + root);
call('onTestModuleCollected', module);
for (const t of tests) {
  call('onTestCaseReady', t);
  call('onTestCaseResult', t);
}
call('onTestRunEnd', [module], [], 'failed');
process.exitCode = 1;
