# Óxido

Una sintaxis en español e inglés que busca generar Rust estándar y conservar su semántica, interoperabilidad y herramientas.

> **Estado del proyecto:** lexer, parser/AST inicial, generación Rust y compilación/ejecución de los modos de bloques para el subconjunto de funciones/control están implementados. Las Fases 5 y 6 tienen pruebas end-to-end; el lenguaje sigue limitado al subconjunto de la [matriz de compatibilidad](./compatibility).

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
