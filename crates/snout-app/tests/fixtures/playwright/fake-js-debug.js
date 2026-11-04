// A stand-in for js-debug's DAP server (dapDebugServer.js) in SnoutCode's tests. It says its
// port as the real one does, and plays what js-debug does for a Node program: the first
// connection's `launch` asks the client to start the program (`runInTerminal`, with a
// bootloader of its own, which does nothing, in NODE_OPTIONS), then asks for a session for the program's worker
// (`startDebugging`); on that session's connection it stops at the first breakpoint it was
// given, shows a frame there with one variable, and once continued ends the session. The
// program (the fake cli.js) waits for the worker's session to let go before it runs: this
// server writes `released` beside itself then. What each connection said goes into
// `js-debug.log` beside it, a JSON line each.

'use strict';

const net = require('net');
const fs = require('fs');
const path = require('path');

const here = __dirname;
const hold = path.join(here, 'released');
const bootloader = path.join(here, 'bootloader.js');
fs.writeFileSync(bootloader, '// The real one connects the process to the debugger.\n');
const log = (m) => fs.appendFileSync(path.join(here, 'js-debug.log'), JSON.stringify(m) + '\n');
let root = null;
let seq = 1000;

function connection(sock) {
  let buf = Buffer.alloc(0);
  const c = {
    role: root ? 'worker' : 'launch',
    bps: [],
    send(m) {
      m.seq = ++seq;
      const body = Buffer.from(JSON.stringify(m));
      sock.write('Content-Length: ' + body.length + '\r\n\r\n');
      sock.write(body);
    },
    end() {
      setTimeout(() => sock.end(), 10);
    },
  };
  if (!root) root = c;
  sock.on('error', () => {});
  sock.on('data', (d) => {
    buf = Buffer.concat([buf, d]);
    for (;;) {
      const i = buf.indexOf('\r\n\r\n');
      if (i < 0) return;
      const n = Number(/Content-Length: (\d+)/i.exec(buf.slice(0, i).toString())[1]);
      if (buf.length < i + 4 + n) return;
      const m = JSON.parse(buf.slice(i + 4, i + 4 + n).toString());
      buf = buf.slice(i + 4 + n);
      on(c, m);
    }
  });
}

function reply(c, m, body) {
  c.send({ type: 'response', request_seq: m.seq, command: m.command, success: true, body });
}

function stop(c) {
  if (c.role === 'worker' && c.launched && c.configured && !c.stopped && c.bps.length) {
    c.stopped = true;
    c.send({ type: 'event', event: 'stopped', body: { reason: 'breakpoint', threadId: 7 } });
  }
}

function on(c, m) {
  log({ role: c.role, type: m.type, command: m.command, arguments: m.arguments, success: m.success, body: m.body });
  if (m.type === 'response') {
    if (m.command === 'runInTerminal') {
      reply(c, c.launch);
      c.send({
        type: 'request',
        command: 'startDebugging',
        arguments: { request: 'launch', configuration: { type: 'pwa-node', name: 'worker', __pendingTargetId: 'w1' } },
      });
    }
    return;
  }
  const a = m.arguments || {};
  switch (m.command) {
    case 'initialize':
      reply(c, m, { supportsConfigurationDoneRequest: true });
      c.send({ type: 'event', event: 'initialized' });
      break;
    case 'setBreakpoints': {
      const file = a.source.path;
      c.bps = c.bps.filter((b) => b.path !== file).concat((a.breakpoints || []).map((b) => ({ path: file, line: b.line })));
      reply(c, m, { breakpoints: (a.breakpoints || []).map(() => ({ verified: true })) });
      stop(c);
      break;
    }
    case 'launch':
      if (c.role === 'launch') {
        c.launch = m;
        c.send({
          type: 'request',
          command: 'runInTerminal',
          arguments: {
            kind: 'integrated',
            cwd: a.cwd,
            args: [a.runtimeExecutable].concat(a.runtimeArgs || []),
            env: { NODE_OPTIONS: '--require ' + JSON.stringify(bootloader), FAKE_DEBUG_HOLD: hold },
          },
        });
      } else {
        reply(c, m);
        c.launched = true;
        stop(c);
      }
      break;
    case 'configurationDone':
      reply(c, m);
      c.configured = true;
      stop(c);
      break;
    case 'threads':
      reply(c, m, { threads: [{ id: 7, name: 'worker' }] });
      break;
    case 'stackTrace': {
      const b = c.bps[0];
      reply(c, m, {
        stackFrames: [{ id: 1, name: 'buys', line: b.line, column: 3, source: { path: b.path, name: path.basename(b.path) } }],
      });
      break;
    }
    case 'scopes':
      reply(c, m, { scopes: [{ name: 'Local', variablesReference: 5 }] });
      break;
    case 'variables':
      reply(c, m, { variables: [{ name: 'total', value: '6', type: 'number', variablesReference: 0 }] });
      break;
    case 'continue':
      reply(c, m, { allThreadsContinued: false });
      c.send({ type: 'event', event: 'continued', body: { threadId: 7 } });
      c.send({ type: 'event', event: 'terminated', body: {} });
      break;
    case 'disconnect':
      reply(c, m);
      if (c.role === 'worker') fs.writeFileSync(hold, '');
      c.end();
      break;
    default:
      reply(c, m);
  }
}

const server = net.createServer(connection);
server.listen(0, '127.0.0.1', () => {
  process.stdout.write('Debug server listening at 127.0.0.1:' + server.address().port + '\n');
});
