# Fase 11 — VS Code y LSP

**Estado:** en curso. La extensión asocia `.ox`; el LSP comparte lexer/parser, entrega semantic tokens, hover, completado básico, símbolos, definición y referencias locales, y calcula diagnósticos de edición en un worker separado. Los resultados viejos se cancelan y descartan al cambiar o cerrar el documento. El servidor limita 64 documentos, 4 MiB por documento, 16 MiB de texto abierto, cuatro cálculos concurrentes de semantic tokens y una tarea de diagnóstico en cola. El editor dispone de `Óxido: Ver estadísticas del editor`, que muestra documentos, bytes, rondas de diagnóstico, duración del último diagnóstico, memoria residente en Linux y tiempo de ida y vuelta. En un workspace confiable, un worker persistente consulta `rust-analyzer` sobre un árbol Rust temporal resuelto desde los módulos `.ox`; las ubicaciones devueltas para archivos mapeados vuelven al URI y rango del `.ox` de origen. Sus solicitudes y las del propio analizador se acotan para no bloquear el bucle LSP. La prueba host cubre la navegación cualificada `util::saludo()` hacia `util.ox`; no equivale a mapeo semántico universal entre módulos. `Óxido: Comprobar con Cargo` se ejecuta en un Extension Development Host real: usa el `oxido.toml` validado, el `Cargo.lock` del proyecto y una dependencia de crates.io vendorizada, y devuelve el `E0308` de rustc al archivo `.ox` del módulo donde está el error. Una ejecución host en Restricted Mode comprueba que ni la extensión ni el servidor invocan Cargo. No se cierra la fase: falta ampliar la navegación multi-módulo, la conformance de formato en host, la respuesta durante Cargo, las mediciones multiplataforma y el VSIX.

La decisión de seguridad está en [ADR 0010](adr/0010-frontera-lsp-y-confianza-editor.md). El servidor LSP no inicia Cargo ni `rust-analyzer` al abrir o editar; `Óxido: Comprobar con Cargo` es una acción separada que exige confianza y usa el `oxido.toml` validado del proyecto. Leer `oxido.toml` no ejecuta código. La extensión declara soporte `limited` para Workspace Trust y restringe la configuración `oxido.serverPath` en Restricted Mode.

## Estado por subfase

| Subfase | Estado y evidencia actual |
|---|---|
| 1. Extensión mínima | Parcial: `editors/vscode-oxido` asocia `.ox`, registra reinicio, define `oxido.serverPath` y conecta el cliente LSP. Una prueba host valida activación; falta VSIX y cobertura de instalación distribuida. |
| 2. Semantic tokens | Parcial: tokens generados desde el lexer real y posiciones UTF-16; las clases de identificador siguen siendo léxicas, sin resolución semántica. |
| 3. Servidor LSP | Parcial avanzado: stdio, sincronización incremental, parser compartido, recuperación, cola acotada de diagnóstico y descarte de resultados obsoletos cubiertos por pruebas de protocolo; una prueba host abre `.ox` y recibe un diagnóstico real. Falta cobertura amplia del ciclo de edición en el host. |
| 4. Diagnósticos | Parcial avanzado: lexer/parser y validaciones estructurales de `parse_ox`, con URI `.ox`, código, severidad y spans convertidos a UTF-16. El análisis de edición es asíncrono y versionado. `Óxido: Comprobar con Cargo` exige Workspace Trust, lee el `oxido.toml` del proyecto, resuelve sus dependencias declaradas y usa el `Cargo.lock` del proyecto. La prueba host ejecuta una compilación real con `itoa` vendorizado y comprueba que el `E0308` de rustc aparece en `util_error.ox` con su rango UTF-16, no en el archivo activo; también verifica que `Cargo.lock` no cambia y que Restricted Mode no invoca Cargo. El comando se atiende de forma síncrona en el bucle LSP, así que una compilación larga puede retrasar otras solicitudes; falta hacerlo cancelable o asíncrono. |
| 5. `rust-analyzer` | Parcial avanzado: en workspace confiable el servidor mantiene un worker persistente con un único proceso `rust-analyzer`, resuelve módulos y genera un árbol Rust temporal controlado. Reenvía hover, completado e inlay hints sobre Rust generado; ubicaciones de archivos `.ox` mapeados regresan a sus URI/rangos de origen. Las consultas no bloquean el hilo del protocolo y tienen timeout. No se afirma mapeo semántico universal para expresiones sin correspondencia, módulos `.rs` mixtos ni respuestas que no contengan rangos. En Restricted Mode no se inicia. |
| 6. Navegación/refactorización | Parcial avanzado: definición, referencias y completado local recorren los `.ox` abiertos y los archivos `.ox` cerrados bajo la raíz del proyecto, conservando la URI del módulo de origen; una prueba host navega `util::saludo()` a `util.ox`. En workspace confiable también se reenvían a `rust-analyzer` sobre Rust generado. No hay renombrado ni un índice incremental persistente. |
| 7. Formato/acciones | Parcial: el proveedor LSP formatea modo llaves mediante la ruta de generación/importación validada y modo indentado con un formateador estructural del servidor. `Óxido: Comprobar con Cargo` y `Óxido: Ver estadísticas del editor` requieren un cliente activo; Cargo solo se ofrece en un workspace confiable. La conformance completa con `oxido formato` y la prueba de formato desde el host siguen pendientes. |
| 8. Workspace Trust | Parcial avanzado: `limited`, ejecutable restringido y sin rutas automáticas que invoquen Cargo o `rust-analyzer`. La prueba host arranca un VS Code 1.143.1 con perfil nuevo en Restricted Mode, ejecuta el comando Cargo y una solicitud LSP directa, y comprueba con un `cargo` falso en `PATH` que no se creó el marcador de proceso. También cubre el rechazo del servidor cuando la confianza no está en la inicialización. |
| 9. Cancelación/recursos | Parcial: `$/cancelRequest` para semantic tokens; diagnósticos reemplazados/cerrados se cancelan y sus resultados obsoletos se descartan. Límites explícitos: framing 4 MiB, cabecera 8 KiB, 64 documentos, 4 MiB por documento, 16 MiB total, cuatro solicitudes semantic tokens y una tarea diagnóstica en cola. Hay regresiones de límites y liberación de memoria al cerrar. El comando de estadísticas informa duración, round-trip y `VmRSS` del proceso LSP en Linux; faltan mediciones equivalentes multiplataforma desde host. |
| 10. Protocolo/instalación | Parcial: tests del crate, manifiesto y tres casos host reales (workspace confiable con `rust-analyzer`, Cargo confiable y Restricted Mode). Falta empaquetar/instalar un VSIX distribuible. |

## Evidencia host de Cargo

- `editors/vscode-oxido/test/fixtures/cargo/` declara `itoa` 1.0 y fija `Cargo.lock`. La prueba configura un `CARGO_HOME` temporal cuyo `config.toml` apunta al vendor versionado `tests/fixtures/cargo-registry/vendor`; no usa la caché del desarrollador ni la red.
- `check_error.ox` importa `util_error.ox`, que contiene `dejar numero: i32 = "texto"`. El comando `Óxido: Comprobar con Cargo` devuelve `success: false`, `dependencyCount: 1`, `lockfilePresent: true`, y un `E0308` en `util_error.ox` línea 3 (0-based), carácter 24, con rango final 31.
- La misma prueba calcula el SHA-256 de `Cargo.lock` antes y después y exige que no cambie. Si el vendor no existe, el runner falla con un mensaje explícito; no declara el caso aprobado.
- El caso Restricted Mode usa un perfil nuevo de VS Code, un `cargo` falso en `PATH` que escribiría un marcador, y comprueba que el marcador no aparece tras ejecutar el comando y una solicitud `oxido/check` directa.

## Verificaciones disponibles

- `cargo test -p oxido-lsp --offline`
- `cargo clippy -p oxido-lsp --all-targets --locked --offline -- -D warnings`
- `npm ci --prefix editors/vscode-oxido`
- `npm test --prefix editors/vscode-oxido`
- `npm run test:host --prefix editors/vscode-oxido` (tres casos: confiable, Cargo y restringido)

El job `vscode-extension` de CI compila `oxido-lsp`, ejecuta la suite de manifiesto y lanza VS Code 1.143.1 con un workspace confiable. Abre un `.ox`, recibe el diagnóstico `OX1002` real y solicita hover, estadísticas, completado, inlay hints y definición a través del cliente LSP real; además verifica que `util::saludo()` navega a `util.ox`. La prueba exige al menos una respuesta real de `rust-analyzer`, no solo el fallback local. El caso Cargo ejecuta una compilación real y el caso restringido demuestra que no se inicia Cargo. `OXIDO_HOST_ONLY=cargo` permite repetir un caso durante el desarrollo.

La suite host sí ejecuta VS Code, el bridge disponible de `rust-analyzer` y Cargo; las pruebas de protocolo y manifiesto no sustituyen pruebas de Cargo, mapeo semántico completo ni otras funciones pendientes del editor. La compilación Cargo del host usa una dependencia vendorizada concreta; no demuestra descarga desde crates.io ni compatibilidad general de dependencias.

## Instalación de desarrollo

1. Compila el servidor: `cargo build --release -p oxido-lsp`.
2. Asegúrate de que `oxido-lsp` esté en `PATH` o configura `oxido.serverPath` en ajustes de usuario.
3. Abre `editors/vscode-oxido` como proyecto de extensión con Extension Development Host, o instala el paquete VSIX cuando se publique.
4. Abre un proyecto `.ox` con `oxido.toml` válido o un archivo con cabecera `sintaxis indentacion`/`sintaxis llaves`.

La configuración leída desde el workspace solo selecciona el modo y está limitada por el parser de configuración; no contiene comandos. El análisis al editar es local. El comando Cargo requiere confianza explícita y usa `--locked --offline`; no habilita dependencias de usuario desde el LSP en este corte.
