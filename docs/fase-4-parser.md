# Fase 4: parser estructural inicial y AST

**Estado:** completada para el subconjunto inicial que la fase entrega. El alcance se cierra aquí; las ampliaciones que aparecen como parciales en la matriz de compatibilidad no se presentan como implementadas.

## Alcance implementado

- AST compartido por los modos de indentación y llaves, con spans de fuente y bloque con estilo explícito.
- Funciones top-level en ambos modos, incluidas visibilidad, `async` y atributos.
- En modo llaves, admite passthrough de `struct`, `enum`, `impl`, `trait`, `mod`, `use`, `type`, `const`, `static`, `extern`, `union`, `macro_rules!` e invocaciones de macro, incluso con atributos/visibilidad; Syn valida el fragmento y su contenido opaco se conserva.
- En cuerpos de función: `let`, `return`, `break`, `continue`, expresiones Rust, `if`/`else if`/`else`, `loop`, `while` y `for`.
- Firmas, tipos, patrones, expresiones y items preservados con sus spans de origen; el backend valida las categorías Rust cubiertas con Syn. `break`, `continue` y cabeceras `for` también reciben validación sintáctica.
- Expresión final sin `;` representada como `tail`; con `;` permanece como statement. Las pruebas confirman esta regla en ambos estilos.
- Diagnósticos con spans, recuperación de funciones posteriores y límite de 128 niveles de bloques y cadenas `else if` para evitar desbordamiento de pila.
- Colisiones de alias reservados en nombres de función y bindings simples de `let` producen `OX3001` con sugerencia `r#...` cuando Rust permite ese raw identifier.

## Criterios y evidencia

`crates/oxido-language/tests/parser.rs` cubre funciones en ambos modos, AST de bloques/controles, paridad de semicolon/tail, atributos, visibilidad, passthrough, macros, Unicode, recuperación, nombres reservados y límites de recursión. `crates/oxido-compiler/tests/pipeline.rs` verifica con Syn items passthrough, todos los controles iniciales en ambos modos, atributos inválidos y diagnósticos para cabeceras `for` y saltos malformados.

La revisión de cierre ejecuta formato, check, Clippy y la suite completa con `--locked`. No se declara generación de Rust ni compilación Cargo: son entregables de Fase 5.

## Límites explícitos del parser inicial

- Items superiores distintos de funciones y passthrough no están admitidos en modo indentación. Los items con cuerpos indentados (`struct`, `enum`, `impl`, `trait`, módulos) pertenecen a la ampliación de lenguaje de Fase 7.
- En el cierre de Fase 4 no se normalizaban aliases dentro de fragmentos Rust; la normalización para el subconjunto de Fase 6 está documentada en [informe de Fase 6](fase-6-sintaxis-bilinguismo.md). Los tokens opacos de macros/atributos siguen sin normalizarse.
- `match` se conserva como expresión Rust con brazos entre llaves; no existe sintaxis indentada para sus brazos.
- Syn valida sintaxis, no resolución, tipos, macros expandidas, `build.rs` ni semántica de `rustc`.

La matriz por característica está en [compatibilidad](compatibility.md). La decisión de separar estructura Óxido de fragmentos Rust está en [ADR-0001](adr/0001-parser-hibrido.md). Fase 5 añadió generación `.ox → .rs`, mapas de posiciones e integración Cargo; los cambios de Fase 6 están registrados en su informe.
