const assert = require('node:assert/strict');
const test = require('node:test');
const { downloadVSCodeWithRetry } = require('./vscode-download');

function networkTimeout() {
  const timeout = new Error('connection timed out');
  timeout.code = 'ETIMEDOUT';
  return new AggregateError([timeout], 'all connection attempts failed');
}

test('retries transient network failures and returns the downloaded executable', async () => {
  let attempts = 0;
  const retries = [];
  const delays = [];
  const executable = await downloadVSCodeWithRetry(
    async () => {
      attempts += 1;
      if (attempts < 3) throw networkTimeout();
      return '/tmp/vscode/code';
    },
    {
      delayMs: 10,
      onRetry: (attempt, total) => retries.push([attempt, total]),
      waitFor: async delay => delays.push(delay)
    }
  );

  assert.equal(executable, '/tmp/vscode/code');
  assert.equal(attempts, 3);
  assert.deepEqual(retries, [[2, 3], [3, 3]]);
  assert.deepEqual(delays, [10, 20]);
});

test('does not retry non-network failures', async () => {
  const failure = new Error('invalid VS Code version');
  let attempts = 0;

  await assert.rejects(
    downloadVSCodeWithRetry(
      async () => {
        attempts += 1;
        throw failure;
      },
      {
        onRetry: () => assert.fail('no debe reintentar un error permanente'),
        waitFor: async () => assert.fail('no debe esperar ante un error permanente')
      }
    ),
    error => error === failure
  );
  assert.equal(attempts, 1);
});

test('returns the last network error after the bounded retries', async () => {
  let attempts = 0;
  const retries = [];
  const failure = networkTimeout();

  await assert.rejects(
    downloadVSCodeWithRetry(
      async () => {
        attempts += 1;
        throw failure;
      },
      {
        attempts: 2,
        delayMs: 1,
        onRetry: (attempt, total) => retries.push([attempt, total]),
        waitFor: async () => {}
      }
    ),
    error => error === failure
  );
  assert.equal(attempts, 2);
  assert.deepEqual(retries, [[2, 2]]);
});
