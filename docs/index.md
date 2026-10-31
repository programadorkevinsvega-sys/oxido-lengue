# Óxido

Una sintaxis en español e inglés que busca generar Rust estándar y conservar su semántica, interoperabilidad y herramientas.

> **Estado del proyecto:** lexer, parser/AST inicial, generación y compilación Rust, modos de bloques, items indentados, CLI inicial, importación/exportación de árboles y la interoperabilidad Rust verificada están implementados para los subconjuntos documentados. El LSP ofrece transporte stdio, diagnósticos y semantic tokens; todavía no inicia compilaciones sin Workspace Trust. La CLI no sandboxa código de dependencias. Los límites por característica están en la [matriz de compatibilidad](./compatibility) y el [roadmap](./roadmap); no se declara compatibilidad general.

<div class="audience-grid">
  <a class="audience-card" href="/audiencias/desarrolladores"><strong>Desarrolladores</strong><span>Arquitectura, compatibilidad con Rust y estado de la toolchain.</span></a>
  <a class="audience-card" href="/audiencias/empresas"><strong>Empresas</strong><span>Alcance, controles existentes y criterios para evaluar el proyecto.</span></a>
  <a class="audience-card" href="/audiencias/estudiantes"><strong>Estudiantes</strong><span>Conceptos, ejemplos comentados y ruta de aprendizaje.</span></a>
</div>

## Explorar

- [Especificación del lenguaje](./language-spec)
- [Gramática inicial](./grammar)
- [Modos de sintaxis](./syntax-modes)
- [Matriz de compatibilidad](./compatibility)
- [Builds reproducibles y SBOM](./reproducibilidad)
- [Benchmark de análisis y LSP](./reports/fase-12/2026-10-30-benchmark-linux)
- [Roadmap](./roadmap)
- [Decisiones de arquitectura (ADR)](./adr/)
- [Casos de uso](./audiencias/casos-de-uso)

## Principio de documentación

Cada página distingue entre **implementado y verificado**, **especificado pero pendiente** y **propuesta futura**. Un ejemplo no demuestra compatibilidad: las características implementadas deben enlazar a pruebas reproducibles.
