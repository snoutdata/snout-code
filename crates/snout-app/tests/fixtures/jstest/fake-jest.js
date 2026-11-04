// A stand-in for Jest's program (`bin/jest.js`) in SnoutCode's tests: loads the
// `--reporters=` given that are files (SnoutCode's own) and plays a run through Jest 29's
// reporter API: `math.test.js` with a passing test, a failing one in `describe('math')`, and
// a skipped one with no case events. What it was given goes into `jest-args.json` beside
// the package.

'use strict';

const fs = require('fs');
const path = require('path');

const args = process.argv.slice(2);
fs.writeFileSync(path.join(process.cwd(), 'jest-args.json'), JSON.stringify(args));
const files = args.filter((a) => a.startsWith('--reporters=')).map((a) => a.slice('--reporters='.length)).filter((r) => r.includes('/') || r.includes('\\'));
const rootDir = process.cwd();
const reporters = files.map((f) => new (require(f))({ rootDir, maxWorkers: 1 }));
const call = (name, ...a) => reporters.forEach((r) => r[name] && r[name](...a));

const file = path.join(rootDir, 'math.test.js');
const test = { path: file, context: { config: { rootDir } } };
const adds = { ancestorTitles: ['math'], title: 'adds', fullName: 'math adds', status: 'passed', duration: 3, failureMessages: [], location: { line: 2, column: 3 } };
const fails = {
  ancestorTitles: ['math'],
  title: 'divides',
  fullName: 'math divides',
  status: 'failed',
  duration: 5,
  failureMessages: ['Error: expect(received).toBe(expected)\n\nExpected: 3\nReceived: 2\n    at Object.toBe (' + file + ':6:19)'],
  location: { line: 5, column: 3 },
};
const later = { ancestorTitles: [], title: 'later', fullName: 'later', status: 'pending', duration: null, failureMessages: [], location: { line: 10, column: 6 } };
call('onRunStart', {}, {});
call('onTestFileStart', test);
for (const r of [adds, fails]) {
  call('onTestCaseStart', test, { ancestorTitles: r.ancestorTitles, title: r.title, fullName: r.fullName, startedAt: Date.now() });
  call('onTestCaseResult', test, r);
}
console.log('FAIL ./math.test.js');
call('onTestResult', test, { testResults: [adds, fails, later] });
call('onRunComplete', new Set(), { numFailedTests: 1, numRuntimeErrorTestSuites: 0, startTime: Date.now() - 20 });
process.exitCode = 1;
