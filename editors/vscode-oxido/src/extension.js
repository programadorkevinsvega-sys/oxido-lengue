const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');

let client;

async function startClient() {
  if (client) {
    await client.stop();
    client = undefined;
  }

  const configuredPath = vscode.workspace.getConfiguration('oxido').get('serverPath', 'oxido-lsp');
  const workspace = vscode.workspace.workspaceFolders?.[0];
  const serverOptions = {
    command: configuredPath,
    args: [],
    options: workspace ? { cwd: workspace.uri.fsPath } : {}
  };
  const clientOptions = { documentSelector: [{ scheme: 'file', language: 'oxido' }] };

  client = new LanguageClient('oxido', 'Óxido Language Server', serverOptions, clientOptions);
  await client.start();
}

async function activate(context) {
  await startClient();
  const configWatcher = vscode.workspace.createFileSystemWatcher('**/oxido.toml');
  context.subscriptions.push(configWatcher);
  context.subscriptions.push(configWatcher.onDidChange(() => restartAfterProjectConfigChange()));
  context.subscriptions.push(configWatcher.onDidCreate(() => restartAfterProjectConfigChange()));
  context.subscriptions.push(vscode.commands.registerCommand('oxido.restartLanguageServer', async () => {
    try {
      await startClient();
      await vscode.window.showInformationMessage('Servidor de Óxido reiniciado.');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'falló el reinicio del servidor';
      await vscode.window.showErrorMessage(`Óxido: ${message}`);
    }
  }));
}

async function restartAfterProjectConfigChange() {
  if (!client) return;
  try {
    await startClient();
  } catch (error) {
    const message = error instanceof Error ? error.message : 'falló el reinicio tras cambiar oxido.toml';
    await vscode.window.showErrorMessage(`Óxido: ${message}`);
  }
}

async function deactivate() {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

module.exports = { activate, deactivate };
