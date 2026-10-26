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

   Para las pruebas de host (descarga VS Code 1.143.1 si no se proporciona `VSCODE_EXECUTABLE_PATH`), compila primero el servidor y ejecuta `npm run test:host`. La suite corre en un workspace confiable y en Restricted Mode.

3. Desde este directorio, inicia el Extension Development Host con `code --extensionDevelopmentPath=. --new-window ../../prueba/modulos_jerarquicos`.

También se puede establecer `oxido.serverPath` en los ajustes de usuario. En Restricted Mode, VS Code no expone esa configuración desde el workspace. El servidor analiza texto y configuración declarativa; Cargo solo se invoca por acción explícita y con confianza.

## Límites actuales

El servidor ofrece hover, completado básico, símbolos, definición, referencias, inlay hints de literales y formato LSP para ambos modos desde el AST de Óxido; no son tipos inferidos por `rust-analyzer`, y la conformance completa del formato indentado con la CLI sigue pendiente. Todavía no hay bridge de `rust-analyzer` ni tipos/inlay hints del Rust generado. Los diagnósticos de cambios se calculan de forma asíncrona y versionada. El paquete no se ha publicado como VSIX. Antes de habilitar cualquier análisis que ejecute `build.rs` o macros procedurales habrá que comprobar Workspace Trust en el propio host, según [ADR 0010](../../docs/adr/0010-frontera-lsp-y-confianza-editor.md).
