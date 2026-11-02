# Preparación empresarial

Esta página describe lo que una organización puede verificar hoy y lo que todavía no debe asumir. **Óxido no es un producto listo para producción, no está certificado y no ofrece SLA.** No sustituye una auditoría, una revisión legal ni una prueba de concepto controlada.

## Estado de evaluación

- El compilador, la CLI y el LSP están en etapa temprana. La compatibilidad se limita a la [matriz publicada](compatibility.md).
- No hay firma, notarización, publicación en marketplaces ni binarios distribuidos oficialmente. La subfase 12.9 se cerró sin publicar.
- El sandbox del playground está verificado solo en Linux y no cubre la CLI ni el LSP. No está desplegado.
- El código del repositorio se ofrece con doble licencia **MIT o Apache-2.0** (`LICENSE-MIT` y `LICENSE-APACHE`). Cada crate declara `license = "MIT OR Apache-2.0"`. El titular puede cambiar esta decisión, y la revisión legal del uso previsto sigue siendo responsabilidad de cada organización.
- La documentación distingue entre lo implementado, lo especificado y lo propuesto. Los casos de uso no son compromisos de producto.

## Controles verificados

| Área | Control | Evidencia | Límite |
|---|---|---|---|
| Entrada | Fuente máxima de 4 MiB, 1.000.000 de tokens, anidamiento de 256 y plazo cooperativo de 10 s | [Fase 12](fase-12-seguridad.md), pruebas de lexer y parser | No demuestra corrección semántica ni cubre todo Rust |
| Memoria y CPU | Benchmarks con objetivos P95/RSS en Linux, Windows y macOS | [Informe de benchmark](reports/fase-12/2026-10-30-benchmark-linux.md) | Runners de CI no son producción; el RSS muestreado es una cota inferior |
| `unsafe` | `#![forbid(unsafe_code)]` en los crates internos; política `--rechazar` para código analizado | [Fase 12](fase-12-seguridad.md) | El análisis léxico no detecta toda la superficie de riesgo |
| Rutas | Canonicalización, rechazo de symlinks en el índice, caché fuera del workspace | [Fase 12](fase-12-seguridad.md), pruebas del LSP | No sustituye controles del sistema operativo |
| Procesos | `Command` con argumentos separados, sin shell; cancelación que mata el árbol de Cargo | [Fase 12](fase-12-seguridad.md) | Depende de las garantías del sistema al matar procesos |
| Editor | Workspace Trust `limited`; Restricted Mode no inicia Cargo ni `rust-analyzer` | [Guía de VS Code](vscode.md), [Fase 11](fase-11-vscode-lsp.md) | El comando Cargo autorizado no está sandboxeado |
| Dependencias | `cargo-audit`, `cargo-deny`, `--locked`, revisión de licencias de terceros | [Política de dependencias](politica-dependencias.md), CI | La base RustSec y la detección de licencias tienen límites |
| Reproducibilidad | Dos builds release con el mismo SHA-256 y SBOM CycloneDX sin rutas absolutas | [Reproducibilidad](reproducibilidad.md) | Verificado en Linux; la matriz completa no se ha ejecutado |
| Informes | `report-hygiene.sh` bloquea rutas absolutas y credenciales; `audit-telemetry.sh` revisa SDK y endpoints | CI, [Fase 12](fase-12-seguridad.md) | Comprobaciones estáticas, no una auditoría dinámica de red |
| Telemetría | Sin SDK ni envío remoto por defecto | [Fase 12](fase-12-seguridad.md) | El usuario puede añadir dependencias que envíen datos |
| Playground | `bwrap`/`prlimit`, red sin acceso, límites y fallo cerrado | [Informe del sandbox](reports/fase-12/2026-10-31-sandbox-linux.md), [ADR 0013](adr/0013-cierre-12-11-alcance-linux.md) | Solo Linux, sin despliegue, sin autenticación ni cuotas |

## Riesgos residuales

1. **Ejecución de código de dependencias.** `compilar` y `ejecutar` con dependencias pueden ejecutar `build.rs` y macros procedurales. `--confiar-dependencias` registra una decisión del operador; no aísla el proceso.
2. **Sin sandbox general.** La CLI y el LSP no están aislados del sistema operativo. El sandbox del playground es un producto separado.
3. **Cobertura parcial del lenguaje.** La matriz no cubre todo Rust. Un programa que compila no demuestra compatibilidad general.
4. **Distribución incompleta.** No hay binarios firmados, instaladores, marketplace ni canal de actualizaciones.
5. **Licencia y revisión legal.** El código usa MIT o Apache-2.0, pero cada organización debe revisar si esa licencia cubre su uso previsto. Las licencias de terceros se auditan con `cargo-deny`, no con una revisión legal completa.
6. **Sin soporte ni SLA.** No hay compromiso de respuesta, ventanas de mantenimiento ni compatibilidad retroactiva.
7. **Datos del entorno.** Los informes pueden contener salidas de diagnóstico; la higiene bloquea rutas absolutas y secretos conocidos, pero no todos los formatos.

## Lista de evaluación

Antes de un piloto interno:

- [ ] Revisar que `LICENSE-MIT`/`LICENSE-APACHE` cubren el uso previsto y conservar la atribución requerida.
- [ ] Fijar un commit y un toolchain reproducibles.
- [ ] Revisar la matriz de compatibilidad y elegir un caso de uso dentro de ella.
- [ ] Ejecutar `cargo test --workspace --locked`, `cargo audit` y `cargo deny check`.
- [ ] Verificar el build reproducible y el SBOM del commit elegido.
- [ ] Revisar la política de dependencias y las licencias de terceros.
- [ ] Decidir si se permite compilar dependencias y en qué entorno aislado.
- [ ] Configurar Workspace Trust solo para repositorios revisados.
- [ ] Comprobar que no hay secretos ni rutas privadas en los artefactos.
- [ ] Definir un plan de reversión y un responsable interno.

Antes de producción:

- [ ] Auditoría de seguridad independiente del compilador y de la cadena de suministro.
- [ ] Firma y notarización de artefactos, con verificación de actualizaciones.
- [ ] Sandbox o aislamiento para compilaciones con dependencias.
- [ ] Matriz de compatibilidad ampliada y probada en cargas reales.
- [ ] Licencia, soporte, SLA y proceso de vulnerabilidades acordados.
- [ ] Revisión legal de licencias de terceros y del uso previsto.

## Manejo de datos

- El proyecto no incluye telemetría ni cliente de red en sus crates.
- La CLI lee archivos del proyecto y escribe en el directorio de salida indicado.
- La descarga de dependencias consulta el índice y ejecuta `cargo fetch` solo tras `--confirmar`.
- El LSP escucha por stdio, no por red. El playground escucha en `127.0.0.1` por defecto.
- Los informes de CI se publican como artefactos y pasan por la comprobación de higiene.

## Preguntas para el proveedor

1. ¿Qué commit, toolchain y versión exacta se evaluaron?
2. ¿Qué casos de la matriz están cubiertos por pruebas y cuáles no?
3. ¿Cómo se ejecutan las dependencias y qué aislamiento existe?
4. ¿Qué licencia se aplica al compilador, a la CLI y a los ejemplos? (MIT o Apache-2.0 en este repositorio.)
5. ¿Qué artefactos están firmados y cómo se verifica su integridad?
6. ¿Qué soporte y SLA existen?
7. ¿Cómo se reportan y corrigen vulnerabilidades?
8. ¿Qué datos salen del entorno y con qué consentimiento?

## Contacto y vulnerabilidades

No publiques detalles explotables en un issue público. Consulta [`SECURITY.md`](https://github.com/programadorkevinsvega-sys/oxido-lengue/blob/main/SECURITY.md) y el [modelo de amenazas](fase-12-seguridad.md).
