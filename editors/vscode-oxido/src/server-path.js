const fs = require('node:fs');
const path = require('node:path');

const DEFAULT_SERVER = 'oxido-lsp';

function bundledServerName(platform = process.platform) {
  return platform === 'win32' ? 'oxido-lsp.exe' : 'oxido-lsp';
}

/**
 * Resuelve el servidor LSP del paquete instalado. `oxido.serverPath` conserva
 * prioridad absoluta; el binario incluido solo se usa cuando la configuración
 * sigue en el valor por defecto y el archivo es ejecutable (o existe en
 * Windows). Si no, se mantiene la resolución por `PATH`.
 */
function resolveServerCommand({ configuredPath, extensionPath, platform = process.platform }) {
  if (configuredPath !== DEFAULT_SERVER) {
    return configuredPath;
  }
  const bundled = path.join(extensionPath, 'server', bundledServerName(platform));
  if (!fs.existsSync(bundled)) {
    return configuredPath;
  }
  if (platform !== 'win32' && (fs.statSync(bundled).mode & 0o111) === 0) {
    return configuredPath;
  }
  return bundled;
}

module.exports = { DEFAULT_SERVER, bundledServerName, resolveServerCommand };
