const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const extensionRoot = path.resolve(__dirname, '..');
const packageJson = JSON.parse(fs.readFileSync(path.join(extensionRoot, 'package.json'), 'utf8'));
const target = process.env.OXIDO_VSIX_TARGET;
const binary = process.env.OXIDO_SERVER_BINARY;

if (!target || !/^(?:linux|darwin|win32|alpine)-(?:x64|arm64|armhf)$/.test(target)) {
  throw new Error('OXIDO_VSIX_TARGET debe ser un target de VS Code como linux-x64 o win32-arm64');
}
if (!binary || !fs.existsSync(binary)) {
  throw new Error('OXIDO_SERVER_BINARY debe apuntar al binario oxido-lsp compilado');
}

const serverDirectory = path.join(extensionRoot, 'server');
const serverName = target.startsWith('win32-') ? 'oxido-lsp.exe' : 'oxido-lsp';
const serverPath = path.join(serverDirectory, serverName);
const output = path.join(
  extensionRoot,
  'dist',
  `${packageJson.name}-${packageJson.version}-${target}.vsix`
);

fs.mkdirSync(serverDirectory, { recursive: true });
fs.copyFileSync(binary, serverPath);
if (!target.startsWith('win32-')) {
  fs.chmodSync(serverPath, 0o755);
}
fs.mkdirSync(path.dirname(output), { recursive: true });

try {
  const vsce = path.join(extensionRoot, 'node_modules', '@vscode', 'vsce', 'vsce');
  if (!fs.existsSync(vsce)) {
    throw new Error('falta @vscode/vsce; ejecuta npm ci en editors/vscode-oxido');
  }
  const result = spawnSync(
    process.execPath,
    [vsce, 'package', '--skip-license', '--target', target, '--out', output],
    { cwd: extensionRoot, stdio: 'inherit', shell: false }
  );
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`el empaquetado terminó con estado ${result.status}`);
  }
  if (!fs.existsSync(output)) {
    throw new Error('vsce no escribió el VSIX esperado');
  }
  process.stdout.write(`VSIX con servidor incluido: ${path.relative(extensionRoot, output)}\n`);
} finally {
  fs.rmSync(serverPath, { force: true });
  if (fs.existsSync(serverDirectory) && fs.readdirSync(serverDirectory).length === 0) {
    fs.rmdirSync(serverDirectory);
  }
}
