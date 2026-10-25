use crate::domain::{source::Span, syntax::SyntaxMode};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct File {
    pub mode: SyntaxMode,
    pub items: Vec<Item>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Function(Box<Function>),
    RustFragment(RustFragment),
    IndentedAggregate(IndentedAggregate),
    IndentedUse(IndentedUse),
    IndentedContainer(IndentedContainer),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerKind {
    Impl,
    Trait,
    Module,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndentedContainer {
    pub kind: ContainerKind,
    pub attributes: Vec<RustFragment>,
    pub visibility: Option<RustFragment>,
    pub header: RustFragment,
    pub members: Vec<Item>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndentedUse {
    pub attributes: Vec<RustFragment>,
    pub visibility: Option<RustFragment>,
    pub tree: RustFragment,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AggregateKind {
    Struct,
    Enum,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndentedAggregate {
    pub kind: AggregateKind,
    pub attributes: Vec<RustFragment>,
    pub visibility: Option<RustFragment>,
    /// Nombre, genéricos y cláusula `where`, preservados como Rust.
    pub header: RustFragment,
    pub members: Vec<IndentedMember>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndentedMember {
    pub attributes: Vec<RustFragment>,
    /// Campo de struct o variante enum, sin coma final.
    pub value: RustFragment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub attributes: Vec<RustFragment>,
    pub visibility: Option<RustFragment>,
    pub asynchronous: bool,
    pub name: String,
    /// Parámetros genéricos, argumentos, retorno y cláusula `where`, preservados como Rust.
    pub signature: RustFragment,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockStyle {
    Indentation,
    Braces,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub style: BlockStyle,
    pub statements: Vec<Statement>,
    /// Expresión final sin `;`; si es `None`, el bloque vale `()` salvo semántica Rust externa.
    pub tail: Option<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Let(LetStatement),
    Return(ControlStatement),
    Break(ControlStatement),
    Continue(ControlStatement),
    Control(ControlExpression),
    Expression(Expression),
    RustItem(RustFragment),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LetStatement {
    pub pattern: RustFragment,
    pub type_annotation: Option<RustFragment>,
    pub initializer: RustFragment,
    pub match_initializer: Option<Box<ControlExpression>>,
    /// Refleja el delimitador de origen; un backend puede añadir el `;` requerido por Rust.
    pub had_semicolon: bool,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlStatement {
    pub value: Option<RustFragment>,
    pub had_semicolon: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlKind {
    If,
    Loop,
    While,
    For,
    Match,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlExpression {
    pub kind: ControlKind,
    /// Condición, patrón o cabecera Rust conservada sin normalización textual.
    pub header: RustFragment,
    pub body: Block,
    pub match_arms: Vec<MatchArm>,
    pub alternative: Option<ControlAlternative>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArm {
    pub pattern: RustFragment,
    pub expression: Option<RustFragment>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlAlternative {
    Block(Block),
    ElseIf(Box<ControlExpression>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Expression {
    pub value: RustFragment,
    pub had_semicolon: bool,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustFragment {
    pub text: String,
    pub span: Span,
}
