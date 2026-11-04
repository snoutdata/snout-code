// SnoutCode's Jest reporter: what the run does, as it does it, one JSON object a line on
// stdout after a marker (U+001E, then "snout:"), in the protocol of SnoutCode's Playwright
// reporter, so the Tests window follows a Jest run as it follows a Playwright one. Jest
// names no tests before it runs a file, so each comes in a `tests` event as it starts (or,
// on a Jest without `onTestCaseStart`, as its result comes). A test's id is its file and
// full name. SnoutCode writes this file into its data folder and passes it to
// `jest --reporters`; Jest's own reporter runs beside it.

'use strict';

const MARK = '\u001esnout:';

function emit(event) {
  try {
    process.stdout.write(MARK + JSON.stringify(event) + '\n');
  } catch (_) {
    // A closed stdout (the app went away) ends nothing here.
  }
}

// Where a failure message points: its first stack frame in the test file.
function where(message, file) {
  const lines = String(message).split('\n');
  for (const l of lines) {
    const m = /\(?([^\s()]+):(\d+):(\d+)\)?\s*$/.exec(l.trim());
    if (m && (!file || m[1] === file)) return { file: m[1], line: Number(m[2]), column: Number(m[3]) };
  }
  return {};
}

function failure(message, file) {
  const text = String(message);
  const cut = text.search(/\n\s+at /);
  return Object.assign(
    { message: cut > 0 ? text.slice(0, cut) : text, stack: cut > 0 ? text.slice(cut + 1) : '', snippet: '' },
    where(text, file),
  );
}

function project(test) {
  const c = test && test.context && test.context.config;
  const name = c && c.displayName;
  return (name && (name.name || name)) || 'Jest';
}

class SnoutJestReporter {
  constructor(globalConfig) {
    this.config = globalConfig || {};
    this.known = new Set();
    this.started = new Set();
    this.done = new Set();
    this.located = new Set();
  }

  onRunStart() {
    emit({
      e: 'begin',
      version: '',
      rootDir: this.config.rootDir,
      workers: this.config.maxWorkers || 0,
      projects: [{ name: 'Jest' }],
      tests: [],
    });
  }

  id(file, ancestors, title) {
    return file + '#' + ancestors.concat([title]).join(' ');
  }

  announce(test, info) {
    const file = test.path;
    const id = this.id(file, info.ancestorTitles || [], info.title);
    // Known from its start, which has no location: its result has one.
    const located = info.location && !this.located.has(id);
    if (!this.known.has(id) || located) {
      this.known.add(id);
      if (info.location) this.located.add(id);
      emit({
        e: 'tests',
        tests: [
          {
            id,
            project: project(test),
            file,
            line: (info.location && info.location.line) || 0,
            column: (info.location && info.location.column) || 0,
            path: (info.ancestorTitles || []).concat([info.title]),
            tags: [],
            expected: 'passed',
            retries: 0,
          },
        ],
      });
    }
    return id;
  }

  onTestCaseStart(test, info) {
    const id = this.announce(test, info);
    this.started.add(id);
    emit({ e: 'testBegin', id, retry: 0, start: info.startedAt || Date.now() });
  }

  onTestCaseResult(test, r) {
    this.caseEvents = true;
    this.result(test, r);
  }

  result(test, r) {
    const id = this.announce(test, r);
    this.done.add(id);
    if (!this.started.has(id)) emit({ e: 'testBegin', id, retry: 0, start: Date.now() });
    const status = r.status === 'passed' ? 'passed' : r.status === 'failed' ? 'failed' : 'skipped';
    const retries = (r.retryReasons && r.retryReasons.length) || 0;
    const outcome =
      status === 'failed' ? 'unexpected' : status === 'passed' ? (retries > 0 ? 'flaky' : 'expected') : 'skipped';
    emit({
      e: 'testEnd',
      id,
      retry: retries,
      status,
      outcome,
      duration: r.duration || 0,
      errors: (r.failureMessages || []).map((m) => failure(m, test.path)),
      attachments: [],
      steps: [],
    });
  }

  // The file is done: its results all come here, those with no case event of their own
  // among them (skipped tests, and every test on a Jest before 29).
  onTestResult(test, result) {
    for (const r of result.testResults || []) {
      if (!this.done.has(this.id(test.path, r.ancestorTitles || [], r.title))) this.result(test, r);
    }
    if (result.testExecError) emit({ e: 'error', error: failure(result.testExecError.message, test.path) });
  }

  onRunComplete(contexts, results) {
    const failed = results && (results.numFailedTests > 0 || results.numRuntimeErrorTestSuites > 0);
    emit({
      e: 'end',
      status: results && results.wasInterrupted ? 'interrupted' : failed ? 'failed' : 'passed',
      duration: results && results.startTime ? Date.now() - results.startTime : 0,
    });
  }
}

module.exports = SnoutJestReporter;
