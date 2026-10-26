const fs = require('node:fs');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');
const { downloadAndUnzipVSCode } = require('@vscode/test-electron');

const extensionRoot = path.resolve(__dirname, '..');
const repositoryRoot = path.resolve(extensionRoot, '..', '..');
const testRunner = path.join(__dirname, 'host', 'runner.js');
const harnessPath = path.join(__dirname, 'host', 'harness');
const packageJson = JSON.parse(fs.readFileSync(path.join(extensionRoot, 'package.json'), 'utf8'));
const HOST_PROFILES = [
  'trusted-workspace',
  'cargo-workspace',
  'navigation-workspace',
  'cancellation-workspace',
  'installed-workspace',
  'bundled-workspace',
  'restricted-workspace'
];
let executablePromise;

function resolveExecutable() {
  executablePromise ??= Promise.resolve(
    process.env.VSCODE_EXECUTABLE_PATH
      || downloadAndUnzipVSCode({ version: '1.143.1' })
  );
  return executablePromise;
}

function cleanHostProfiles() {
  for (const name of HOST_PROFILES) {
    fs.rmSync(path.join(extensionRoot, '.vscode-test', name), { recursive: true, force: true });
  }
}

function resolveVsCodeCli(executable) {
  const directory = path.dirname(executable);
  const candidates = process.platform === 'win32'
    ? [path.join(directory, 'bin', 'code.cmd')]
    : process.platform === 'darwin'
      ? [
          path.join(directory, '..', 'Resources', 'app', 'bin', 'code'),
          path.join(directory, 'bin', 'code')
        ]
      : [path.join(directory, 'bin', 'code')];
  const cli = candidates.find(candidate => fs.existsSync(candidate));
  if (!cli) {
    throw new Error(`no se encontró la CLI de VS Code junto al ejecutable ${executable}`);
  }
  return cli;
}

function packageVsix() {
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const result = spawnSync(npm, ['run', 'package:vsix'], {
    cwd: extensionRoot,
    stdio: 'inherit',
    shell: false
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`el empaquetado del VSIX terminó con estado ${result.status}`);
  }
  const vsix = path.join(extensionRoot, 'dist', `${packageJson.name}-${packageJson.version}.vsix`);
  if (!fs.existsSync(vsix)) {
    throw new Error(`el VSIX esperado no existe en ${vsix}`);
  }
  return vsix;
}

function vsCodeTarget() {
  const architecture = process.arch === 'arm64' ? 'arm64' : process.arch === 'arm' ? 'armhf' : 'x64';
  return `${process.platform}-${architecture}`;
}

function packageBundledVsix() {
  const target = vsCodeTarget();
  const binary = path.join(
    repositoryRoot,
    'target',
    'release',
    process.platform === 'win32' ? 'oxido-lsp.exe' : 'oxido-lsp'
  );
  if (!fs.existsSync(binary)) {
    throw new Error('compila oxido-lsp en release antes de empaquetar el servidor incluido');
  }
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  const result = spawnSync(npm, ['run', 'package:vsix:server'], {
    cwd: extensionRoot,
    stdio: 'inherit',
    shell: false,
    env: {
      ...process.env,
      OXIDO_SERVER_BINARY: binary,
      OXIDO_VSIX_TARGET: target
    }
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`el empaquetado del VSIX con servidor terminó con estado ${result.status}`);
  }
  const vsix = path.join(
    extensionRoot,
    'dist',
    `${packageJson.name}-${packageJson.version}-${target}.vsix`
  );
  if (!fs.existsSync(vsix)) {
    throw new Error('el VSIX con servidor incluido no existe');
  }
  return vsix;
}

function findOnPath(name, pathValue) {
  for (const entry of pathValue.split(path.delimiter)) {
    if (entry && fs.existsSync(path.join(entry, name))) return true;
  }
  return false;
}

function installVsix(executable, profile, vsix) {
  const cli = resolveVsCodeCli(executable);
  const extensions = path.join(profile, 'extensions');
  const result = spawnSync(cli, [
    '--no-sandbox',
    '--disable-gpu',
    `--user-data-dir=${path.join(profile, 'user-data')}`,
    `--extensions-dir=${extensions}`,
    '--install-extension',
    vsix,
    '--force'
  ], {
    stdio: 'inherit',
    shell: false
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`la instalación del VSIX terminó con estado ${result.status}`);
  }
  const installed = path.join(
    extensions,
    `${packageJson.publisher}.${packageJson.name}-${packageJson.version}`
  );
  if (!fs.existsSync(path.join(installed, 'package.json'))) {
    throw new Error(`VS Code no instaló el VSIX en ${installed}`);
  }
  return installed;
}

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

function prepareSlowFakeCargo(profile) {
  if (process.platform === 'win32') {
    return undefined;
  }
  const directory = path.join(profile, 'slow-cargo');
  const started = path.join(profile, 'cargo-started');
  const finished = path.join(profile, 'cargo-finished');
  fs.mkdirSync(directory, { recursive: true });
  const quote = value => `'${value.replaceAll("'", "'\\''")}'`;
  const script = `#!/bin/sh\n: > ${quote(started)}\nsleep 30\n: > ${quote(finished)}\nexit 0\n`;
  fs.writeFileSync(path.join(directory, 'cargo'), script, { mode: 0o755 });
  return { directory, started, finished };
}

async function runHostCase({
  label,
  fixture,
  extraArgs = [],
  environment,
  developmentPath = extensionRoot
}) {
  process.stdout.write(`VS Code host test: ${label}\n`);
  const executable = await resolveExecutable();
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
    `--extensionDevelopmentPath=${developmentPath}`,
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
  cleanHostProfiles();
  const only = process.env.OXIDO_HOST_ONLY ?? process.argv[2];
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
  if (!only || only === 'navigation') {
    await runHostCase({
      label: 'navigation workspace',
      fixture: path.join(__dirname, 'fixtures', 'navigation'),
      extraArgs: [],
      environment: {
        OXIDO_EXPECT_TRUSTED: 'false',
        OXIDO_HOST_CASE: 'navigation'
      }
    });
  }
  if (!only || only === 'cancellation') {
    const cancellationProfile = path.join(extensionRoot, '.vscode-test', 'cancellation-workspace');
    const fakeCargo = prepareSlowFakeCargo(cancellationProfile);
    if (fakeCargo) {
      fs.rmSync(fakeCargo.started, { force: true });
      fs.rmSync(fakeCargo.finished, { force: true });
      const cancellationPath = `${fakeCargo.directory}${path.delimiter}${process.env.PATH ?? ''}`;
      await runHostCase({
        label: 'cancellation workspace',
        fixture: path.join(__dirname, 'fixtures', 'navigation'),
        extraArgs: ['--disable-workspace-trust'],
        environment: {
          OXIDO_EXPECT_TRUSTED: 'true',
          OXIDO_HOST_CASE: 'cancellation',
          OXIDO_CARGO_STARTED: fakeCargo.started,
          OXIDO_CARGO_FINISHED: fakeCargo.finished,
          OXIDO_CARGO_BINARY: path.join(fakeCargo.directory, 'cargo'),
          PATH: cancellationPath
        }
      });
    } else {
      process.stdout.write('VS Code host test: el caso de cancelación se omite en Windows\n');
    }
  }
  if (!only || only === 'installed') {
    const vsix = packageVsix();
    const executable = await resolveExecutable();
    const installedProfile = path.join(extensionRoot, '.vscode-test', 'installed-workspace');
    const installedPath = installVsix(executable, installedProfile, vsix);
    await runHostCase({
      label: 'installed workspace',
      fixture: path.join(__dirname, 'fixtures', 'trusted'),
      extraArgs: ['--disable-workspace-trust'],
      developmentPath: harnessPath,
      environment: {
        OXIDO_EXPECT_TRUSTED: 'true',
        OXIDO_HOST_CASE: 'installed',
        OXIDO_EXPECT_INSTALLED_PATH: installedPath,
        OXIDO_EXPECT_VERSION: packageJson.version
      }
    });
  }
  if (!only || only === 'bundled') {
    const vsix = packageBundledVsix();
    const executable = await resolveExecutable();
    const bundledProfile = path.join(extensionRoot, '.vscode-test', 'bundled-workspace');
    const installedPath = installVsix(executable, bundledProfile, vsix);
    const sanitizedPath = (process.env.PATH ?? '')
      .split(path.delimiter)
      .filter(entry => entry && path.resolve(entry) !== path.join(repositoryRoot, 'target', 'release'))
      .join(path.delimiter);
    if (findOnPath('oxido-lsp', sanitizedPath) || findOnPath('oxido-lsp.exe', sanitizedPath)) {
      throw new Error('la prueba del servidor incluido requiere que oxido-lsp no esté en PATH');
    }
    await runHostCase({
      label: 'bundled workspace',
      fixture: path.join(__dirname, 'fixtures', 'trusted'),
      developmentPath: harnessPath,
      environment: {
        PATH: sanitizedPath,
        OXIDO_EXPECT_TRUSTED: 'false',
        OXIDO_HOST_CASE: 'bundled',
        OXIDO_EXPECT_INSTALLED_PATH: installedPath,
        OXIDO_EXPECT_VERSION: packageJson.version,
        OXIDO_EXPECT_BUNDLED_SERVER: 'true'
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
