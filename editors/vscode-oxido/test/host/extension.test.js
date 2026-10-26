const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const vscode = require('vscode');

const hostCase = process.env.OXIDO_HOST_CASE ?? 'trusted';

async function openFixture(fixturePath, name) {
  const file = vscode.Uri.file(path.join(fixturePath, name));
  const document = await vscode.workspace.openTextDocument(file);
  assert.equal(document.languageId, 'oxido');
  await vscode.window.showTextDocument(document, { preview: false });
  await new Promise(resolve => setTimeout(resolve, 300));
  return file;
}

async function runCargoCase(fixturePath, withTimeout) {
  const activeFile = await openFixture(fixturePath, 'check_error.ox');
  const lockfile = path.join(fixturePath, 'Cargo.lock');
  const before = crypto.createHash('sha256').update(fs.readFileSync(lockfile)).digest('hex');
  const result = await withTimeout(
    vscode.commands.executeCommand('oxido.checkCargo'),
    'comprobación Cargo',
    60_000
  );
  assert.ok(result, 'el comando Cargo devuelve su resultado');
  assert.equal(result.success, false, 'el fixture contiene un error de tipos real');
  assert.equal(result.dependencyCount, 1, 'se procesó la dependencia declarada');
  assert.equal(result.lockfilePresent, true, 'se usó el Cargo.lock del proyecto');
  assert.equal(result.timedOut, false, 'Cargo terminó dentro del límite');
  assert.equal(typeof result.exitCode, 'number', 'Cargo devolvió un código de salida');

  const moduleUri = vscode.Uri.file(path.join(fixturePath, 'util_error.ox'));
  const diagnostics = vscode.languages.getDiagnostics(moduleUri);
  const cargoError = diagnostics.find(diagnostic => diagnostic.code === 'E0308');
  assert.ok(cargoError, `diagnóstico E0308 en util_error.ox: ${JSON.stringify(diagnostics)}`);
  assert.equal(cargoError.range.start.line, 3);
  assert.equal(cargoError.range.start.character, 24);
  assert.equal(
    vscode.languages.getDiagnostics(activeFile).filter(diagnostic => diagnostic.code === 'E0308').length,
    0,
    'el error no se atribuye al archivo activo'
  );

  const after = crypto.createHash('sha256').update(fs.readFileSync(lockfile)).digest('hex');
  assert.equal(after, before, 'Cargo.lock no cambia durante la comprobación');
  const marker = process.env.OXIDO_CARGO_MARKER;
  assert.ok(!marker || !fs.existsSync(marker), 'la comprobación confiada no usa procesos falsos');
  console.log('Cargo host: diagnóstico real mapeado a util_error.ox con lockfile intacto');
}

async function runNavigationCase(fixturePath, withTimeout, extensionApi) {
  assert.equal(vscode.workspace.isTrusted, false, 'la navegación local no requiere confianza');
  const activeFile = await openFixture(fixturePath, 'main.ox');
  await new Promise(resolve => setTimeout(resolve, 300));
  const request = (method, position) => extensionApi.sendLanguageServerRequest(method, {
    textDocument: { uri: activeFile.toString() },
    position
  });
  const definitionUris = async (position, label) => {
    const locations = await withTimeout(request('textDocument/definition', position), label);
    assert.ok(Array.isArray(locations), `${label}: se devuelve un array`);
    return locations.map(location => location?.uri ?? '');
  };

  const alpha = await definitionUris({ line: 6, character: 12 }, 'definición alpha');
  assert.ok(alpha.some(uri => uri.endsWith('/alpha.ox')), `alpha: ${JSON.stringify(alpha)}`);
  const beta = await definitionUris({ line: 7, character: 12 }, 'definición beta');
  assert.ok(beta.some(uri => uri.endsWith('/beta.ox')), `beta: ${JSON.stringify(beta)}`);
  const structure = await definitionUris({ line: 8, character: 38 }, 'definición Alumno');
  assert.ok(
    structure.some(uri => uri.endsWith('/dominio/modelos.ox')),
    `Alumno: ${JSON.stringify(structure)}`
  );
  const method = await definitionUris({ line: 8, character: 46 }, 'definición nuevo');
  assert.ok(
    method.some(uri => uri.endsWith('/dominio/modelos.ox')),
    `nuevo: ${JSON.stringify(method)}`
  );

  const hover = await withTimeout(request('textDocument/hover', { line: 8, character: 46 }), 'hover nuevo');
  assert.ok(
    typeof hover?.contents?.value === 'string' && hover.contents.value.includes('método'),
    `hover: ${JSON.stringify(hover)}`
  );

  const moduleDefinition = await definitionUris({ line: 8, character: 30 }, 'definición modelos');
  assert.ok(
    moduleDefinition.some(uri => uri.endsWith('/dominio.ox')),
    `modelos: ${JSON.stringify(moduleDefinition)}`
  );

  const alphaFile = await openFixture(fixturePath, 'alpha.ox');
  const alphaReferences = await withTimeout(
    extensionApi.sendLanguageServerRequest('textDocument/references', {
      textDocument: { uri: alphaFile.toString() },
      position: { line: 1, character: 18 },
      context: { includeDeclaration: true }
    }),
    'referencias desde alpha'
  );
  const alphaReferenceUris = (Array.isArray(alphaReferences) ? alphaReferences : [])
    .map(location => location?.uri ?? '');
  assert.ok(
    alphaReferenceUris.some(uri => uri.endsWith('/main.ox')),
    `alpha incluye la llamada cualificada: ${JSON.stringify(alphaReferenceUris)}`
  );
  assert.ok(
    !alphaReferenceUris.some(uri => uri.endsWith('/beta.ox')),
    'las referencias de alpha no cruzan a beta'
  );

  const ambiguaFile = await openFixture(fixturePath, 'ambigua.ox');
  const ambiguousDefinition = await withTimeout(
    extensionApi.sendLanguageServerRequest('textDocument/definition', {
      textDocument: { uri: ambiguaFile.toString() },
      position: { line: 2, character: 6 }
    }),
    'definición ambigua'
  );
  const ambiguousUris = (Array.isArray(ambiguousDefinition) ? ambiguousDefinition : [])
    .map(location => location?.uri ?? '');
  assert.ok(ambiguousUris.some(uri => uri.endsWith('/alpha.ox')), 'ambigua incluye alpha');
  assert.ok(ambiguousUris.some(uri => uri.endsWith('/beta.ox')), 'ambigua incluye beta');
  const ambiguousHover = await withTimeout(
    extensionApi.sendLanguageServerRequest('textDocument/hover', {
      textDocument: { uri: ambiguaFile.toString() },
      position: { line: 2, character: 6 }
    }),
    'hover ambiguo'
  );
  assert.ok(
    typeof ambiguousHover?.contents?.value === 'string'
      && ambiguousHover.contents.value.includes('2 declaraciones candidatas'),
    `hover ambiguo: ${JSON.stringify(ambiguousHover)}`
  );

  const references = await withTimeout(
    extensionApi.sendLanguageServerRequest('textDocument/references', {
      textDocument: { uri: activeFile.toString() },
      position: { line: 6, character: 12 },
      context: { includeDeclaration: true }
    }),
    'referencias alpha'
  );
  const referenceUris = (Array.isArray(references) ? references : [])
    .map(location => location?.uri ?? '');
  assert.ok(referenceUris.some(uri => uri.endsWith('/alpha.ox')), 'referencias de alpha');
  assert.ok(!referenceUris.some(uri => uri.endsWith('/beta.ox')), 'alpha no referencia beta');

  const messyFile = await openFixture(fixturePath, 'messy.ox');
  const edits = await withTimeout(
    extensionApi.sendLanguageServerRequest('textDocument/formatting', {
      textDocument: { uri: messyFile.toString() },
      options: { tabSize: 4, insertSpaces: true }
    }),
    'formato compartido'
  );
  assert.ok(Array.isArray(edits) && edits.length === 1, `formato: ${JSON.stringify(edits)}`);
  assert.equal(
    edits[0].newText,
    'sintaxis indentacion\nfuncion suma(a: i32, b: i32) -> i32:\n    retornar a + b\n',
    'el LSP aplica el mismo formato canónico que la CLI'
  );
  console.log('Navigation host: módulos, referencias inversas y ambigüedad segura sin confianza');
}

function waitForMarker(marker, timeoutMs = 5000) {
  const started = Date.now();
  return new Promise((resolve, reject) => {
    const poll = () => {
      if (fs.existsSync(marker)) {
        resolve();
        return;
      }
      if (Date.now() - started > timeoutMs) {
        reject(new Error(`Cargo no creó el marcador ${path.basename(marker)}`));
        return;
      }
      setTimeout(poll, 50);
    };
    poll();
  });
}

async function runCancellationCase(fixturePath, withTimeout, extensionApi) {
  assert.equal(vscode.workspace.isTrusted, true, 'la cancelación Cargo exige confianza');
  const activeFile = await openFixture(fixturePath, 'main.ox');
  const startedMarker = process.env.OXIDO_CARGO_STARTED;
  const finishedMarker = process.env.OXIDO_CARGO_FINISHED;
  assert.ok(startedMarker && finishedMarker, 'la prueba define los marcadores de Cargo');
  fs.rmSync(startedMarker, { force: true });
  fs.rmSync(finishedMarker, { force: true });

  const cancellation = new vscode.CancellationTokenSource();
  const check = extensionApi.sendLanguageServerRequest('oxido/check', {
    trusted: true,
    textDocument: { uri: activeFile.toString() }
  }, cancellation.token);
  let settled = false;
  check.then(() => { settled = true; }, () => { settled = true; });

  const early = await Promise.race([
    waitForMarker(startedMarker).then(() => undefined),
    check.then(
      result => ({ result }),
      error => ({ error: error instanceof Error ? error.message : String(error) })
    )
  ]);
  assert.equal(
    early,
    undefined,
    `la comprobación terminó antes de que Cargo arrancara: ${JSON.stringify(early)}`
  );
  assert.equal(settled, false, 'Cargo sigue en curso cuando se solicitan estadísticas');
  const statsStarted = Date.now();
  const stats = await withTimeout(
    extensionApi.sendLanguageServerRequest('oxido/stats', {}),
    'estadísticas durante Cargo',
    1500
  );
  assert.ok(Date.now() - statsStarted < 1500, 'las estadísticas no esperan a Cargo');
  assert.equal(typeof stats.openDocuments, 'number');
  assert.equal(stats.openDocuments >= 1, true);

  cancellation.cancel();
  let canceled = false;
  try {
    await withTimeout(check, 'cancelación de Cargo', 5000);
  } catch (error) {
    canceled = error?.code === -32800
      || /cancel/i.test(error instanceof Error ? error.message : String(error));
  }
  assert.ok(canceled, 'la solicitud Cargo cancelada devuelve -32800');
  await new Promise(resolve => setTimeout(resolve, 500));
  assert.ok(!fs.existsSync(finishedMarker), 'la cancelación mata el proceso Cargo');
  console.log('Cancellation host: stats respondió durante Cargo y la cancelación lo detuvo');
}

async function runRestrictedCase(fixturePath, withTimeout) {
  assert.equal(vscode.workspace.isTrusted, false, 'el host arranca en Restricted Mode');
  const activeFile = await openFixture(fixturePath, 'main.ox');
  const result = await withTimeout(
    vscode.commands.executeCommand('oxido.checkCargo'),
    'rechazo Cargo sin confianza'
  );
  assert.equal(result, undefined, 'el comando no envía una solicitud Cargo sin confianza');
  const marker = process.env.OXIDO_CARGO_MARKER;
  assert.ok(marker, 'la prueba define el marcador de procesos Cargo');
  assert.ok(!fs.existsSync(marker), 'el comando no inicia Cargo en Restricted Mode');

  let rejectedByServer = false;
  try {
    await withTimeout(
      require('../../src/extension.js').sendLanguageServerRequest('oxido/check', {
        trusted: true,
        textDocument: { uri: activeFile.toString() }
      }),
      'rechazo del servidor LSP'
    );
  } catch (error) {
    rejectedByServer = /confianza/i.test(error instanceof Error ? error.message : String(error));
  }
  assert.ok(rejectedByServer, 'el servidor rechaza Cargo cuando el workspace no es confiable');
  assert.ok(!fs.existsSync(marker), 'el servidor no inicia Cargo antes de rechazar');
  console.log('Restricted host: ni la extensión ni el servidor invocaron Cargo');
}

async function runInstalledCase(fixturePath, withTimeout, extension) {
  const expectedPath = process.env.OXIDO_EXPECT_INSTALLED_PATH;
  assert.ok(expectedPath, 'la prueba define la ruta de la extensión instalada');
  assert.equal(
    fs.realpathSync(extension.extensionPath),
    fs.realpathSync(expectedPath),
    'la extensión activa procede del directorio de extensiones instaladas'
  );
  assert.equal(extension.packageJSON.version, process.env.OXIDO_EXPECT_VERSION);
  for (const excluded of ['test', '.vscode-test', 'package-lock.json', 'dist']) {
    assert.ok(
      !fs.existsSync(path.join(extension.extensionPath, excluded)),
      `el VSIX no incluye ${excluded}`
    );
  }
  assert.ok(
    !fs.existsSync(path.join(extension.extensionPath, 'oxido-lsp')),
    'el VSIX no empaqueta un binario oxido-lsp dependiente de plataforma'
  );

  const file = vscode.Uri.file(path.join(fixturePath, 'main.ox'));
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
  await withTimeout(diagnosticEvent, 'diagnóstico del VSIX instalado');
  const diagnostics = vscode.languages.getDiagnostics(file);
  assert.ok(
    diagnostics.some(diagnostic => diagnostic.code === 'OX1002'),
    `el LSP instalado publica OX1002: ${JSON.stringify(diagnostics)}`
  );
  console.log('Installed host: VSIX instalado abrió .ox y recibió OX1002 desde oxido-lsp');
}

async function run() {
  const withTimeout = (promise, label, ms = 8000) => Promise.race([
    promise,
    new Promise((_, reject) => setTimeout(() => reject(new Error(`${label} superó ${ms} ms`)), ms))
  ]);
  const extension = vscode.extensions.getExtension('oxido-language.oxido-language-support');
  assert.ok(extension, 'manifest extension is installed in the host');
  await extension.activate();
  const extensionApi = hostCase === 'installed' ? undefined : require('../../src/extension.js');
  assert.equal(extension.isActive, true);
  const commands = await vscode.commands.getCommands();
  assert.ok(commands.includes('oxido.restartLanguageServer'));
  assert.ok(commands.includes('oxido.showResourceStats'));
  assert.ok(commands.includes('oxido.checkCargo'));

  const fixturePath = vscode.workspace.workspaceFolders[0].uri.fsPath;
  const expectedTrusted = process.env.OXIDO_EXPECT_TRUSTED === 'true';
  assert.equal(vscode.workspace.isTrusted, expectedTrusted);

  if (hostCase === 'installed') {
    await runInstalledCase(fixturePath, withTimeout, extension);
    // Quit after the assertions so the installed extension and its language
    // client finish deactivating before the host process exits.
    await vscode.commands.executeCommand('workbench.action.quit');
    return;
  }
  if (hostCase === 'cargo') {
    await runCargoCase(fixturePath, withTimeout);
    await vscode.commands.executeCommand('workbench.action.quit');
    return;
  }
  if (hostCase === 'restricted') {
    await runRestrictedCase(fixturePath, withTimeout);
    // The Restricted Mode banner initializes asynchronously.  Quit the
    // application after the assertions instead of closing the window while
    // that startup work can still race the host shutdown.
    await vscode.commands.executeCommand('workbench.action.quit');
    return;
  }
  if (hostCase === 'navigation') {
    await runNavigationCase(fixturePath, withTimeout, extensionApi);
    await vscode.commands.executeCommand('workbench.action.quit');
    return;
  }
  if (hostCase === 'cancellation') {
    await runCancellationCase(fixturePath, withTimeout, extensionApi);
    await vscode.commands.executeCommand('workbench.action.quit');
    return;
  }

  const file = vscode.Uri.file(path.join(fixturePath, 'main.ox'));
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

  await vscode.commands.executeCommand('workbench.action.quit');
}

module.exports = { run };
