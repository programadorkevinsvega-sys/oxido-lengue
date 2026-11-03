# Fase 13: documentación y material educativo

**Fecha:** 2026-11-02

**Estado:** alcance documental en curso. Este informe registra lo construido y verificado; no declara cerrada la fase porque los pilotos educativos y la validación con estudiantes siguen sin evidencia.

## Objetivo de la revisión

Convertir capacidades ya verificadas en material utilizable: instalación, editor, tutorial, educación, evaluación empresarial, licencia y publicación del sitio desde los Markdown canónicos.

## Cambios

- Nuevas guías canónicas: [`docs/instalacion.md`](../../instalacion.md), [`docs/vscode.md`](../../vscode.md), [`docs/education.md`](../../education.md) y [`docs/enterprise-readiness.md`](../../enterprise-readiness.md).
- Nuevo [`docs/tutorial.md`](../../tutorial.md) con tres lecciones ejecutables: saludo, condiciones y estructuras con métodos.
- Ejemplos `examples/tutorial/01_saludo.ox`, `02_condiciones.ox` y `03_punto.ox`, con una prueba que compila y ejecuta cada uno.
- Doble licencia MIT o Apache-2.0 mediante `LICENSE-MIT` y `LICENSE-APACHE`; `license = "MIT OR Apache-2.0"` en el workspace y en cada crate.
- Páginas de audiencia actualizadas para describir el LSP y el sandbox Linux reales, sin presentarlos como prototipos ni como productos desplegados.
- El build del sitio reescribe los enlaces que salen de `docs/` hacia GitHub, publica solo la documentación de usuario y ejecuta `scripts-check-links.mjs`. Los informes internos, ADR, prompts y roadmap quedan fuera del sitio público.
- Nuevo job `documentation` en CI con Node 22.23.4; el check forma parte de la protección de `main`.
- Favicon propio en `website/public/favicon.svg`.

## Evidencia ejecutada

| Comprobación | Resultado |
|---|---|
| `cargo fmt --all -- --check` | Sin cambios pendientes |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Sin advertencias |
| `cargo test --workspace --locked` | 222 pruebas, 0 fallos |
| `tools/report-hygiene.sh docs/reports` | Sin secretos ni rutas absolutas |
| `tools/audit-telemetry.sh` | Sin SDK ni endpoints de analítica |
| `npm --prefix website run build` | 57 páginas, enlaces internos sin destinos rotos |
| `npm --prefix website run check` | 0 errores |
| `cargo install --path crates/oxido-cli --locked` | Instala el binario `oxido` |
| `oxido nuevo` y ejecución de los ejemplos documentados | Salidas `Hola, Óxido.`, `Hola, mundo.`, `5:1`, `5:1:3:5`, `valor 4: cuatro` |
| Tutorial | Salidas `Hola, mundo!`, tres líneas de clasificación y `(2, 3) suma 5` |

Los comandos se ejecutaron en Linux con Rust 1.99.1. La CLI también se prueba en Windows y macOS en CI; las guías no afirman que el sandbox del playground funcione fuera de Linux.

## Corrección de una prueba intermitente

El job `reference-regressions` falló una vez al esperar el servidor de referencia: el sondeo recibía una conexión TCP sin respuesta HTTP completa y abortaba con `BadStatusLine` antes del plazo de 30 s. El sondeo ahora trata `http.client.HTTPException` como reintento, igual que los errores de red, sin cambiar el criterio de éxito. La ejecución posterior pasó.

## Lo que no se ha hecho

- No hay pilotos educativos ejecutados ni datos de estudiantes.
- No se ha validado el material con docentes.
- No existe una guía bilingüe mantenida; el sitio publica solo la locale `es`.
- No se ha revisado legalmente el uso previsto de la licencia ni las licencias de terceros más allá de `cargo-deny`.
- No se ha verificado el despliegue de Cloudflare Pages desde este entorno; la configuración documentada es la existente.
- La documentación puede desviarse de la implementación si cambia el compilador sin actualizarla.

## Conclusión

Las subfases documentales —instalación, tutorial, educación, evaluación empresarial, licencia, publicación desde `docs/` y verificación de enlaces en CI— cuentan con material y evidencia reproducible. La Fase 13 sigue en curso porque los pilotos y la validación educativa requieren participantes y análisis que no existen todavía.
