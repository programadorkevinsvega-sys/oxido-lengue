const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { downloadAndUnzipVSCode } = require('@vscode/test-electron');

const extensionRoot = path.resolve(__dirname, '..');
const repositoryRoot = path.resolve(extensionRoot, '..', '..');
const testRunner = path.join(__dirname, 'host', 'runner.js');

function prepareCargoHome(profile) {
  const vendor = path.join(repositoryRoot, 'tests', 'fixtures', 'cargo-registry', 'vendor');
  if (!fs.existsSync(path.join(vendor, 'itoa', 'Cargo.toml'))) {
    throw new Error(`el vendor Cargo versionado no está disponible en ${vendor}`);
  }
  const home = path.join(profile, 'cargo-home');
  fs.mkdirSync(home, { recursive: true });
  const vendorToml = JSON.stringify(vendor.replaceAll('\\', '/'));
  fs.writeFileSync(
    path.join(home, 'config.toml'),
    `[source.crates-io]\nreplace-with = "fixture"\n\n[source.fixture]\ndirectory = ${vendorToml}\n`
  );
  return home;
}

function prepareFakeCargo(profile) {
  const directory = path.join(profile, 'fake-cargo');
  const marker = path.join(profile, 'cargo-invoked');
  fs.mkdirSync(directory, { recursive: true });
  const script = `#!/bin/sh\n: > '${marker.replaceAll("'", "'\\''")}'\nexit 97\n`;
  fs.writeFileSync(path.join(directory, 'cargo'), script, { mode: 0o755 });
  return { directory, marker };
}

async function runHostCase({ label, fixture, extraArgs, environment }) {
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
    ...extraArgs
  ];
  await new Promise((resolve, reject) => {
    const child = spawn(executable, args, {
      stdio: 'inherit',
      shell: false,
      env: { ...process.env, ...environment }
    });
    const timeout = setTimeout(() => {
      child.kill('SIGTERM');
      reject(new Error(`VS Code extension host timed out in ${label}`));
    }, 180_000);
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
  const only = process.env.OXIDO_HOST_ONLY;
  const cargoProfile = path.join(extensionRoot, '.vscode-test', 'cargo-workspace');
  const cargoHome = prepareCargoHome(cargoProfile);
  if (!only || only === 'trusted') {
    await runHostCase({
      label: 'trusted workspace',
      fixture: path.join(__dirname, 'fixtures', 'trusted'),
      extraArgs: ['--disable-workspace-trust'],
      environment: { OXIDO_EXPECT_TRUSTED: 'true', OXIDO_HOST_CASE: 'trusted' }
    });
  }
  if (!only || only === 'cargo') {
    await runHostCase({
      label: 'cargo workspace',
      fixture: path.join(__dirname, 'fixtures', 'cargo'),
      extraArgs: ['--disable-workspace-trust'],
      environment: {
        OXIDO_EXPECT_TRUSTED: 'true',
        OXIDO_HOST_CASE: 'cargo',
        CARGO_HOME: cargoHome,
        CARGO_NET_OFFLINE: 'true'
      }
    });
  }
  if (!only || only === 'restricted') {
    const restrictedProfile = path.join(extensionRoot, '.vscode-test', 'restricted-workspace');
    const fakeCargo = prepareFakeCargo(restrictedProfile);
    fs.rmSync(fakeCargo.marker, { force: true });
    const restrictedPath = `${fakeCargo.directory}${path.delimiter}${process.env.PATH ?? ''}`;
    await runHostCase({
      label: 'restricted workspace',
      fixture: path.join(__dirname, 'fixtures', 'restricted'),
      extraArgs: [],
      environment: {
        OXIDO_EXPECT_TRUSTED: 'false',
        OXIDO_HOST_CASE: 'restricted',
        OXIDO_CARGO_MARKER: fakeCargo.marker,
        PATH: restrictedPath
      }
    });
    if (fs.existsSync(fakeCargo.marker)) {
      throw new Error('Cargo se ejecutó en un workspace restringido');
    }
    process.stdout.write('VS Code host test: Restricted Mode no invocó Cargo\n');
  }
}

main().catch(error => {
  process.stderr.write(`${error instanceof Error ? error.stack : String(error)}\n`);
  process.exitCode = 1;
});
