// SnoutCode's Vitest reporter: what the run does, as it does it, one JSON object a line on
// stdout after a marker (U+001E, then "snout:"), in the protocol of SnoutCode's Playwright
// reporter, so the Tests window follows a Vitest run as it follows a Playwright one. Vitest
// finds a file's tests as it loads the file, so they come in `tests` events as each file is
// collected. It uses the reporter API of Vitest 3 and later (`onTestCaseResult`), and reads
// the finished tree at the end on Vitest 2. SnoutCode writes this file into its data
// folder and passes it to `vitest run --reporter`; Vitest's own reporter runs beside it.

const MARK = '\u001esnout:';

function emit(event) {
  try {
    process.stdout.write(MARK + JSON.stringify(event) + '\n');
  } catch (_) {
    // A closed stdout (the app went away) ends nothing here.
  }
}

function error(e) {
  const at = (e && e.stacks && e.stacks[0]) || {};
  let message = (e && (e.message || String(e))) || '';
  if (e && e.diff) message += '\n\n' + e.diff;
  return { message, stack: (e && e.stack) || '', snippet: '', file: at.file, line: at.line, column: at.column };
}

// The titles of the suites around a test, outermost first, then its own.
function titles(task) {
  const out = [task.name];
  for (let s = task.parent || task.suite; s && s.type !== 'module' && s.name && !s.filepath; s = s.parent || s.suite) {
    out.unshift(s.name);
  }
  return out;
}

export default class SnoutVitestReporter {
  constructor() {
    this.seen = new Set();
    this.modern = false;
  }

  onInit(vitest) {
    this.vitest = vitest;
    // Each test's line and column, for the editor and the Tests window.
    const configs = [vitest.config].concat((vitest.projects || []).map((p) => p.config));
    for (const c of configs) if (c) c.includeTaskLocation = true;
    const projects = (vitest.projects || []).map((p) => ({ name: p.name || 'Vitest' }));
    emit({
      e: 'begin',
      version: vitest.version || '',
      rootDir: vitest.config && vitest.config.root,
      workers: 0,
      projects: projects.length ? projects : [{ name: 'Vitest' }],
      tests: [],
    });
  }

  // Vitest 3 and later.
  onTestModuleCollected(module) {
    this.modern = true;
    const tests = [];
    for (const t of module.children.allTests()) tests.push(this.test(t, module.moduleId));
    if (tests.length) emit({ e: 'tests', tests });
  }

  test(t, file) {
    this.seen.add(t.id);
    const mode = t.options && t.options.mode;
    return {
      id: t.id,
      project: (t.project && t.project.name) || 'Vitest',
      file,
      line: (t.location && t.location.line) || 0,
      column: (t.location && t.location.column) || 0,
      path: titles(t.task || t),
      tags: [],
      expected: mode === 'skip' || mode === 'todo' ? 'skipped' : 'passed',
      retries: (t.options && t.options.retry) || 0,
    };
  }

  onTestCaseReady(t) {
    emit({ e: 'testBegin', id: t.id, retry: 0, start: Date.now() });
  }

  onTestCaseResult(t) {
    const r = t.result() || {};
    const d = (t.diagnostic && t.diagnostic()) || {};
    const state = r.state;
    const status = state === 'passed' ? 'passed' : state === 'failed' ? 'failed' : 'skipped';
    const outcome =
      state === 'failed' ? 'unexpected' : state === 'passed' ? (d.flaky ? 'flaky' : 'expected') : 'skipped';
    emit({
      e: 'testEnd',
      id: t.id,
      retry: d.retryCount || 0,
      status,
      outcome,
      duration: d.duration || 0,
      errors: (r.errors || []).map(error),
      attachments: [],
      steps: [],
    });
  }

  onTestRunEnd(modules, errors, reason) {
    for (const e of errors || []) emit({ e: 'error', error: error(e) });
    for (const m of modules || []) {
      for (const e of (m.errors && m.errors()) || []) emit({ e: 'error', error: error(e) });
    }
    this.ended = true;
    emit({ e: 'end', status: reason === 'interrupted' ? 'interrupted' : reason || 'passed', duration: 0 });
  }

  // Vitest 2: the whole tree once the run is over.
  onFinished(files, errors) {
    if (this.modern || this.ended) return;
    const tests = [];
    const ends = [];
    const walk = (task, file) => {
      for (const c of task.tasks || []) {
        if (c.type === 'suite') {
          walk(c, file);
          continue;
        }
        tests.push({
          id: c.id,
          project: (c.file && c.file.projectName) || 'Vitest',
          file,
          line: (c.location && c.location.line) || 0,
          column: (c.location && c.location.column) || 0,
          path: titles(c),
          tags: [],
          expected: c.mode === 'skip' || c.mode === 'todo' ? 'skipped' : 'passed',
          retries: 0,
        });
        const state = c.result && c.result.state;
        ends.push({
          e: 'testEnd',
          id: c.id,
          retry: 0,
          status: state === 'pass' ? 'passed' : state === 'fail' ? 'failed' : 'skipped',
          outcome: state === 'pass' ? 'expected' : state === 'fail' ? 'unexpected' : 'skipped',
          duration: (c.result && c.result.duration) || 0,
          errors: ((c.result && c.result.errors) || []).map(error),
          attachments: [],
          steps: [],
        });
      }
    };
    for (const f of files || []) {
      walk(f, f.filepath);
      for (const e of (f.result && f.result.errors) || []) emit({ e: 'error', error: error(e) });
    }
    emit({ e: 'tests', tests });
    for (const e of ends) emit(e);
    for (const e of errors || []) emit({ e: 'error', error: error(e) });
    emit({ e: 'end', status: ends.some((e) => e.status === 'failed') ? 'failed' : 'passed', duration: 0 });
  }
}
