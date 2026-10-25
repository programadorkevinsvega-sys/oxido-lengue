# ADR-0001: Parser estructural con fragmentos Rust preservados

- **Estado:** Aceptada
- **Fecha:** 2026-10-25
- **Fase:** 4, parser y AST

## Contexto

Óxido busca introducir alias y bloques alternativos sin redefinir la semántica de Rust. Implementar una gramática completa de Rust aumentaría el riesgo de incompatibilidad y duplicaría trabajo que ya cubre el ecosistema. El frontend sí necesita reconocer la estructura que transforma: modo, items admitidos, funciones, bloques, declaraciones y control.

## Decisión

El parser propio analiza únicamente esa estructura y conserva firmas, expresiones, tipos, patrones, macros y otros fragmentos Rust como texto/span de origen. El backend valida fragmentos sintácticos compatibles con Syn. La aceptación semántica definitiva corresponde a `rustc` cuando exista generación e integración Cargo.

El AST representa explícitamente la expresión final sin `;` como `tail`; una expresión con `;` permanece como statement. Macros y atributos se conservan opacos y no se traducen por defecto. Los items Rust passthrough de esta fase se limitan al modo llaves.

## Alternativas consideradas

1. **Implementar un parser completo de Rust:** se descarta por duplicación, coste y riesgo de divergir de Rust.
2. **Tratar todo el archivo como Rust y traducir texto globalmente:** se descarta porque no reconoce de forma segura bloques, aliases ni contexto de macros.
3. **Parser estructural Óxido + fragmentos Rust opacos:** aceptada; acota la gramática propia y mantiene la validación Rust delegada al backend oficial.

## Consecuencias

- La compatibilidad crece por subconjuntos documentados y pruebas, no por asumir que un fragmento parseable ya compila.
- Syn valida sintaxis, pero no tipos, macros expandidas, `build.rs` ni semántica de `rustc`.
- La Fase 4 no genera `.rs`. La generación con tokens, los mapas de posiciones y la integración Cargo pertenecen a la Fase 5.
- La gramática de items en modo indentación, `match`, continuaciones generales y normalización de aliases dentro de fragmentos siguen pendientes.
- Cambiar esta frontera requiere actualizar la matriz de compatibilidad, los fixtures y este ADR.

## Evidencia

La implementación está descrita en [Fase 4](../fase-4-parser.md) y su alcance por característica en [compatibilidad](../compatibility.md). Las pruebas del parser y del pipeline verifican AST, recuperación y validación de fragmentos con Syn.
