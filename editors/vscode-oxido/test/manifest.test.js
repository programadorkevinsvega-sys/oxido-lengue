const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');
const { resolveServerCommand } = require('../src/server-path');

const root = path.resolve(__dirname, '..');

test('contributes .ox as the Óxido language and gates executable path settings', () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
  assert.ok(manifest.contributes.languages.some(language => language.id === 'oxido' && language.extensions.includes('.ox')));
  assert.equal(manifest.capabilities.untrustedWorkspaces.supported, 'limited');
  assert.ok(manifest.capabilities.untrustedWorkspaces.restrictedConfigurations.includes('oxido.serverPath'));
  assert.ok(manifest.contributes.commands.some(command => command.command === 'oxido.restartLanguageServer'));
  assert.ok(manifest.contributes.commands.some(command => command.command === 'oxido.checkCargo'));
  assert.ok(manifest.contributes.commands.some(command => command.command === 'oxido.showResourceStats'));
});

test('extension source has no shell or rust-analyzer launch path', () => {
  const extension = fs.readFileSync(path.join(root, 'src', 'extension.js'), 'utf8');
  assert.doesNotMatch(extension, /shell\s*:\s*true/);
  assert.doesNotMatch(extension, /rust-analyzer/);
});

test('packaging pins vsce and excludes test-only material from the VSIX', () => {
  const manifest = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
  assert.equal(manifest.devDependencies['@vscode/vsce'], '3.9.2');
  assert.match(manifest.scripts['package:vsix'], /vsce package/);
  assert.equal(manifest.scripts['test:vsix'], 'node test/run-host.js installed');
  assert.equal(manifest.scripts['test:vsix:server'], 'node test/run-host.js bundled');
  assert.match(manifest.scripts['package:vsix:server'], /scripts\/package-vsix-server\.js/);
  const ignored = fs.readFileSync(path.join(root, '.vscodeignore'), 'utf8')
    .split('\n')
    .map(line => line.trim())
    .filter(Boolean);
  assert.ok(ignored.includes('test/**'));
  assert.ok(ignored.includes('.vscode-test/**'));
  assert.ok(ignored.includes('dist/**'));
  assert.ok(ignored.includes('scripts/**'));
});

test('resolves a bundled server only for the default path and an executable file', () => {
  const extensionPath = fs.mkdtempSync(path.join(os.tmpdir(), 'oxido-server-path-'));
  try {
    assert.equal(
      resolveServerCommand({ configuredPath: '/custom/lsp', extensionPath, platform: 'linux' }),
      '/custom/lsp'
    );
    assert.equal(
      resolveServerCommand({ configuredPath: 'oxido-lsp', extensionPath, platform: 'linux' }),
      'oxido-lsp'
    );

    const directory = path.join(extensionPath, 'server');
    fs.mkdirSync(directory);
    const binary = path.join(directory, 'oxido-lsp');
    fs.writeFileSync(binary, '#!/bin/sh\n');
    if (process.platform !== 'win32') {
      fs.chmodSync(binary, 0o644);
      assert.equal(
        resolveServerCommand({ configuredPath: 'oxido-lsp', extensionPath, platform: 'linux' }),
        'oxido-lsp'
      );
      fs.chmodSync(binary, 0o755);
      assert.equal(
        resolveServerCommand({ configuredPath: 'oxido-lsp', extensionPath, platform: 'linux' }),
        binary
      );
    }

    const windowsBinary = path.join(directory, 'oxido-lsp.exe');
    fs.writeFileSync(windowsBinary, '');
    assert.equal(
      resolveServerCommand({ configuredPath: 'oxido-lsp', extensionPath, platform: 'win32' }),
      windowsBinary
    );
  } finally {
    fs.rmSync(extensionPath, { recursive: true, force: true });
  }
});
