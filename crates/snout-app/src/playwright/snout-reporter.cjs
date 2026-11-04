// SnoutCode's Playwright reporter: what the run does, as it does it, one JSON object a line
// on stdout, each after a marker (U+001E, then "snout:") so the app can tell its lines from
// what the other reporters and the tests print. It prints nothing else; Playwright's own
// reporters run beside it. SnoutCode writes this file into its data folder and passes it to
// `playwright test --reporter`. It runs in the test runner's process, on the project's Node.

'use strict';

const MARK = '\u001esnout:';

// Steps worth showing: actions, assertions and the test's own `test.step`s.
const SHOWN = new Set(['pw:api', 'expect', 'test.step']);

function emit(event) {
  try {
    process.stdout.write(MARK + JSON.stringify(event) + '\n');
  } catch (_) {
    // A closed stdout (the app went away) ends nothing here.
  }
}

function where(location) {
  return location ? { file: location.file, line: location.line, column: location.column } : {};
}

function error(e) {
  return Object.assign(
    { message: e.message || e.value || '', stack: e.stack || '', snippet: e.snippet || '' },
    where(e.location),
  );
}

function steps(list, depth, out) {
  for (const s of list || []) {
    const shown = SHOWN.has(s.category);
    if (shown || s.category === 'hook') {
      out.push({
        title: s.title,
        category: s.category,
        duration: s.duration,
        error: s.error ? (s.error.message || '').split('\n')[0] : null,
        line: s.location ? s.location.line : null,
        depth,
      });
    }
    if (out.length < 2000) steps(s.steps, shown ? depth + 1 : depth, out);
  }
  return out;
}

class SnoutReporter {
  printsToStdio() {
    return false;
  }

  onBegin(config, suite) {
    const projects = (config.projects || []).map((p) => ({
      name: p.name,
      browser: (p.use && (p.use.browserName || p.use.defaultBrowserType)) || '',
    }));
    const tests = suite.allTests().map((t) => {
      const path = t.titlePath();
      // ['', project, file, ...describes, title]
      return {
        id: t.id,
        project: t.parent && t.parent.project() ? t.parent.project().name : '',
        file: t.location.file,
        line: t.location.line,
        column: t.location.column,
        path: path.slice(3),
        tags: t.tags || [],
        expected: t.expectedStatus,
        retries: t.retries,
      };
    });
    emit({
      e: 'begin',
      version: config.version,
      rootDir: config.rootDir,
      workers: config.workers,
      projects,
      tests,
    });
  }

  onTestBegin(test, result) {
    emit({ e: 'testBegin', id: test.id, retry: result.retry, worker: result.parallelIndex, start: Date.now() });
  }

  onStepBegin(test, result, step) {
    if (SHOWN.has(step.category)) {
      emit({ e: 'step', id: test.id, retry: result.retry, worker: result.parallelIndex, title: step.title });
    }
  }

  onStdOut(chunk, test, result) {
    if (test) emit({ e: 'out', id: test.id, retry: result ? result.retry : 0, text: String(chunk) });
  }

  onStdErr(chunk, test, result) {
    if (test) emit({ e: 'out', id: test.id, retry: result ? result.retry : 0, text: String(chunk), err: true });
  }

  onTestEnd(test, result) {
    emit({
      e: 'testEnd',
      id: test.id,
      retry: result.retry,
      worker: result.parallelIndex,
      status: result.status,
      outcome: test.outcome(),
      duration: result.duration,
      errors: (result.errors || []).map(error),
      attachments: (result.attachments || [])
        .filter((a) => a.path)
        .map((a) => ({ name: a.name, contentType: a.contentType, path: a.path })),
      steps: steps(result.steps, 0, []),
    });
  }

  onError(e) {
    emit({ e: 'error', error: error(e) });
  }

  onEnd(result) {
    emit({ e: 'end', status: result.status, duration: result.duration });
  }
}

module.exports = SnoutReporter;
