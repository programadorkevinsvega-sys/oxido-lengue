# ADR-0008: Contrato de formato de Óxido y Rust generado

- **Estado:** Aceptada
- **Fecha:** 2026-10-26
- **Fase:** 9, CLI y experiencia de proyecto

## Contexto

Óxido tiene sintaxis y reglas de formato propias. A la vez, los archivos Rust exportados deben integrarse directamente con Cargo y respetar el formato canónico de `rustfmt`. Aplicar `rustfmt` al texto `.ox` confundiría ambos contratos; formatear el Rust con reglas propias rompería la expectativa de herramientas Rust.

## Decisión

1. `oxido formato` aplica las convenciones de Óxido al `.ox`, preservando comentarios, literales y grupos opacos.
2. El backend genera Rust desde tokens/AST y aplica `rustfmt` 1.99.1, edición 2024, al `.rs` exportado.
3. Para cada construcción estructural admitida en la matriz, el Rust generado debe coincidir carácter por carácter con la salida de esa versión fijada de `rustfmt`. Esta promesa cubre la matriz, no toda expresión Rust ni futuras versiones de `rustfmt`.
4. Una modificación del formateador requiere fixtures de regresión que comprueben idempotencia de `.ox`, preservación de contenido y conformidad del Rust exportado; los casos ejecutables además se compilan.

## Consecuencias

- El aspecto del `.ox` no tiene que ser idéntico al Rust; las sintaxis tienen delimitadores y palabras clave distintos.
- La salida Rust queda lista para inspección y uso con herramientas estándar, dentro de la cobertura publicada.
- Actualizar la versión de `rustfmt` requiere actualizar la matriz y la evidencia de forma explícita.
- Comentarios y tokens opacos no se reescriben para satisfacer una regla de espaciado general.

## Evidencia

La cobertura y los comandos están en [Fase 9 y su matriz](../roadmap.md#fase-9-cli-y-experiencia-de-proyecto) y en [testing.md](../testing.md#revisión-de-cobertura-estructural-admitida). La CI prueba cinco grupos estructurales contra `rustfmt` 1.99.1, compila los exports y ejecuta el fixture ejecutable.
