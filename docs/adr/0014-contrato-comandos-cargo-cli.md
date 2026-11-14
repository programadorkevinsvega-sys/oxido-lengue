# ADR-0014: Contrato de comandos Cargo de la CLI

- **Estado:** Aceptada
- **Fecha:** 2026-11-13
- **Ámbito:** CLI de Óxido y workspaces Cargo importados

## Contexto

La CLI usaba `compilar` para validar con `cargo check`. Eso no dejaba un ejecutable, aunque los usuarios esperan que compilar un proyecto pueda producir el binario que Cargo produce. La validación, construcción, ejecución y pruebas son acciones distintas y deben ser visibles desde la CLI.

## Decisión

- `check`/`verificar` ejecuta `cargo check` y valida sin prometer un binario.
- `build`/`compilar` genera el proyecto Cargo en `target/oxido/` y ejecuta `cargo build`; `--release` selecciona el perfil optimizado de Cargo. Para proyectos `.ox`, los artefactos persisten bajo `target/oxido/target/debug` o `target/oxido/target/release`.
- `run`/`ejecutar` compila y ejecuta el programa.
- `test`/`probar` ejecuta `cargo test`; no admite `--release` en el contrato actual.
- En workspaces importados, `check` valida los targets del workspace y `build` construye los paquetes del workspace. Los binarios quedan en el `target/oxido` del proyecto importado, no en el staging temporal.
- Todas las acciones usan Cargo con argumentos separados, `--locked` y `--offline`. La CLI no convierte la generación release en una promesa de aptitud para producción: la configuración, operación y seguridad del servicio siguen bajo responsabilidad del proyecto.

## Consecuencias

- `compilar` cambia a ser alias de `build`; scripts que solo querían validar deben usar `check`.
- Los proyectos pueden producir binarios debug y release sin invocar Cargo manualmente.
- La CLI informa la ruta del directorio de artefactos. El JSON de workspaces importados no está cubierto por el esquema JSON estructurado de proyectos de un paquete.
- `--release` usa las opciones estándar de Cargo/rustc; Óxido no mantiene un perfil de optimización propio.

## Evidencia

`cargo test --workspace --locked` incluye pruebas CLI de check sin ejecutable, build debug/release persistente y build release de un workspace importado. La matriz de comandos y su alcance se documentan en [Fase 9](../roadmap.md#fase-9-cli-y-experiencia-de-proyecto) y [testing.md](../testing.md).
