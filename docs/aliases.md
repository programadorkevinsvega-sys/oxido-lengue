# Alias, keywords e identificadores

## Reglas

- Rust es sensible a mayúsculas: `mismo` y `Mismo` representan `self` y `Self` respectivamente.
- Los alias son tokens de keyword solo en forma no escapada y en contexto de código.
- Literales, comentarios, documentación, atributos y árboles de macro se preservan.
- Los nombres de usuario, métodos, tipos y crates no se traducen.
- `r#nombre` fuerza un identificador literal llamado `nombre`, sin alias. Ejemplo: `r#tipo` es una variable llamada `tipo`; no es el keyword `type`.
- Si se usa un alias reservado sin escape como identificador, emitir `OX3001`: «`tipo` es palabra reservada de Óxido; usa `r#tipo` o cámbialo».
- La forma cruda respeta restricciones de identificadores Rust; Rust no permite `r#_`, `r#crate`, `r#self`, `r#Self` ni `r#super` ([Rust Reference: Identifiers](https://doc.rust-lang.org/reference/identifiers.html#raw-identifiers)).

## Keywords estrictas

| Rust 2024 | Alias Óxido |
|---|---|
| `as` | `como` |
| `async` | `asincrono` |
| `await` | `esperar` |
| `break` | `romper` |
| `const` | `constante` |
| `continue` | `continuar` |
| `crate` | `paquete` |
| `dyn` | `dinamico` |
| `else` | `sino` |
| `enum` | `enumeracion` |
| `extern` | `externo` |
| `false` | `falso` |
| `fn` | `funcion` |
| `for` | `para` |
| `if` | `si` |
| `impl` | `implementar` |
| `in` | `en` |
| `let` | `dejar` |
| `loop` | `bucle` |
| `match` | `coincidir` |
| `mod` | `modulo` |
| `move` | `mover` |
| `mut` | `mutable` |
| `pub` | `publico` |
| `ref` | `referencia` |
| `return` | `retornar` |
| `self` | `mismo` |
| `Self` | `Mismo` |
| `static` | `estatico` |
| `struct` | `estructura` |
| `super` | `superior` |
| `trait` | `rasgo` |
| `true` | `verdadero` |
| `type` | `tipo` |
| `unsafe` | `inseguro` |
| `use` | `usar` |
| `where` | `donde` |
| `while` | `mientras` |
| `_` | `_` (sin traducción) |

## Keywords reservadas sin semántica Óxido

Estas formas siguen reservadas y no se convierten en características. Su uso como sintaxis produce `OX4001` (construcción Rust reservada/no soportada); como identificador requieren `r#` cuando Rust lo permita.

| Rust 2024 | Alias reservado |
|---|---|
| `abstract` | `abstracto` |
| `become` | `convertirse` |
| `box` | `caja` |
| `do` | `hacer` |
| `final` | `final` |
| `gen` | `generar` |
| `macro` | `macro` |
| `override` | `sobrescribir` |
| `priv` | `privado` |
| `try` | `intentar` |
| `typeof` | `tipo_de` |
| `unsized` | `sin_tamano` |
| `virtual` | `virtual` |
| `yield` | `ceder` |

Rust 2024 reserva `gen`; el inventario debe actualizarse al cambiar la edición objetivo ([Rust Reference: Keywords](https://doc.rust-lang.org/reference/keywords.html#reserved-keywords)).

## Keywords débiles/contextuales conservadas

Se mantienen en inglés y con reglas contextuales de Rust: `'static`, `macro_rules`, `raw`, `safe`, `union`. No se les asigna alias español en 0.1.

## Macros y atributos

Óxido 0.1 no da alias a macros (`println!`, `vec!`, etc.). Conserva el nombre y todo el árbol de tokens de la invocación, incluida `matches!`; no normaliza `si`, `tipo` u otros alias dentro de argumentos. Los tokens de atributos se preservan igualmente. Esto protege macros externas de la traducción de su lenguaje interno. Rust especifica una invocación como path, `!` y árbol delimitado ([Rust Reference: Macro Invocation](https://doc.rust-lang.org/reference/macros.html#macro-invocation)).
