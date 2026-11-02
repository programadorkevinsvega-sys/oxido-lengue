# Historial de cambios

## [Unreleased]

- Se reforzó la interoperabilidad con macros de Rust y la compilación reproducible con Cargo.
- La CI valida el MSRV declarado y ejecuta `cargo-deny`.
- Se publicó el modelo de amenazas y la matriz de límites de Fase 12; lexer y parser aceptan un plazo cooperativo de 10 s (`OX6001`) que el compilador, la CLI y el LSP aplican por fuente y por operación.
- `#![forbid(unsafe_code)]` cubre los seis crates y el binario del LSP; `oxido auditar-proyecto` acepta `--rechazar` para fallar ante usos detectados.
- La detección de caracteres confundibles incorpora alfabetos armenios, formas y dígitos de ancho completo.
- El índice del LSP restringe la caché a `0700`/`0600` en Unix y no la publica si falla la restricción de permisos.
- Lexer, parser y compilador limitan a 1.000 diagnósticos por fuente y resumen los omitidos con `OX6001`.
- Se integró `cargo-audit` 0.22.2 en CI y se documentó la política de dependencias y licencias en `docs/politica-dependencias.md`.
- Se añadió `tools/benchmark` con 13 escenarios de lexer, parser, generación y LSP, objetivos P95/RSS medidos y validación funcional en `--check`; el informe está en `docs/reports/fase-12/2026-10-30-benchmark-linux.md` y CI midió Linux, Windows y macOS.
- Se añadieron builds reproducibles verificados por SHA-256 (`tools/verify-reproducible-build.sh`), SBOM CycloneDX por crate (`tools/generate-sbom.sh`) y los jobs `reproducible`/`sbom`; el workflow de release los adjunta sin firmar ni publicar.
- La guía de distribución y los límites de reproducibilidad están en `docs/reproducibilidad.md`; firma, notarización y publicación siguen pendientes de credenciales y autorización.
- Se añadieron `tools/report-hygiene.sh` y `tools/audit-telemetry.sh`, ejecutados en CI, para bloquear rutas absolutas, credenciales y SDK de telemetría en informes y artefactos.
- Se corrigió la medición de RSS del benchmark: un hijo mide tiempos y otro, con una sola ejecución, el pico de memoria muestreado por el padre. El método anterior acumulaba páginas entre iteraciones y hacía fallar `lex-large` en macOS de forma intermitente.
- Se añadieron las guías de instalación, VS Code/LSP, educación y preparación empresarial, y se actualizaron las páginas de audiencia para reflejar el editor y el sandbox Linux reales.
- El build del sitio verifica los enlaces internos y reescribe los enlaces que salen de `docs/` hacia GitHub; un enlace roto falla el build y el nuevo job `documentation` de CI.
- Se incorporó `tools/playground` (`oxido-playground`) con sandbox Linux mediante `bwrap`/`prlimit`, límites de fuente, memoria, CPU, procesos, tiempo y salida, y fallo cerrado si el aislamiento no está disponible. La ejecución de extremo a extremo se verificó en un runner Ubuntu de GitHub y en un contenedor Linux anidado: programa ejecutado, red externa inaccesible desde el sandbox, ruta del host oculta y plazo de pared aplicado. La prueba descubrió y corrigió un `bwrap` duplicado, un límite de 1 GiB que impedía enlazar (quedó en 2 GiB) y un límite de 64 procesos insuficiente en el runner (quedó en 256). 12.11 se cerró con alcance Linux (ADR 0013); Windows/macOS y el despliegue público quedan fuera de la subfase.
