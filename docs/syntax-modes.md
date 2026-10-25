# Modos de sintaxis

## Configuración normativa

El proyecto puede fijar un único modo en `oxido.toml`:

```toml
[language]
syntax = "indentation" # o "braces"
edition = "2024"
```

Si el proyecto no fija el modo, cada archivo `.ox` debe empezar con `sintaxis indentacion` o `sintaxis llaves`. Si el proyecto sí lo fija, la declaración de archivo es opcional y, si aparece, debe coincidir. Nunca se detecta el modo por inspección del cuerpo ni se cambia dentro del archivo.

## Indentación

- La indentación se compone de espacios ASCII, cuatro por nivel.
- Un tabulador en el prefijo de indentación es error; dentro de literales/grupos conserva el tratamiento Rust.
- Mezclar niveles no múltiplos de cuatro o dedentar a un nivel inexistente es error.
- Líneas vacías y líneas solo con comentarios no modifican el stack.
- LF y CRLF son finales de línea válidos; la fuente conserva sus offsets originales.
- Los grupos `()`, `[]`, literales, comentarios y árboles de macro pueden abarcar líneas sin dedentar el bloque externo.
- Una línea continúa en la siguiente si hay un grupo abierto o si termina con operador/puntuación de continuación (`=`, `,`, `.`, `::`, `->`, `=>`, `+`, `-`, `*`, `/`, `%`, `&&`, `||`, `&`, `|`, `^`, comparador, `as`, `where`). También continúa si la siguiente línea comienza con `.` o `::`. No existe escape `\` de fin de línea en 0.1.
- Un bloque se abre con `:` como último token no comentario de un encabezado estructural reconocido.
- El bloque cierra al volver a un nivel anterior; una dedentación irregular produce diagnóstico en el primer espacio discrepante.
- `{}` puede aparecer en un grupo Rust (por ejemplo, struct literal o macro) si el parser lo clasifica como payload. No sustituye delimitadores de bloques estructurales de Óxido.

### Valor de bloque

La última expresión de un bloque, si no lleva `;`, es el valor del bloque. Si lleva `;`, es una instrucción y el valor es `()`. Una expresión anterior a otra línea se considera statement; el normalizador agrega el terminador Rust necesario. Cada variante debe tener prueba de tipo/valor.

```ox
funcion valor() -> i32:
    42

funcion unidad():
    42;
```

## Llaves

- Los bloques estructurales usan `{` y `}`.
- La indentación visual no cambia la estructura.
- Los puntos y coma y terminadores siguen la gramática Rust 2024.
- Un `:` de bloque es error; los dos puntos legítimos dentro de tipos, campos, paths y expresiones se conservan.
- Las líneas pueden partir expresiones según la gramática de Rust; no se infieren statements a partir de saltos de línea.

## Mezclas y casos especiales

Un delimitador contrario al modo produce `OX2002` solo cuando ocupa la posición de un delimitador de bloque. Los delimitadores de macros, atributos, literales, struct literals y otras expresiones se tratan según el contexto y no se rechazan por sí solos. Las declaraciones de bloque en modo indentation usan `:`; en modo braces usan `{}`.

La extensión de VS Code, el formateador y el LSP leen la misma configuración y consumen el mismo análisis. Formatear nunca cambia el modo.
