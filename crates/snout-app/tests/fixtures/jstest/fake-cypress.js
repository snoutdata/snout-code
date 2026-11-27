#!/usr/bin/env node
// A stand-in for `cypress run`, for the tests: it keeps its arguments in cypress-args.json,
// then runs each spec as Cypress 13 does for a reporter: one reporter per spec, all in this
// process, with the runner's events and test objects shaped as Cypress makes them (ids,
// `invocationDetails`, a failure's `codeFrame`). The specs are read by line: `describe('x'`
// opens a group, `it('y'` is a test, which fails when its title has "fails" in it.
// FAKE_CYPRESS_CRASH=1 ends the run as an old Cypress does on a new macOS, and
// FAKE_CYPRESS_NO_APP=1 as Cypress does without its app, before any spec. `cypress install`
// only says so: the app it would download is not needed here.

'use strict';

const fs = require('fs');
const path = require('path');
const { EventEmitter } = require('events');

const args = process.argv.slice(2);
fs.writeFileSync(path.join(process.cwd(), 'cypress-args.json'), JSON.stringify(args));
if (args[0] === 'install') {
  console.log('Installing Cypress');
  process.exit(0);
}
if (process.env.FAKE_CYPRESS_NO_APP === '1') {
  console.error('No version of Cypress is installed in: /home/qa/.cache/Cypress/13.6.4/Cypress');
  console.error('');
  console.error('Please reinstall Cypress by running: cypress install');
  process.exit(1);
}
const flag = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const Reporter = require(flag('--reporter'));

function walk(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) => {
    const p = path.join(dir, e.name);
    return e.isDirectory() ? walk(p) : /\.cy\.[jt]sx?$/.test(e.name) ? [p] : [];
  });
}

const specs = flag('--spec')
  ? flag('--spec').split(',').map((s) => path.resolve(s))
  : walk(path.resolve('cypress/e2e')).sort();

let ids = 0;
function parse(file) {
  const rel = path.relative(process.cwd(), file);
  const root = { root: true, title: '', file: rel, suites: [], tests: [], id: 'r' + ids++ };
  const stack = [{ suite: root, indent: -1 }];
  fs.readFileSync(file, 'utf8')
    .split('\n')
    .forEach((line, i) => {
      const m = /^(\s*)(describe|context|it)\((['"])(.*?)\3/.exec(line);
      if (!m) return;
      const indent = m[1].length;
      while (stack[stack.length - 1].indent >= indent) stack.pop();
      const parent = stack[stack.length - 1].suite;
      const details = { absoluteFile: file, relativeFile: rel, line: i + 1, column: indent };
      if (m[2] === 'it') {
        parent.tests.push({ type: 'test', title: m[4], parent, id: 'r' + ids++, invocationDetails: details, _currentRetry: 0 });
      } else {
        const suite = { title: m[4], parent, suites: [], tests: [], id: 'r' + ids++, invocationDetails: details };
        parent.suites.push(suite);
        stack.push({ suite, indent });
      }
    });
  return root;
}

let failed = 0;
for (const file of specs) {
  const root = parse(file);
  const runner = new EventEmitter();
  runner.suite = root;
  new Reporter(runner, { reporterOptions: {} });
  runner.emit('start');
  const run = (suite) => {
    runner.emit('suite', suite);
    for (const t of suite.tests) {
      runner.emit('test', t);
      t.duration = 5;
      if (/fails/.test(t.title)) {
        failed += 1;
        t.state = 'failed';
        t.err = {
          message: 'expected 4 to equal 5',
          stack: 'AssertionError: expected 4 to equal 5\n    at Context.eval (webpack://shop/./' + root.file + ':' + t.invocationDetails.line + ':10)',
          codeFrame: {
            absoluteFile: file,
            relativeFile: root.file,
            line: t.invocationDetails.line,
            column: 10,
            frame: '> ' + t.invocationDetails.line + ' |   it(\'' + t.title + '\')',
          },
        };
        runner.emit('fail', t, t.err);
      } else {
        t.state = 'passed';
        runner.emit('pass', t);
      }
      runner.emit('test end', t);
    }
    for (const s of suite.suites) run(s);
    runner.emit('suite end', suite);
  };
  run(root);
  runner.emit('end');
}
console.log('  (Run Finished)');
if (process.env.FAKE_CYPRESS_CRASH === '1') {
  console.error('The Test Runner unexpectedly exited via a exit event with signal SIGSEGV');
  process.exit(1);
}
process.exit(failed > 0 ? 1 : 0);
