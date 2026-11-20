// SnoutCode's Cypress reporter: what `cypress run` does, as it does it, one JSON object a
// line on stdout after a marker (U+001E, then "snout:"), in the protocol of SnoutCode's
// Playwright reporter, so the Tests window follows a Cypress run as it follows a Playwright
// one. Cypress makes a reporter for each spec, all in one Node process: the run begins with
// the first, and ends when the process does. A test's id is its file and full title.
// SnoutCode writes this file into its data folder and passes it to `cypress run --reporter`;
// in place of Cypress's own `spec` reporter, it prints a line per test too.

'use strict';

const path = require('path');

const MARK = '\u001esnout:';
const shared = (globalThis.__snoutCypress = globalThis.__snoutCypress || { begun: false });

function emit(event) {
  try {
    process.stdout.write(MARK + JSON.stringify(event) + '\n');
  } catch (_) {
    // A closed stdout (the app went away) ends nothing here.
  }
}

function say(line) {
  try {
    process.stdout.write(line + '\n');
  } catch (_) {}
}

// The folder spec paths are relative to: the project's, as SnoutCode started the run in.
const root = process.env.SNOUT_CYPRESS_ROOT || process.cwd();

function titles(runnable) {
  const out = [];
  for (let r = runnable; r && !r.root; r = r.parent) out.unshift(r.title);
  return out;
}

function fileOf(runnable, spec) {
  const d = runnable && runnable.invocationDetails;
  if (d && d.absoluteFile) return d.absoluteFile;
  if (d && d.relativeFile) return path.resolve(root, d.relativeFile);
  return spec ? path.resolve(root, spec) : '';
}

function failure(err, spec) {
  if (!err) return { message: '', stack: '', snippet: '' };
  const frame = err.codeFrame || {};
  const out = {
    message: String(err.message || err),
    stack: String(err.stack || ''),
    snippet: String(frame.frame || ''),
  };
  if (frame.absoluteFile || frame.relativeFile) {
    out.file = frame.absoluteFile || path.resolve(root, frame.relativeFile);
    if (frame.line) out.line = Number(frame.line);
    if (frame.column) out.column = Number(frame.column);
  } else if (spec) {
    out.file = path.resolve(root, spec);
  }
  return out;
}

function ms(n) {
  return n >= 1000 ? (n / 1000).toFixed(1) + 's' : n + 'ms';
}

class SnoutCypressReporter {
  constructor(runner) {
    const project = process.env.SNOUT_CYPRESS_BROWSER || 'Electron';
    this.spec = runner.suite && runner.suite.file;
    this.started = new Set();
    this.ended = new Set();
    this.ids = new Map();

    runner.on('start', () => {
      if (!shared.begun) {
        shared.begun = true;
        emit({ e: 'begin', version: '', rootDir: root, workers: 1, projects: [{ name: project }], tests: [] });
      }
      const tests = [];
      const walk = (suite) => {
        for (const t of suite.tests || []) tests.push(this.describe(t, project));
        for (const s of suite.suites || []) walk(s);
      };
      if (runner.suite) walk(runner.suite);
      if (tests.length) emit({ e: 'tests', tests });
      if (this.spec) say('\n  ' + this.spec);
    });

    runner.on('test', (t) => this.begin(t));
    // What a retry hears is a bare copy of the test (no id, no `describe`s): the one running.
    runner.on('retry', (t, err) => this.current && this.end(this.current, 'failed', err || (t && t.err), true));
    runner.on('test end', (t) => this.end(t, t.state === 'failed' ? 'failed' : t.state === 'pending' ? 'skipped' : 'passed', t.err));
    runner.on('pending', (t) => this.end(t, 'skipped', null));
    runner.on('fail', (r, err) => {
      // A hook that failed takes its tests with it, and has no test end of its own.
      if (r && r.type === 'hook') {
        const e = failure(err || r.err, this.spec);
        e.message = r.title + ': ' + e.message;
        emit({ e: 'error', error: e });
        say('    ✗ ' + r.title);
      }
    });
  }

  // By Cypress's own id of the test (`r3`): a retry is a copy of the test without its
  // `describe`s.
  id(t) {
    const key = t.id || t;
    if (!this.ids.has(key)) this.ids.set(key, { id: fileOf(t, this.spec) + '#' + titles(t).join(' '), depth: titles(t).length });
    return this.ids.get(key).id;
  }

  describe(t, project) {
    const d = t.invocationDetails || {};
    return {
      id: this.id(t),
      project,
      file: fileOf(t, this.spec),
      line: d.line || 0,
      column: d.column != null ? d.column + 1 : 0,
      path: titles(t),
      tags: [],
      expected: t.pending ? 'skipped' : 'passed',
      retries: t._retries > 0 ? t._retries : 0,
    };
  }

  retry(t) {
    return t._currentRetry > 0 ? t._currentRetry : 0;
  }

  begin(t) {
    this.current = t;
    const key = this.id(t) + '@' + this.retry(t);
    if (this.started.has(key)) return;
    this.started.add(key);
    emit({ e: 'testBegin', id: this.id(t), retry: this.retry(t), worker: 0, start: Date.now() });
  }

  end(t, status, err, again) {
    const key = this.id(t) + '@' + this.retry(t);
    if (this.ended.has(key)) return;
    this.begin(t);
    this.ended.add(key);
    const retry = this.retry(t);
    const outcome =
      status === 'skipped' ? 'skipped' : status === 'failed' ? 'unexpected' : retry > 0 ? 'flaky' : 'expected';
    emit({
      e: 'testEnd',
      id: this.id(t),
      retry,
      worker: 0,
      status,
      outcome,
      duration: t.duration || 0,
      errors: status === 'failed' ? [failure(err || t.err, this.spec)] : [],
      attachments: [],
      steps: [],
    });
    if (again) return;
    const indent = '    ' + '  '.repeat(Math.max(0, this.ids.get(t.id || t).depth - 1));
    if (status === 'passed') say(indent + '✓ ' + t.title + (t.duration ? ' (' + ms(t.duration) + ')' : ''));
    else if (status === 'failed') say(indent + '✗ ' + t.title);
    else say(indent + '- ' + t.title);
  }
}

module.exports = SnoutCypressReporter;
