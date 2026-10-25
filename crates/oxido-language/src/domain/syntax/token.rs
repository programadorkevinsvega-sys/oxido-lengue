use crate::domain::source::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxMode {
    Indentation,
    Braces,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier {
        raw: bool,
    },
    Underscore,
    Keyword {
        rust_keyword: &'static str,
        reserved: bool,
    },
    Number,
    StringLiteral,
    CharacterLiteral,
    Lifetime,
    Comment,
    Punctuation,
    Newline,
    Indent,
    Dedent,
    ModeHeader(SyntaxMode),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    /// Indica tokens dentro de argumentos de macros o árboles de atributos.
    pub opaque: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct KeywordInfo {
    pub rust_keyword: &'static str,
    pub reserved: bool,
}

pub(crate) fn keyword(word: &str) -> Option<KeywordInfo> {
    KEYWORD_ALIASES
        .iter()
        .find_map(|(alias, rust_keyword, reserved)| {
            (*alias == word).then_some(KeywordInfo {
                rust_keyword,
                reserved: *reserved,
            })
        })
        .or_else(|| {
            KEYWORD_ALIASES
                .iter()
                .find_map(|(_, rust_keyword, reserved)| {
                    (*rust_keyword == word).then_some(KeywordInfo {
                        rust_keyword,
                        reserved: *reserved,
                    })
                })
        })
}

/// Palabras traducidas por el lexer. Es la fuente para traducción y documentación.
pub const KEYWORD_ALIASES: &[(&str, &str, bool)] = &[
    ("como", "as", false),
    ("asincrono", "async", false),
    ("esperar", "await", false),
    ("romper", "break", false),
    ("constante", "const", false),
    ("continuar", "continue", false),
    ("paquete", "crate", false),
    ("dinamico", "dyn", false),
    ("sino", "else", false),
    ("enumeracion", "enum", false),
    ("externo", "extern", false),
    ("falso", "false", false),
    ("funcion", "fn", false),
    ("para", "for", false),
    ("si", "if", false),
    ("implementar", "impl", false),
    ("en", "in", false),
    ("dejar", "let", false),
    ("bucle", "loop", false),
    ("coincidir", "match", false),
    ("modulo", "mod", false),
    ("mover", "move", false),
    ("mutable", "mut", false),
    ("publico", "pub", false),
    ("principal", "main", false),
    ("referencia", "ref", false),
    ("retornar", "return", false),
    ("mismo", "self", false),
    ("Mismo", "Self", false),
    ("estatico", "static", false),
    ("estructura", "struct", false),
    ("superior", "super", false),
    ("rasgo", "trait", false),
    ("verdadero", "true", false),
    ("tipo", "type", false),
    ("inseguro", "unsafe", false),
    ("usar", "use", false),
    ("donde", "where", false),
    ("mientras", "while", false),
    ("abstracto", "abstract", true),
    ("convertirse", "become", true),
    ("caja", "box", true),
    ("hacer", "do", true),
    ("final", "final", true),
    ("generar", "gen", true),
    ("macro", "macro", true),
    ("sobrescribir", "override", true),
    ("privado", "priv", true),
    ("intentar", "try", true),
    ("tipo_de", "typeof", true),
    ("sin_tamano", "unsized", true),
    ("virtual", "virtual", true),
    ("ceder", "yield", true),
];

/// Devuelve la palabra Rust equivalente a un alias reconocido por el lexer.
/// La salida permite que adapters normalicen fragmentos sin volver a mantener
/// una tabla bilingüe paralela.
#[must_use]
pub fn keyword_translation(word: &str) -> Option<&'static str> {
    keyword(word).map(|info| info.rust_keyword)
}

/// Alias del prelude de Rust. Los identificadores crudos nunca se traducen.
#[must_use]
pub fn prelude_translation(word: &str) -> Option<&'static str> {
    lookup_alias(PRELUDE_ALIASES, word)
}

/// Alias `texto` solo en posición de tipo; fuera de ella es un identificador normal.
#[must_use]
pub fn type_translation(word: &str) -> Option<&'static str> {
    (word == "texto").then_some("str")
}

/// Alias de tipos/variantes del prelude; fuente única para el traductor y su documentación.
pub const PRELUDE_ALIASES: &[(&str, &str)] = &[
    ("Cadena", "String"),
    ("Vector", "Vec"),
    ("Opcion", "Option"),
    ("Alguno", "Some"),
    ("Ninguno", "None"),
    ("Resultado", "Result"),
    ("Bien", "Ok"),
    ("Falla", "Err"),
    ("Caja", "Box"),
    ("MapaHash", "HashMap"),
    ("booleano", "bool"),
    ("caracter", "char"),
    ("Defecto", "Default"),
    ("Iterador", "Iterator"),
];

/// Alias admitidos para nombres de métodos habituales.
#[must_use]
pub fn method_translation(word: &str) -> Option<&'static str> {
    lookup_alias(METHOD_ALIASES, word)
}

/// Alias de métodos del prelude; fuente única para el traductor y su documentación.
pub const METHOD_ALIASES: &[(&str, &str)] = &[
    ("nuevo", "new"),
    ("esta_vacio", "is_empty"),
    ("agregar", "push"),
    ("sacar", "pop"),
    ("obtener", "get"),
    ("insertar", "insert"),
    ("quitar", "remove"),
    ("contiene", "contains"),
    ("vaciar", "clear"),
    ("largo", "len"),
    ("iterar", "iter"),
    ("iterar_mutable", "iter_mut"),
    ("mapear", "map"),
    ("filtrar", "filter"),
    ("recolectar", "collect"),
    ("encontrar", "find"),
    ("sumar", "sum"),
    ("ordenar", "sort"),
    ("clonar", "clone"),
    ("desenvolver", "unwrap"),
    ("exigir", "expect"),
    ("es_alguno", "is_some"),
    ("es_ninguno", "is_none"),
    ("a_mayusculas", "to_uppercase"),
    ("a_minusculas", "to_lowercase"),
    ("a_cadena", "to_string"),
];

/// Renderiza las tablas normativas de tipos y métodos para `docs/aliases.md`.
#[must_use]
pub fn render_alias_markdown() -> String {
    let mut markdown = String::from(
        "<!-- BEGIN GENERATED ALIASES -->\n## Keywords estrictas\n\n| Rust 2024 | Alias Óxido |\n|---|---|\n",
    );
    for (alias, rust_name, reserved) in KEYWORD_ALIASES {
        if !reserved {
            markdown.push_str(&format!("| `{rust_name}` | `{alias}` |\n"));
        }
    }
    markdown.push_str("\n## Keywords reservadas sin semántica Óxido\n\n| Rust 2024 | Alias reservado |\n|---|---|\n");
    for (alias, rust_name, reserved) in KEYWORD_ALIASES {
        if *reserved {
            markdown.push_str(&format!("| `{rust_name}` | `{alias}` |\n"));
        }
    }
    markdown.push_str("\n## Macros estándar\n\n| Óxido | Rust |\n|---|---|\n");
    for (alias, rust_name) in MACRO_ALIASES {
        markdown.push_str(&format!("| `{alias}!` | `{rust_name}!` |\n"));
    }
    markdown.push_str("\n## Tipos y variantes del prelude\n\n| Óxido | Rust |\n|---|---|\n");
    for (alias, rust_name) in PRELUDE_ALIASES {
        markdown.push_str(&format!("| `{alias}` | `{rust_name}` |\n"));
    }
    markdown.push_str("| `texto` (solo tipo) | `str` |\n");
    markdown.push_str("\n### Métodos del prelude\n\n| Óxido | Rust |\n|---|---|\n");
    for (alias, rust_name) in METHOD_ALIASES {
        if *alias == "nuevo" {
            markdown.push_str("| `Vector::nuevo()` | `Vec::new()` |\n");
        } else {
            markdown.push_str(&format!("| `.{alias}()` | `.{rust_name}()` |\n"));
        }
    }
    markdown.push_str("\n### Módulos de `std`\n\n| Óxido | Rust |\n|---|---|\n");
    for (alias, rust_name) in STD_MODULE_ALIASES {
        markdown.push_str(&format!("| `{alias}` (tras `std::`) | `{rust_name}` |\n"));
    }
    markdown.push_str("\n### Atributos\n\n| Óxido | Rust |\n|---|---|\n");
    for (alias, rust_name) in ATTRIBUTE_ALIASES {
        markdown.push_str(&format!("| `#[{alias}]` | `#[{rust_name}]` |\n"));
    }
    markdown.push_str("<!-- END GENERATED ALIASES -->");
    markdown
}

fn lookup_alias(
    aliases: &'static [(&'static str, &'static str)],
    word: &str,
) -> Option<&'static str> {
    aliases
        .iter()
        .find_map(|(alias, rust_name)| (*alias == word).then_some(*rust_name))
}

/// Nombre de macro traducido; los argumentos se mantienen como tokens opacos.
#[must_use]
pub fn macro_translation(word: &str) -> Option<&'static str> {
    lookup_alias(MACRO_ALIASES, word)
}

/// Alias únicos de macros estándar; fuente para traducción y documentación.
pub const MACRO_ALIASES: &[(&str, &str)] = &[
    ("imprimir", "println"),
    ("imprimir_sin_salto", "print"),
    ("imprimir_error", "eprintln"),
    ("imprimir_error_sin_salto", "eprint"),
    ("formato", "format"),
    ("argumentos_formato", "format_args"),
    ("escribir", "write"),
    ("escribir_linea", "writeln"),
    ("vector", "vec"),
    ("panico", "panic"),
    ("afirmar", "assert"),
    ("afirmar_igual", "assert_eq"),
    ("afirmar_distinto", "assert_ne"),
    ("afirmar_debug", "debug_assert"),
    ("afirmar_igual_debug", "debug_assert_eq"),
    ("afirmar_distinto_debug", "debug_assert_ne"),
    ("depurar", "dbg"),
    ("pendiente", "todo"),
    ("sin_implementar", "unimplemented"),
    ("inalcanzable", "unreachable"),
    ("coincide", "matches"),
    ("incluir_texto", "include_str"),
    ("incluir_bytes", "include_bytes"),
    ("entorno", "env"),
    ("entorno_opcional", "option_env"),
    ("concatenar", "concat"),
    ("a_texto", "stringify"),
    ("linea", "line"),
    ("archivo", "file"),
    ("columna", "column"),
    ("config", "cfg"),
    ("error_de_compilacion", "compile_error"),
    ("local_del_hilo", "thread_local"),
];

/// Alias contextual de módulos de `std`.
#[must_use]
pub fn std_module_translation(word: &str) -> Option<&'static str> {
    lookup_alias(STD_MODULE_ALIASES, word)
}

/// Alias de módulos admitidos solo en rutas prefijadas por `std::`.
pub const STD_MODULE_ALIASES: &[(&str, &str)] = &[
    ("colecciones", "collections"),
    ("archivos", "fs"),
    ("es", "io"),
    ("hilo", "thread"),
];

/// Alias de atributos admitidos por el backend.
pub const ATTRIBUTE_ALIASES: &[(&str, &str)] = &[
    ("prueba", "test"),
    ("permitir", "allow"),
    ("derivar", "derive"),
];

/// Devuelve el atributo Rust equivalente a un alias documentado.
#[must_use]
pub fn attribute_translation(word: &str) -> Option<&'static str> {
    lookup_alias(ATTRIBUTE_ALIASES, word)
}
