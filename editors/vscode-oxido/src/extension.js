const vscode = require('vscode');
const { LanguageClient } = require('vscode-languageclient/node');
const { resolveServerCommand } = require('./server-path');

let client;
let cargoDiagnostics;

async function startClient(context) {
  if (client) {
    await client.stop();
    client = undefined;
  }

  const configuredPath = vscode.workspace.getConfiguration('oxido').get('serverPath', 'oxido-lsp');
  const command = resolveServerCommand({
    configuredPath,
    extensionPath: context.extensionPath
  });
  const workspace = vscode.workspace.workspaceFolders?.[0];
  const serverOptions = {
    command,
    args: [],
    options: workspace ? { cwd: workspace.uri.fsPath } : {}
  };
  const clientOptions = {
    documentSelector: [{ scheme: 'file', language: 'oxido' }],
    initializationOptions: { trusted: vscode.workspace.isTrusted }
  };

  client = new LanguageClient('oxido', 'Óxido Language Server', serverOptions, clientOptions);
  await client.start();
}

async function activate(context) {
  cargoDiagnostics = vscode.languages.createDiagnosticCollection('oxido-cargo');
  context.subscriptions.push(cargoDiagnostics);
  await startClient(context);
  const configWatcher = vscode.workspace.createFileSystemWatcher('**/oxido.toml');
  context.subscriptions.push(configWatcher);
  context.subscriptions.push(configWatcher.onDidChange(() => restartAfterProjectConfigChange(context)));
  context.subscriptions.push(configWatcher.onDidCreate(() => restartAfterProjectConfigChange(context)));
  context.subscriptions.push(vscode.commands.registerCommand('oxido.restartLanguageServer', async () => {
    try {
      await startClient(context);
      await vscode.window.showInformationMessage('Servidor de Óxido reiniciado.');
    } catch (error) {
      const message = error instanceof Error ? error.message : 'falló el reinicio del servidor';
      await vscode.window.showErrorMessage(`Óxido: ${message}`);
    }
  }));
  context.subscriptions.push(vscode.commands.registerCommand('oxido.checkCargo', async () => {
    if (!vscode.workspace.isTrusted) {
      void vscode.window.showWarningMessage('Óxido: confía en el workspace antes de ejecutar Cargo.');
      return undefined;
    }
    if (!client || !vscode.window.activeTextEditor || vscode.window.activeTextEditor.document.languageId !== 'oxido') {
      void vscode.window.showWarningMessage('Óxido: abre un archivo .ox antes de comprobarlo.');
      return undefined;
    }
    try {
      const result = await client.sendRequest('oxido/check', {
        trusted: true,
        textDocument: { uri: vscode.window.activeTextEditor.document.uri.toString() }
      });
      cargoDiagnostics.clear();
      let diagnosticCount = 0;
      for (const file of result?.files ?? []) {
        const diagnostics = (file.diagnostics ?? []).map(diagnostic => {
          const item = new vscode.Diagnostic(
            new vscode.Range(
              new vscode.Position(diagnostic.range.start.line, diagnostic.range.start.character),
              new vscode.Position(diagnostic.range.end.line, diagnostic.range.end.character)
            ),
            diagnostic.message,
            diagnostic.severity === 2 ? vscode.DiagnosticSeverity.Warning : vscode.DiagnosticSeverity.Error
          );
          item.code = diagnostic.code;
          item.source = diagnostic.source ?? 'cargo';
          return item;
        });
        diagnosticCount += diagnostics.length;
        cargoDiagnostics.set(vscode.Uri.parse(file.uri), diagnostics);
      }
      const summary = result?.success ? 'sin errores' : `${diagnosticCount} diagnóstico(s)`;
      void vscode.window.showInformationMessage(
        `Óxido: Cargo terminó ${summary} con ${result?.dependencyCount ?? 0} dependencia(s).`
      );
      return result;
    } catch (error) {
      const message = error instanceof Error ? error.message : 'falló la comprobación con Cargo';
      void vscode.window.showErrorMessage(`Óxido: ${message}`);
      return undefined;
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
        `memoria residente ${stats.residentMemoryKiB ?? 'n/d'} KiB, ` +
        `respuesta ${roundTripMs} ms.`
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : 'no se pudieron leer las estadísticas';
      await vscode.window.showErrorMessage(`Óxido: ${message}`);
    }
  }));
}

async function restartAfterProjectConfigChange(context) {
  if (!client) return;
  try {
    await startClient(context);
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

async function sendLanguageServerRequest(method, params, token) {
  if (!client) {
    throw new Error('el servidor de lenguaje de Óxido no está iniciado');
  }
  return token
    ? client.sendRequest(method, params, token)
    : client.sendRequest(method, params);
}

module.exports = { activate, deactivate, sendLanguageServerRequest };
