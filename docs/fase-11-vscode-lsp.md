# Fase 11 — VS Code y LSP

**Estado:** en curso. La extensión asocia `.ox`; el LSP comparte lexer/parser, entrega semantic tokens, hover, completado básico, símbolos, definición y referencias locales, y calcula diagnósticos de edición en un worker separado. Los resultados viejos se cancelan y descartan al cambiar o cerrar el documento. El servidor limita 64 documentos, 4 MiB por documento, 16 MiB de texto abierto, cuatro cálculos concurrentes de semantic tokens y una tarea de diagnóstico en cola. El editor dispone de `Óxido: Ver estadísticas del editor`, que muestra documentos, bytes, rondas de diagnóstico, duración del último diagnóstico y tiempo de ida y vuelta. Aún no se ha medido el consumo de memoria del proceso desde una sesión de editor real. No se cierra la fase: falta `rust-analyzer`.

La decisión de seguridad está en [ADR 0010](adr/0010-frontera-lsp-y-confianza-editor.md). El servidor LSP no inicia Cargo ni `rust-analyzer` al abrir o editar; `Óxido: Comprobar con Cargo` es una acción separada que exige confianza y usa el `oxido.toml` validado del proyecto. Leer `oxido.toml` no ejecuta código. La extensión declara soporte `limited` para Workspace Trust y restringe la configuración `oxido.serverPath` en Restricted Mode.

## Estado por subfase

| Subfase | Estado y evidencia actual |
|---|---|
| 1. Extensión mínima | Parcial: `editors/vscode-oxido` asocia `.ox`, registra reinicio, define `oxido.serverPath` y conecta el cliente LSP. Una prueba host valida activación; falta VSIX y cobertura de instalación distribuida. |
| 2. Semantic tokens | Parcial: tokens generados desde el lexer real y posiciones UTF-16; las clases de identificador siguen siendo léxicas, sin resolución semántica. |
| 3. Servidor LSP | Parcial avanzado: stdio, sincronización incremental, parser compartido, recuperación, cola acotada de diagnóstico y descarte de resultados obsoletos cubiertos por pruebas de protocolo; una prueba host abre `.ox` y recibe un diagnóstico real. Falta cobertura amplia del ciclo de edición en el host. |
| 4. Diagnósticos | Parcial: lexer/parser y validaciones estructurales de `parse_ox`, con URI `.ox`, código, severidad y spans convertidos a UTF-16. El análisis de edición es asíncrono y versionado. `Óxido: Comprobar con Cargo` exige Workspace Trust, lee el `oxido.toml` del proyecto, resuelve sus dependencias declaradas y usa el `Cargo.lock` del proyecto; devuelve diagnósticos mapeados. No ejecuta esa ruta al abrir o editar. |
| 5. `rust-analyzer` | Parcial: en workspace confiable el servidor genera un `main.rs` temporal controlado, inicia el binario real `rust-analyzer --stdio` y reenvía hover, completado e inlay hints sobre Rust generado. La posición se proyecta al token generado; no se afirma mapeo semántico universal para expresiones sin correspondencia. En Restricted Mode no se inicia. |
| 6. Navegación/refactorización | Parcial: definición, referencias y símbolos se reenvían a `rust-analyzer` en workspace confiable y tienen fallback local; el fallback solo cubre funciones del documento. No hay renombrado ni navegación semántica completa de módulos externos. |
| 7. Formato/acciones | Parcial: el proveedor LSP formatea modo llaves mediante la ruta de generación/importación validada y modo indentado con el formateador estructural compartido del servidor. `Óxido: Comprobar con Cargo` y `Óxido: Ver estadísticas del editor` requieren un cliente activo; Cargo solo se ofrece en un workspace confiable. La conformance completa con `oxido formato` sigue pendiente. |
| 8. Workspace Trust | Parcial: `limited`, ejecutable restringido y sin rutas que invoquen Cargo; pruebas host verifican ambos estados y diagnóstico estático en Restricted Mode. Cualquier futura ruta de ejecución requiere nuevas pruebas. |
| 9. Cancelación/recursos | Parcial: `$/cancelRequest` para semantic tokens; diagnósticos reemplazados/cerrados se cancelan y sus resultados obsoletos se descartan. Límites explícitos: framing 4 MiB, cabecera 8 KiB, 64 documentos, 4 MiB por documento, 16 MiB total, cuatro solicitudes semantic tokens y una tarea diagnóstica en cola. Hay regresiones de límites y liberación de memoria al cerrar. El comando de estadísticas informa duración, round-trip y `VmRSS` del proceso LSP en Linux; faltan mediciones equivalentes multiplataforma desde host. |
| 10. Protocolo/instalación | Parcial: tests del crate, manifiesto y ciclo de edición/diagnóstico en host real; falta empaquetar/instalar un VSIX distribuible. |

## Verificaciones disponibles

- `cargo test -p oxido-lsp --offline`
- `cargo clippy -p oxido-lsp --all-targets --locked --offline -- -D warnings`
- `npm ci --prefix editors/vscode-oxido`
- `npm test --prefix editors/vscode-oxido`
- `npm run test:host --prefix editors/vscode-oxido`

El job `vscode-extension` de CI compila `oxido-lsp`, ejecuta la suite de manifiesto y lanza VS Code 1.143.1 dos veces: con el workspace confiable y en Restricted Mode con perfiles aislados. Ambas pruebas abren un `.ox` y esperan el diagnóstico `OX1002` real del LSP. No prueban `rust-analyzer` ni Cargo.

La suite host sí ejecuta VS Code; las pruebas de protocolo y manifiesto no sustituyen pruebas de `rust-analyzer` ni cubren otras funciones del editor.

## Instalación de desarrollo

1. Compila el servidor: `cargo build --release -p oxido-lsp`.
2. Asegúrate de que `oxido-lsp` esté en `PATH` o configura `oxido.serverPath` en ajustes de usuario.
3. Abre `editors/vscode-oxido` como proyecto de extensión con Extension Development Host, o instala el paquete VSIX cuando se publique.
4. Abre un proyecto `.ox` con `oxido.toml` válido o un archivo con cabecera `sintaxis indentacion`/`sintaxis llaves`.

La configuración leída desde el workspace solo selecciona el modo y está limitada por el parser de configuración; no contiene comandos. El análisis al editar es local. El comando Cargo requiere confianza explícita y no habilita dependencias de usuario desde el LSP en este corte.
