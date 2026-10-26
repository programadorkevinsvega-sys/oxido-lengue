# ADR 0010: frontera del LSP y confianza del editor

- **Estado:** aceptado
- **Fecha:** 2026-10-26
- **Contexto:** Fase 11

## Contexto

Abrir un proyecto Rust puede ejecutar `build.rs` y macros procedurales si se invoca Cargo o se activa `rust-analyzer`. Workspace Trust de VS Code debe controlar esas rutas. El parser y lexer de Óxido pueden analizar texto local sin compilar el proyecto.

## Decisión

1. `oxido-lsp` comparte lexer/parser y diagnósticos del compilador; no inicia Cargo ni `rust-analyzer` al abrir o editar documentos. Un comando explícito `oxido/check` puede ejecutar el chequeo aislado de Cargo solo cuando el cliente envía confianza explícita; la extensión solo lo envía desde un workspace confiable.
2. El modo del proyecto se lee de `oxido.toml` como datos declarativos, limitado a 64 KiB y solo desde la raíz del workspace; los archivos con cabecera pueden determinar su modo cuando el proyecto no lo configura.
3. La extensión usa `Workspace Trust` en modo `limited`. El análisis estático puede operar en Restricted Mode. Cualquier función futura que compile o active macros/build scripts debe comprobar `workspace.isTrusted` en cada punto de entrada y solo ejecutarse por acción explícita.
4. La ruta configurable del ejecutable LSP es una configuración restringida en workspaces no confiables. Los procesos se lanzan con ejecutable y argumentos separados, nunca mediante shell.
5. La integración con `rust-analyzer` usa un worker persistente por proceso LSP, un workspace temporal controlado y Rust generado por el backend. Inicia el binario configurado (`OXIDO_RUST_ANALYZER` o `rust-analyzer`) solo con confianza explícita, reutiliza la sesión entre consultas, aplica timeout y limpia el proceso al cerrar el worker. Las posiciones se proyectan por ocurrencia de token; respuestas sin correspondencia no se inventan y usan el fallback local. Un error transitorio de `rust-analyzer` tampoco reemplaza el resultado local en hover, completado, referencias, símbolos o inlay hints.
6. El fallback local indexa declaraciones `.ox` con la estructura del lexer/parser, no con un analizador de tipos paralelo. Resuelve cualificadores de módulo y contenedor cuando la ruta es demostrable y devuelve todas las ubicaciones candidatas ante ambigüedad; no elige una coincidencia arbitraria. El formateador indentado es único y compartido por la CLI y el LSP.
7. `oxido/check` se ejecuta en un worker independiente con una sola comprobación concurrente. El bucle LSP conserva la capacidad de responder otras solicitudes y `$/cancelRequest` activa una bandera compartida que mata el grupo de procesos Cargo y evita lanzar pasos posteriores. Un resultado cancelado u obsoleto no publica diagnósticos.
8. El binario Cargo por defecto es `cargo`. `OXIDO_CARGO_BINARY` permite al operador del entorno sustituirlo de forma explícita; no es una configuración del workspace ni se acepta desde `oxido.toml`. La memoria residente se mide en Linux con `VmRSS`; en otras plataformas se informa como no disponible en lugar de estimarla.
9. El VSIX se empaqueta con `vsce` como herramienta de desarrollo y excluye pruebas, perfiles y artefactos de build. No incluye el binario `oxido-lsp` ni otro proceso dependiente de plataforma: el servidor se resuelve desde `PATH` o `oxido.serverPath`. El paquete local no se firma ni se publica desde este repositorio.

## Consecuencias

- La extensión puede ofrecer análisis básico sin ejecutar código del proyecto.
- La acción explícita `Óxido: Comprobar con Cargo` usa el `oxido.toml` y el `Cargo.lock` validados, ejecuta `cargo check --locked --offline` con confianza y devuelve los diagnósticos al `.ox` de origen. La prueba host cubre una dependencia vendorizada concreta y Restricted Mode; no demuestra descarga ni compatibilidad general de dependencias.
- La comprobación Cargo no bloquea el bucle LSP: corre en un worker con una sola solicitud concurrente, responde `-32002` si ya hay otra y devuelve `-32800` al cancelar. La prueba host usa un Cargo controlado con marcadores para demostrar que las estadísticas responden durante la compilación y que la cancelación evita la finalización del proceso. Una segunda comprobación concurrente no se pone en cola.
- `OXIDO_CARGO_BINARY` es una anulación del entorno del operador, análoga a `OXIDO_RUST_ANALYZER`; el workspace no puede establecerla. La prueba host la usa porque el `PATH` del proceso LSP hijo no conserva el shim del runner.
- La memoria residente solo tiene una medición real en Linux. En Windows y macOS la interfaz dice «no disponible»; no se extrapola `VmRSS` ni se presenta una estimación como medición.
- El VSIX 0.1.0 se instala con la CLI de VS Code en un perfil limpio y la prueba host exige que la extensión activa proceda del directorio de extensiones instaladas, que el paquete no contenga pruebas/perfiles y que un `.ox` reciba `OX1002` desde `oxido-lsp`. Esto demuestra instalación local, no firma, publicación ni distribución del binario LSP.
- `@vscode/vsce` es una dependencia de desarrollo y no forma parte del VSIX. Los `overrides` que fijan `@azure/identity` y `uuid` solo afectan al árbol de empaquetado; no cambian el código de la extensión ni se presentan como dependencias de ejecución.
- La inferencia de tipos y el bridge de `rust-analyzer` siguen limitados a las ubicaciones que el árbol Rust generado y el mapeo por token pueden justificar; las respuestas sin correspondencia usan el fallback local y no se inventan. El índice local tampoco sustituye la resolución semántica de Rust: solo demuestra nombres, contenedores y rutas estructurales.
- `textDocument/formatting` y `oxido formato` comparten implementación para el modo indentado; una divergencia entre ambos sería un defecto, no una variante admitida.
- La aceptación de Workspace Trust requiere pruebas reales en VS Code Restricted Mode y en un workspace confiable; el manifiesto por sí solo no prueba el comportamiento completo. El caso restringido se ejecuta con un perfil nuevo de VS Code y un `cargo` falso en `PATH` que comprueba la ausencia de procesos.
