# ADR 0011: cierre técnico de Fase 11 y traslado de la distribución

- **Estado:** aceptado
- **Fecha:** 2026-10-26
- **Contexto:** cierre de Fase 11

## Contexto

La Fase 11 debía demostrar la integración editor/LSP con pruebas reproducibles. Al revisar el alcance quedaron tres categorías distintas:

1. Capacidades del editor y del LSP que ya se pueden probar localmente: semantic tokens, navegación, formato compartido, diagnósticos, Cargo bajo confianza, cancelación y límites.
2. Evidencia semántica de `rust-analyzer` sobre Rust generado y su mapeo de vuelta al `.ox` de origen.
3. Cadena de distribución: matriz Windows/macOS, firma, notarización, publicación, SBOM, builds reproducibles y mediciones de latencia/memoria fuera de Linux.

La tercera categoría no se puede cerrar sin cuentas, credenciales, autorización de publicación y ejecución real en otras plataformas. Mantenerla dentro de Fase 11 impedía declarar cerrado lo que sí estaba verificado y mezclaba evidencia local con promesas de distribución.

## Decisión

1. La Fase 11 se cierra técnicamente cuando las capacidades del editor/LSP están cubiertas por pruebas del crate y por la suite host real de VS Code. La evidencia host debe comprobar contenido semántico, no solo que una solicitud devuelva una forma válida.
2. La clasificación de semantic tokens usa el índice de declaraciones de los documentos abiertos. Los nombres declarados, sus referencias cualificadas y los módulos abiertos reciben la clase correspondiente; lo que no se puede resolver conserva el fallback léxico. Los archivos cerrados siguen disponibles para definición, hover, referencias y símbolos, pero no se recorren en cada edición para colorear tokens.
3. No se declara resolución de tipos universal. Un parámetro o un enlace local sin declaración indexada conserva una clase léxica; los tipos y firmas profundos siguen delegados en `rust-analyzer` cuando el workspace es confiable. El índice tampoco modela ámbitos locales: si un enlace local comparte nombre con una declaración indexada, el token puede recibir la clase de esa declaración. Esta limitación se documenta y no se presenta como resolución de tipos.
4. `rename` y un índice incremental persistente quedan fuera de Fase 11. Un nombre ambiguo sigue devolviendo todas las candidatas y no se elige una arbitrariamente.
5. La distribución, la firma, la notarización, la publicación, el SBOM, los builds reproducibles y los benchmarks de latencia/memoria fuera de Linux pasan a Fase 12. El workflow de release existente es preparación, no evidencia de ejecución multiplataforma.
6. `rust-analyzer` sigue siendo un servicio auxiliar: un error transitorio o una respuesta vacía de navegación no reemplaza el resultado local acotado cuando el índice puede resolverlo.

## Consecuencias

- La Fase 11 puede declararse completa en su alcance técnico con evidencia local reproducible y límites explícitos.
- El VSIX estándar y el VSIX Linux x64 con `server/oxido-lsp` están verificados localmente; no se presentan como paquetes firmados, notarizados o publicados.
- La matriz Windows/macOS y la publicación quedan bloqueadas hasta que existan credenciales y autorización; su ausencia no invalida las pruebas del editor en Linux.
- Los benchmarks de latencia y la memoria Windows/macOS no bloquean el cierre técnico; se medirán en Fase 12 antes de prometer rendimiento o soporte multiplataforma.
- Los usuarios deben poder distinguir entre navegación sobre archivos cerrados (índice del proyecto) y clasificación de tokens sobre archivos abiertos (índice de edición).

## Evidencia

- `cargo test -p oxido-lsp --locked`: 50 pruebas, incluida la clasificación semántica de módulos, tipos, funciones y referencias cualificadas.
- Suite host de VS Code 1.143.1: workspace confiable con `rust-analyzer`, Cargo real vendorizado, navegación/formato en Restricted Mode, cancelación Cargo, VSIX estándar, VSIX con servidor incluido y Restricted Mode con `cargo` falso.
- La prueba host solicita semantic tokens sobre `semantic_tokens.ox` y exige tipos concretos para declaraciones y referencias; además cubre completado, inlay hints, símbolos, referencias, edición incremental, documento incompleto y tres solicitudes concurrentes de tokens.
- El informe canónico es [Fase 11](../fase-11-vscode-lsp.md); los límites de distribución se registran en [Fase 12](../roadmap.md#fase-12-seguridad-y-calidad-avanzada).
