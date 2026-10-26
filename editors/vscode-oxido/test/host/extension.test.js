const assert = require('node:assert/strict');
const path = require('node:path');
const vscode = require('vscode');

async function run() {
  const withTimeout = (promise, label, ms = 8000) => Promise.race([
    promise,
    new Promise((_, reject) => setTimeout(() => reject(new Error(`${label} superó ${ms} ms`)), ms))
  ]);
  const extension = vscode.extensions.getExtension('oxido-language.oxido-language-support');
  assert.ok(extension, 'manifest extension is installed in the host');
  await extension.activate();
  const extensionApi = require('../../src/extension.js');
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
  const expectedTrusted = process.env.OXIDO_EXPECT_TRUSTED === 'true';
  assert.equal(vscode.workspace.isTrusted, expectedTrusted);

  if (expectedTrusted) {
    console.log('RA host: opening semantic fixture');
    const semanticFile = vscode.Uri.file(path.join(fixturePath, 'semantic.ox'));
    const semanticDocument = await vscode.workspace.openTextDocument(semanticFile);
    assert.equal(semanticDocument.languageId, 'oxido');
    // LanguageClient receives didOpen from the editor lifecycle.  Showing the
    // document here exercises the same path that a developer uses, instead of
    // relying on a provider request to open it implicitly.
    await vscode.window.showTextDocument(semanticDocument, { preview: false });
    await new Promise(resolve => setTimeout(resolve, 500));
    console.log('RA host: requesting hover');
    const request = (method, position) => extensionApi.sendLanguageServerRequest(method, {
      textDocument: { uri: semanticFile.toString() },
      position
    });
    const hover = await withTimeout(request('textDocument/hover', { line: 2, character: 12 }), 'hover');
    assert.ok(hover === null || typeof hover === 'object', 'rust-analyzer hover request completed');
    const stats = await withTimeout(extensionApi.sendLanguageServerRequest('oxido/stats', {}), 'estadísticas');
    assert.equal(typeof stats.openDocuments, 'number', 'language server responds after hover');
    console.log('RA host: stats returned');
    const completion = await withTimeout(request('textDocument/completion', { line: 5, character: 10 }), 'completado');
    assert.ok(
      completion === null || Array.isArray(completion) || Array.isArray(completion?.items),
      'rust-analyzer completion request completed'
    );
    console.log('RA host: completion returned');
    const hints = await withTimeout(extensionApi.sendLanguageServerRequest('textDocument/inlayHint', {
      textDocument: { uri: semanticFile.toString() },
      range: { start: { line: 0, character: 0 }, end: { line: 20, character: 0 } }
    }), 'inlay hints');
    assert.ok(Array.isArray(hints), 'inlay hint request completed');
    console.log('RA host: inlay hints returned');
    const definition = await withTimeout(request('textDocument/definition', { line: 5, character: 5 }), 'definición');
    assert.ok(Array.isArray(definition), 'definition request completed');
    console.log('RA host: definition returned');
    let evidence;
    for (let attempt = 0; attempt < 8; attempt += 1) {
      await withTimeout(request('textDocument/hover', { line: 2, character: 12 }), 'hover de rust-analyzer');
      evidence = await withTimeout(
        extensionApi.sendLanguageServerRequest('oxido/stats', {}),
        'estadísticas de rust-analyzer'
      );
      if (evidence.rustAnalyzerResponses > 0) break;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    assert.ok(evidence.rustAnalyzerResponses > 0, 'rust-analyzer respondió al menos una consulta del host');

    const moduleFile = vscode.Uri.file(path.join(fixturePath, 'semantic_modulo.ox'));
    const moduleDocument = await vscode.workspace.openTextDocument(moduleFile);
    await vscode.window.showTextDocument(moduleDocument, { preview: false });
    await new Promise(resolve => setTimeout(resolve, 200));
    const moduleDefinition = await withTimeout(
      extensionApi.sendLanguageServerRequest('textDocument/definition', {
        textDocument: { uri: moduleFile.toString() },
        position: { line: 4, character: 12 }
      }),
      'definición entre módulos'
    );
    const locations = Array.isArray(moduleDefinition) ? moduleDefinition : [moduleDefinition];
    assert.ok(
      locations.some(location => location?.targetUri?.endsWith('/util.ox') || location?.uri?.endsWith('/util.ox')),
      'la navegación entre módulos vuelve al archivo .ox de origen'
    );
  }

  await vscode.commands.executeCommand('workbench.action.closeWindow');
}

module.exports = { run };
