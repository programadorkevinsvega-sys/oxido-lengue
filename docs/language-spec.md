# Especificación del lenguaje Óxido 0.1

**Estado:** contrato normativo de Óxido 0.1. Describe el diseño del lenguaje; la implementación verificada por característica está en [compatibility.md](compatibility.md), y no se infiere del texto de esta especificación.
**Fuente normativa:** esta especificación y los documentos enlazados abajo.  
**Edición objetivo de Rust:** 2024.

## 1. Propósito

Óxido es una sintaxis bilingüe español/inglés para producir Rust estándar. No tiene runtime, type checker, reglas de ownership ni sistema de paquetes propios. Cargo y `rustc` resuelven compilación, tipos, traits, borrowing, macros y linking.

```text
.ox → lexer/normalizador de Óxido → normalización estructural → Rust tokens/AST
    → validación sintáctica → Cargo/rustc → binario o biblioteca Rust
```

Una construcción solo se anuncia como soportada cuando aparece en la matriz de compatibilidad y supera sus pruebas de aceptación.

## 2. Documentos normativos

- [Gramática inicial](grammar.md)
- [Modos de sintaxis](syntax-modes.md)
- [Alias e identificadores](aliases.md)
- [Matriz de compatibilidad](compatibility.md)
- [Diagnósticos y posiciones](diagnostics.md)
- [Pipeline y generación](compiler-pipeline.md)
- [Criterios de aceptación](testing.md)
- [Arquitectura](architecture.md) y [reglas estrictas](../ARCHITECTURE_RULES.md)

## 3. Perfil inicial

La especificación define dos modos de bloques: `indentation` y `braces`. Un proyecto usa un modo; un archivo no puede cambiarlo. Ambos se traducen a un AST común y al mismo backend.

El frontend propio analiza la selección del modo, alias, balance de grupos, elementos estructurales de Óxido y bloques. En el subconjunto implementado, los aliases se normalizan a tokens Rust fuera de DSL opacas; macros estándar de expresiones admiten aliases anidados. Syn valida la sintaxis y `rustc` el programa completo. La gramática de expresiones, tipos y patrones sigue siendo Rust, no una gramática española independiente.

## 4. Configuración

Configuración de proyecto:

```toml
[language]
syntax = "indentation" # o "braces"
edition = "2024"
```

`syntax` es opcional solo si cada archivo `.ox` empieza con `sintaxis indentacion` o `sintaxis llaves`. Si el proyecto define `syntax`, la declaración inicial por archivo también es opcional, pero, si aparece, debe coincidir. No existe detección automática ni override por archivo. `edition` admite inicialmente solo `2024`.

`oxido.toml` contiene datos declarativos; no admite código, comandos ni scripts.

## 5. Principios semánticos

1. Los alias son tokens de keyword, no reemplazos de texto.
2. Identificadores de usuario, cadenas, comentarios, documentación, rutas y nombres de crates se conservan.
3. Los argumentos de macros desconocidas/externas y DSL de metaprogramación se conservan opacos. Los macros estándar expresivos allowlisted pueden normalizar recursivamente aliases en sus expresiones; las cadenas y literales no se cambian.
4. Un identificador crudo `r#nombre` es un identificador de usuario cuyo nombre es `nombre`, incluso cuando coincide con un alias Óxido. No activa una keyword.
5. En bloques indentados, la expresión final sin `;` es el valor del bloque. Añadir `;` la convierte en instrucción y el bloque vale `()`.
6. En modo llaves, los terminadores siguen las reglas Rust; no se infieren puntos y coma por saltos de línea.
7. Las keywords no crean capacidades Rust inexistentes. Palabras reservadas futuras permanecen reservadas y sin semántica Óxido.
8. Tipos, referencias, ownership, borrowing, lifetimes, macros y ejecución conservan semántica Rust.
9. Las variables locales pueden omitir su tipo cuando `rustc` lo infiere estáticamente. Óxido no implementa inferencia ni un sistema de tipos propios. Las firmas de funciones y métodos siguen las reglas de Rust: los parámetros llevan tipo y los retornos se escriben cuando Rust los exige; Óxido no infiere firmas.
10. El editor mostrará tipos inferidos consultando a `rust-analyzer` sobre el Rust generado y traduciendo sus posiciones al `.ox` con el mapa de fuente. No se simulan tipos en el LSP de Óxido.

## 6. No objetivos de 0.1

- Traducir nombres de tipos, métodos, funciones de bibliotecas o crates.
- Un prelude de aliases de tipos/métodos.
- Expansión propia de macros y traducción de DSLs de macros externas.
- Un type checker o borrow checker propio.
- Inferencia de tipos propia, incluida la inferencia de firmas de funciones. Cualquier propuesta en esa dirección requiere una decisión explícita que actualice esta especificación.
- Aceptar automáticamente toda gramática Rust dentro de bloques indentados.
- Afirmar compatibilidad general sin pruebas por construcción y dependencia. La interoperabilidad amplia con Rust sí es el objetivo; el alcance probado de 0.1 solo delimita la evidencia disponible.
- Telemetría remota por defecto.
- Convertir el proyecto Óxido en una API/backend o añadir infraestructura que la herramienta local no necesite.

El contrato de preservación, interoperabilidad con bibliotecas/macros, `async`/`await` y `unsafe` está en [ADR 0007](adr/0007-contrato-interoperabilidad-rust.md).

## 7. Compatibilidad de edición

La edición objetivo es Rust 2024. El inventario de palabras estrictas, reservadas y contextuales sigue la referencia de Rust. `gen` es reservado desde Rust 2024 y se incluye en la tabla de keywords reservadas ([Rust Reference: Keywords](https://doc.rust-lang.org/reference/keywords.html), [Edition Guide: `gen`](https://doc.rust-lang.org/edition-guide/rust-2024/gen-keyword.html)). Los identificadores crudos siguen la forma `r#ident`; Rust excluye de ella `_`, `crate`, `self`, `Self` y `super` ([Rust Reference: Identifiers](https://doc.rust-lang.org/reference/identifiers.html)).
