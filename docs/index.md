# Óxido

Una sintaxis en español e inglés que busca generar Rust estándar y conservar su semántica, interoperabilidad y herramientas.

> **Estado del proyecto:** lexer, parser/AST inicial, generación y compilación Rust, modos de bloques, items indentados iniciales e interoperabilidad con Rust están implementados para los subconjuntos verificados de Fases 5–8. La Fase 8 está completada para su matriz inicial; la CLI no sandboxa el código de dependencias y el LSP aún debe implementar Workspace Trust. Los límites por característica están en la [matriz de compatibilidad](./compatibility) y el [roadmap](./roadmap); no se declara compatibilidad general.

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
- [Roadmap](./roadmap)
- [Casos de uso](./audiencias/casos-de-uso)

## Principio de documentación

Cada página distingue entre **implementado y verificado**, **especificado pero pendiente** y **propuesta futura**. Un ejemplo no demuestra compatibilidad: las características implementadas deben enlazar a pruebas reproducibles.
