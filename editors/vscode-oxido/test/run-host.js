const path = require('node:path');
const { spawn } = require('node:child_process');
const { downloadAndUnzipVSCode } = require('@vscode/test-electron');

const extensionRoot = path.resolve(__dirname, '..');
const testRunner = path.join(__dirname, 'host', 'runner.js');

async function runTrustCase(label, fixture, launchArgs, expectedTrust) {
  process.stdout.write(`VS Code host test: ${label}\n`);
  const executable = process.env.VSCODE_EXECUTABLE_PATH
    || await downloadAndUnzipVSCode({ version: '1.143.1' });
  const profile = path.join(extensionRoot, '.vscode-test', label.replaceAll(' ', '-'));
  const args = [
    fixture,
    '--no-sandbox',
    '--disable-gpu',
    '--disable-gpu-sandbox',
    '--disable-updates',
    '--skip-welcome',
    '--skip-release-notes',
    '--no-cached-data',
    `--user-data-dir=${path.join(profile, 'user-data')}`,
    `--extensions-dir=${path.join(profile, 'extensions')}`,
    `--extensionTestsPath=${testRunner}`,
    `--extensionDevelopmentPath=${extensionRoot}`,
    ...launchArgs
  ];
  await new Promise((resolve, reject) => {
    const child = spawn(executable, args, {
      stdio: 'inherit',
      shell: false,
      env: { ...process.env, OXIDO_EXPECT_TRUSTED: String(expectedTrust) }
    });
    const timeout = setTimeout(() => {
      child.kill('SIGTERM');
      reject(new Error(`VS Code extension host timed out in ${label}`));
    }, 120_000);
    child.once('error', error => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once('close', (code, signal) => {
      clearTimeout(timeout);
      if (code === 0) resolve();
      else reject(new Error(`VS Code extension host exited with ${code ?? signal} in ${label}`));
    });
  });
}

async function main() {
  await runTrustCase(
    'trusted workspace',
    path.join(__dirname, 'fixtures', 'trusted'),
    ['--disable-workspace-trust'],
    true
  );
}

main().catch(error => {
  process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
  process.exitCode = 1;
});
