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
    let (rust_keyword, reserved) = match word {
        "como" | "as" => ("as", false),
        "asincrono" | "async" => ("async", false),
        "esperar" | "await" => ("await", false),
        "romper" | "break" => ("break", false),
        "constante" | "const" => ("const", false),
        "continuar" | "continue" => ("continue", false),
        "paquete" | "crate" => ("crate", false),
        "dinamico" | "dyn" => ("dyn", false),
        "sino" | "else" => ("else", false),
        "enumeracion" | "enum" => ("enum", false),
        "externo" | "extern" => ("extern", false),
        "falso" | "false" => ("false", false),
        "funcion" | "fn" => ("fn", false),
        "para" | "for" => ("for", false),
        "si" | "if" => ("if", false),
        "implementar" | "impl" => ("impl", false),
        "en" | "in" => ("in", false),
        "dejar" | "variable" | "let" => ("let", false),
        "bucle" | "loop" => ("loop", false),
        "coincidir" | "match" => ("match", false),
        "modulo" | "mod" => ("mod", false),
        "mover" | "move" => ("move", false),
        "mutable" | "mut" => ("mut", false),
        "publico" | "pub" => ("pub", false),
        "principal" => ("main", false),
        "referencia" | "ref" => ("ref", false),
        "retornar" | "return" => ("return", false),
        "mismo" | "self" => ("self", false),
        "Mismo" | "Self" => ("Self", false),
        "estatico" | "static" => ("static", false),
        "estructura" | "struct" => ("struct", false),
        "superior" | "super" => ("super", false),
        "rasgo" | "trait" => ("trait", false),
        "verdadero" | "true" => ("true", false),
        "tipo" | "type" => ("type", false),
        "inseguro" | "unsafe" => ("unsafe", false),
        "usar" | "use" => ("use", false),
        "donde" | "where" => ("where", false),
        "mientras" | "while" => ("while", false),
        "abstracto" | "abstract" => ("abstract", true),
        "convertirse" | "become" => ("become", true),
        "caja" | "box" => ("box", true),
        "hacer" | "do" => ("do", true),
        "final" => ("final", true),
        "generar" | "gen" => ("gen", true),
        "macro" => ("macro", true),
        "sobrescribir" | "override" => ("override", true),
        "privado" | "priv" => ("priv", true),
        "intentar" | "try" => ("try", true),
        "tipo_de" | "typeof" => ("typeof", true),
        "sin_tamano" | "unsized" => ("unsized", true),
        "virtual" => ("virtual", true),
        "ceder" | "yield" => ("yield", true),
        _ => return None,
    };
    Some(KeywordInfo {
        rust_keyword,
        reserved,
    })
}

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
    Some(match word {
        "Cadena" => "String",
        "Vector" => "Vec",
        "Opcion" => "Option",
        "Alguno" => "Some",
        "Ninguno" => "None",
        "Resultado" => "Result",
        "Bien" => "Ok",
        "Error" => "Err",
        "Caja" => "Box",
        "MapaHash" => "HashMap",
        "booleano" => "bool",
        "caracter" => "char",
        "texto" => "str",
        "Defecto" => "Default",
        "Iterador" => "Iterator",
        _ => return None,
    })
}

/// Alias admitidos para nombres de métodos habituales.
#[must_use]
pub fn method_translation(word: &str) -> Option<&'static str> {
    Some(match word {
        "desenvolver" => "unwrap",
        "esperar_o" => "expect",
        "agregar" => "push",
        "largo" => "len",
        "iterar" => "iter",
        "mapear" => "map",
        "filtrar" => "filter",
        "recolectar" => "collect",
        "clonar" => "clone",
        "a_cadena" => "to_string",
        "es_alguno" => "is_some",
        _ => return None,
    })
}

/// Nombre de macro traducido; los argumentos se mantienen como tokens opacos.
#[must_use]
pub fn macro_translation(word: &str) -> Option<&'static str> {
    Some(match word {
        "imprimirln" => "println",
        "imprimir" => "print",
        "formato" => "format",
        "vector" => "vec",
        "panico" => "panic",
        "afirmar" => "assert",
        "afirmar_igual" => "assert_eq",
        "pendiente" => "todo",
        "no_implementado" => "unimplemented",
        "inalcanzable" => "unreachable",
        "depurar" => "dbg",
        _ => return None,
    })
}

/// Alias contextual de módulos de `std`.
#[must_use]
pub fn std_module_translation(word: &str) -> Option<&'static str> {
    Some(match word {
        "colecciones" => "collections",
        "archivos" => "fs",
        "es" => "io",
        "hilo" => "thread",
        _ => return None,
    })
}
