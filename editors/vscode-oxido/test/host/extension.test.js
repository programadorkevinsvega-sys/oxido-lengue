const assert = require('node:assert/strict');
const path = require('node:path');
const vscode = require('vscode');

async function run() {
  const extension = vscode.extensions.getExtension('oxido-language.oxido-language-support');
  assert.ok(extension, 'manifest extension is installed in the host');
  await extension.activate();
  assert.equal(extension.isActive, true);
  const commands = await vscode.commands.getCommands();
  assert.ok(commands.includes('oxido.restartLanguageServer'));

  const file = vscode.Uri.file(path.join(vscode.workspace.workspaceFolders[0].uri.fsPath, 'main.ox'));
  const diagnosticEvent = new Promise(resolve => {
    const subscription = vscode.languages.onDidChangeDiagnostics(event => {
      if (event.uris.some(uri => uri.toString() === file.toString())) {
        subscription.dispose();
        resolve();
      }
    });
  });
  const document = await vscode.workspace.openTextDocument(file);
  assert.equal(document.languageId, 'oxido');
  await vscode.window.showTextDocument(document);
  await diagnosticEvent;
  const diagnostics = vscode.languages.getDiagnostics(file);
  assert.ok(diagnostics.some(diagnostic => diagnostic.code === 'OX1002'));

  const fixturePath = vscode.workspace.workspaceFolders[0].uri.fsPath;
  const expectedTrusted = fixturePath.includes(`${path.sep}trusted`);
  assert.equal(vscode.workspace.isTrusted, expectedTrusted);
}

module.exports = { run };
