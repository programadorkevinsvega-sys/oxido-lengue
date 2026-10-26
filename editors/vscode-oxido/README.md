# Extensión de VS Code para Óxido

La extensión asocia `.ox` con el lenguaje Óxido, inicia `oxido-lsp` y recibe diagnósticos del lexer/parser y semantic tokens léxicos. El estado por subfase y sus límites están en [Fase 11](../../docs/fase-11-vscode-lsp.md).

## Desarrollo local

1. Compila el servidor:

   ```sh
   cargo build --release -p oxido-lsp
   ```

   Añade `target/release` a `PATH` o configura la ruta absoluta `target/release/oxido-lsp` en el ajuste de usuario `oxido.serverPath`.

2. Instala las dependencias de la extensión y ejecuta sus pruebas:

   ```sh
   npm ci
   npm test
   ```

   Para las pruebas de host (descarga VS Code 1.143.1 si no se proporciona `VSCODE_EXECUTABLE_PATH`), compila primero el servidor y ejecuta `npm run test:host`. La suite corre en un workspace confiable, en Restricted Mode, desde un VSIX estándar instalado y desde un VSIX con el servidor incluido. `npm run test:vsix` empaqueta, instala y prueba la ruta distribuible estándar; `npm run test:vsix:server` hace lo mismo con `oxido-lsp` incluido y exige que no haya otro `oxido-lsp` en `PATH`.

3. Empaqueta e instala el VSIX local:

   ```sh
   npm run package:vsix
   code --install-extension dist/oxido-language-support-0.1.0.vsix
   ```

   El VSIX estándar no incluye `oxido-lsp`; el paso 1 sigue siendo necesario. Para incluir el servidor de la plataforma actual:

   ```sh
   OXIDO_SERVER_BINARY=../../target/release/oxido-lsp \
   OXIDO_VSIX_TARGET=linux-x64 \
   npm run package:vsix:server
   code --install-extension dist/oxido-language-support-0.1.0-linux-x64.vsix
   ```

   El binario incluido se usa solo si `oxido.serverPath` conserva el valor por defecto `oxido-lsp`; una ruta configurada explícitamente tiene prioridad. Ningún paquete está firmado ni se publica en un marketplace. `vsce` es una dependencia de desarrollo y no se incluye en el VSIX. El workflow `.github/workflows/release.yml` prepara la matriz Linux x64, Windows x64 y macOS arm64 con checksums, pero solo Linux x64 se ha ejecutado localmente.

4. Desde este directorio, inicia el Extension Development Host con `code --extensionDevelopmentPath=. --new-window ../../prueba/modulos_jerarquicos`.

También se puede establecer `oxido.serverPath` en los ajustes de usuario. En Restricted Mode, VS Code no expone esa configuración desde el workspace. El servidor analiza texto y configuración declarativa; Cargo solo se invoca por acción explícita y con confianza.

## Límites actuales

El servidor ofrece hover, completado, símbolos, definición, referencias e inlay hints desde el AST. Definición, referencias y completado local recorren todos los `.ox` abiertos y conservan la URI del módulo. En workspace confiable las consultas semánticas se reenvían al `rust-analyzer` real sobre Rust generado en un workspace temporal persistente; en Restricted Mode se usa el fallback local y no se inicia ningún proceso. El formato LSP funciona para ambos modos, aunque la conformance completa del modo indentado con la CLI sigue pendiente. Los diagnósticos de cambios se calculan de forma asíncrona y versionada. El VSIX 0.1.0 estándar se empaqueta e instala localmente; un VSIX adicional por plataforma incluye `server/oxido-lsp`, verificado solo en Linux x64. Ningún paquete está firmado, notarizado ni publicado en un marketplace, y la matriz Windows/macOS del workflow de release no se ha ejecutado. Antes de habilitar cualquier análisis que ejecute `build.rs` o macros procedurales habrá que comprobar Workspace Trust en el propio host, según [ADR 0010](../../docs/adr/0010-frontera-lsp-y-confianza-editor.md).
