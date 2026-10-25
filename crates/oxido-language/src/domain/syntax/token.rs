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
        "dejar" | "let" => ("let", false),
        "bucle" | "loop" => ("loop", false),
        "coincidir" | "match" => ("match", false),
        "modulo" | "mod" => ("mod", false),
        "mover" | "move" => ("move", false),
        "mutable" | "mut" => ("mut", false),
        "publico" | "pub" => ("pub", false),
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
