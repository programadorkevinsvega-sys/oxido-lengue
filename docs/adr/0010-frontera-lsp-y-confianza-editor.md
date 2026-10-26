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
5. La integración con `rust-analyzer` usa un worker persistente por proceso LSP, un workspace temporal controlado y Rust generado por el backend. Inicia el binario configurado (`OXIDO_RUST_ANALYZER` o `rust-analyzer`) solo con confianza explícita, reutiliza la sesión entre consultas, aplica timeout y limpia el proceso al cerrar el worker. Las posiciones se proyectan por ocurrencia de token; respuestas sin correspondencia no se inventan y usan el fallback local.
6. El fallback local indexa declaraciones `.ox` con la estructura del lexer/parser, no con un analizador de tipos paralelo. Resuelve cualificadores de módulo y contenedor cuando la ruta es demostrable y devuelve todas las ubicaciones candidatas ante ambigüedad; no elige una coincidencia arbitraria. El formateador indentado es único y compartido por la CLI y el LSP.

## Consecuencias

- La extensión puede ofrecer análisis básico sin ejecutar código del proyecto.
- La acción explícita `Óxido: Comprobar con Cargo` usa el `oxido.toml` y el `Cargo.lock` validados, ejecuta `cargo check --locked --offline` con confianza y devuelve los diagnósticos al `.ox` de origen. La prueba host cubre una dependencia vendorizada concreta y Restricted Mode; no demuestra descarga ni compatibilidad general de dependencias.
- La inferencia de tipos y el bridge de `rust-analyzer` siguen limitados a las ubicaciones que el árbol Rust generado y el mapeo por token pueden justificar; las respuestas sin correspondencia usan el fallback local y no se inventan. El índice local tampoco sustituye la resolución semántica de Rust: solo demuestra nombres, contenedores y rutas estructurales.
- `textDocument/formatting` y `oxido formato` comparten implementación para el modo indentado; una divergencia entre ambos sería un defecto, no una variante admitida.
- La aceptación de Workspace Trust requiere pruebas reales en VS Code Restricted Mode y en un workspace confiable; el manifiesto por sí solo no prueba el comportamiento completo. El caso restringido se ejecuta con un perfil nuevo de VS Code y un `cargo` falso en `PATH` que comprueba la ausencia de procesos.
