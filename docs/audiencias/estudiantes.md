# Óxido para estudiantes

Óxido busca ofrecer una sintaxis bilingüe apoyándose en Rust. La meta es poder aprender estructuras de programación y, al mismo tiempo, encontrar las reglas reales de Rust para tipos, referencias, ownership y borrowing.

> Existe un parser para el subconjunto inicial, pero Óxido aún no genera ni compila `.ox`. Los fragmentos siguientes ilustran la sintaxis y no se deben presentar como programas ejecutables en la versión actual.

## Una función propuesta

```ox
sintaxis indentacion
funcion saludar(nombre: &str) -> String:
    format!("Hola, {nombre}")
```

La expresión `format!` seguiría siendo una macro Rust y sus argumentos se conservarían sin traducir. La última expresión sin `;` sería el valor del bloque según la especificación.

## Ruta de aprendizaje

1. Variables, expresiones y funciones.
2. Tipos y mutabilidad de Rust.
3. Condiciones y bucles.
4. Referencias, ownership y borrowing.
5. Errores del compilador y lectura del Rust generado.

Los tutoriales y ejercicios se habilitarán conforme cada característica tenga implementación y pruebas. Consulta [modos de sintaxis](/referencia/syntax-modes/), [alias](/referencia/aliases/) y [criterios de aceptación](/referencia/testing/).
