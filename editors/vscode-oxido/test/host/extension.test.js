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
  assert.ok(commands.includes('oxido.showResourceStats'));

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

  if (expectedTrusted) {
    console.log('RA host: opening semantic fixture');
    const semanticFile = vscode.Uri.file(path.join(fixturePath, 'semantic.ox'));
    const semanticDocument = await vscode.workspace.openTextDocument(semanticFile);
    await vscode.window.showTextDocument(semanticDocument);
    await new Promise(resolve => setTimeout(resolve, 1500));
    console.log('RA host: requesting hover');
    const hover = await vscode.commands.executeCommand('vscode.executeHoverProvider', semanticFile, new vscode.Position(2, 12));
    assert.ok(Array.isArray(hover), 'rust-analyzer hover request completed');
    const completion = await vscode.commands.executeCommand('vscode.executeCompletionItemProvider', semanticFile, new vscode.Position(7, 4));
    assert.ok(completion, 'rust-analyzer completion request completed');
    console.log('RA host: completion returned');
    const hints = await vscode.commands.executeCommand('vscode.executeInlayHintProvider', semanticFile, new vscode.Range(0, 0, 20, 0));
    assert.ok(Array.isArray(hints), 'inlay hint request completed');
    console.log('RA host: inlay hints returned');
    const definition = await vscode.commands.executeCommand('vscode.executeDefinitionProvider', semanticFile, new vscode.Position(7, 5));
    assert.ok(Array.isArray(definition), 'definition request completed');
    console.log('RA host: definition returned');
    await vscode.commands.executeCommand('oxido.showResourceStats');
  }
}

module.exports = { run };
