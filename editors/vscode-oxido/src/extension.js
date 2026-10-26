const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');

let client;
let cargoDiagnostics;

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
  cargoDiagnostics = vscode.languages.createDiagnosticCollection('oxido-cargo');
  context.subscriptions.push(cargoDiagnostics);
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
  context.subscriptions.push(vscode.commands.registerCommand('oxido.checkCargo', async () => {
    if (!vscode.workspace.isTrusted) {
      await vscode.window.showWarningMessage('Óxido: confía en el workspace antes de ejecutar Cargo.');
      return;
    }
    if (!client || !vscode.window.activeTextEditor || vscode.window.activeTextEditor.document.languageId !== 'oxido') {
      await vscode.window.showWarningMessage('Óxido: abre un archivo .ox antes de comprobarlo.');
      return;
    }
    try {
      const result = await client.sendRequest('oxido/check', {
        trusted: true,
        textDocument: { uri: vscode.window.activeTextEditor.document.uri.toString() }
      });
      const diagnostics = (result?.diagnostics ?? []).map(diagnostic => new vscode.Diagnostic(
        new vscode.Range(
          new vscode.Position(diagnostic.range.start.line, diagnostic.range.start.character),
          new vscode.Position(diagnostic.range.end.line, diagnostic.range.end.character)
        ),
        diagnostic.message,
        vscode.DiagnosticSeverity.Error
      ));
      cargoDiagnostics.set(vscode.window.activeTextEditor.document.uri, diagnostics);
      await vscode.window.showInformationMessage(`Óxido: Cargo terminó con ${diagnostics.length} diagnóstico(s).`);
    } catch (error) {
      const message = error instanceof Error ? error.message : 'falló la comprobación con Cargo';
      await vscode.window.showErrorMessage(`Óxido: ${message}`);
    }
  }));
  context.subscriptions.push(vscode.commands.registerCommand('oxido.showResourceStats', async () => {
    if (!client) return;
    const started = Date.now();
    try {
      const stats = await client.sendRequest('oxido/stats', {});
      const roundTripMs = Date.now() - started;
      await vscode.window.showInformationMessage(
        `Óxido: ${stats.openDocuments} documento(s), ${stats.documentBytes} bytes, ` +
        `${stats.diagnosticRuns} diagnóstico(s), último cálculo ${stats.lastDiagnosticMs ?? 'n/d'} ms, ` +
        `respuesta ${roundTripMs} ms.`
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : 'no se pudieron leer las estadísticas';
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
