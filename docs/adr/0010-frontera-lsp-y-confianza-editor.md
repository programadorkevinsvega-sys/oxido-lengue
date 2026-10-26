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

## Consecuencias

- La extensión puede ofrecer análisis básico sin ejecutar código del proyecto.
- La inferencia de tipos, el bridge de `rust-analyzer` y las acciones dependientes de un Cargo.toml con dependencias quedan pendientes hasta implementar sus controles de confianza y mapeo.
- La aceptación de Workspace Trust requiere pruebas reales en VS Code Restricted Mode y en un workspace confiable; el manifiesto por sí solo no prueba el comportamiento completo.
