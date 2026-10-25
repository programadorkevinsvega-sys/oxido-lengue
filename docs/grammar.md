# Gramática inicial de Óxido

**Versión:** Óxido 0.1. Las producciones son normativas para estructura propia. `RustTokens` representa un fragmento balanceado preservado y validado con Syn/rustc; no pretende redefinir la gramática de Rust.

## 1. Convenciones

- `A ::= B` significa producción.
- `{ X }` significa cero o más repeticiones; `[ X ]`, opcional.
- `NL` es fin de línea lógico.
- Los comentarios no generan `NL` dentro de cadenas ni árboles de tokens.
- Los identificadores y literales siguen la gramática de tokens de Rust 2024 salvo alias y raw identifiers documentados.

## 2. Archivo y modo

```ebnf
SourceFile       ::= [Shebang] [Header NL] {BlankOrComment | Item | NL} EOF ;
Header           ::= "sintaxis" Mode ;
Mode             ::= "indentacion" | "llaves" ;
Item             ::= Attribute* [Visibility] StructuralItem
                   | PassthroughRustItem ;
StructuralItem   ::= FunctionItem | StructItem | EnumItem | ImplItem
                   | TraitItem | ModuleItem | UseItem ;
```

`Header` solo puede aparecer antes del primer item, después de shebang/comentarios iniciales. El lexer reconoce la declaración, configura el modo y la elimina de la salida Rust. `PassthroughRustItem` está admitido en modo llaves si Syn lo parsea y `rustc` lo acepta; en modo indentación solo se admite si el normalizador puede identificar de forma inequívoca sus bloques.

## 3. Items estructurales

```ebnf
FunctionItem     ::= ["asincrono"] ("funcion" | "fn") RustFunctionHeader FunctionBody ;
StructItem       ::= ("estructura" | "struct") RustItemHeader ItemBody ;
EnumItem         ::= ("enumeracion" | "enum") RustItemHeader ItemBody ;
ImplItem         ::= ("implementar" | "impl") RustItemHeader ItemBody ;
TraitItem        ::= ("rasgo" | "trait") RustItemHeader ItemBody ;
ModuleItem       ::= ("modulo" | "mod") Identifier ItemBody ;
UseItem          ::= ("usar" | "use") RustUseTree [";"] ;

FunctionBody     ::= BracedBlock | IndentedBlock ;
ItemBody         ::= BracedItemBlock | IndentedItemBlock ;
BracedBlock      ::= "{" {Statement | NL} "}" ;
IndentedBlock    ::= ":" NL INDENT {Statement | Item | NL} DEDENT ;
BracedItemBlock  ::= "{" {Item | Member | NL} "}" ;
IndentedItemBlock ::= ":" NL INDENT {Item | Member | NL} DEDENT ;
```

`RustFunctionHeader`, `RustItemHeader` y `RustUseTree` son secuencias de tokens Rust balanceadas hasta el delimitador estructural. El frontend no cambia sus identificadores ni literales. Syn valida el fragmento en su contexto final.

En modo indentación, el `:` final solo abre un bloque después de un encabezado reconocido y al nivel de delimitadores cero. Los dos puntos de parámetros, tipos, paths y expresiones no abren bloques.

## 4. Instrucciones y bloques

```ebnf
Statement        ::= LetStatement | ReturnStatement | BreakStatement
                   | ContinueStatement | ControlStatement | ExprLine ;
LetStatement     ::= ("dejar" | "let") RustPattern [":" RustType]
                     "=" RustExpr [";"] ;
ReturnStatement  ::= ("retornar" | "return") [RustExpr] [";"] ;
BreakStatement   ::= ("romper" | "break") [RustLabel] [RustExpr] [";"] ;
ContinueStatement ::= ("continuar" | "continue") [RustLabel] [";"] ;
ControlStatement ::= IfStatement | LoopStatement | WhileStatement | ForStatement ;
IfStatement     ::= ("si" | "if") RustExpr Block
                     [("sino" | "else") (IfStatement | Block)] ;
LoopStatement   ::= ("bucle" | "loop") Block ;
WhileStatement  ::= ("mientras" | "while") RustExpr Block ;
ForStatement    ::= ("para" | "for") RustPattern ("en" | "in") RustExpr Block ;
Block           ::= BracedBlock | IndentedBlock ;
ExprLine        ::= RustExpr [";"] ;
```

`RustExpr`, `RustPattern` y `RustType` son fragmentos balanceados Rust tras normalizar keywords fuera de regiones opacas. Syn valida la categoría esperada; `rustc` valida tipos, nombres y semántica. `match`/`coincidir` y sus brazos se escriben como expresión Rust con `{}` en 0.1; no se define aún una sintaxis indentada para brazos.

## 5. Reglas de salida de bloques indentados

El normalizador inserta terminadores según la estructura:

- `let` y items que Rust exige terminar con `;` reciben el terminador necesario.
- Una `ExprLine` anterior a otra instrucción del mismo bloque se emite como statement con `;` si la gramática Rust lo requiere.
- La última `ExprLine` sin `;` es la expresión tail y conserva su valor.
- La última `ExprLine` con `;` es statement y deja `()` como valor del bloque.
- `return`, `break` y `continue` se emiten como expresiones de control Rust; su semicolon normalizado no altera el tipo `!` ni el flujo.
- Items y bloques vacíos son válidos cuando Rust los permite.

La dedentación cierra un bloque; nunca decide por sí sola si la última expresión tenía `;`.

## 6. Fragmentos Rust y árboles opacos

```ebnf
RustTokens       ::= Token {Token | BalancedGroup} ;
BalancedGroup    ::= "(" TokenTree* ")" | "[" TokenTree* "]" | "{" TokenTree* "}" ;
MacroInvocation  ::= RustPath "!" BalancedGroup [";"] ;
Attribute        ::= "#" ["!"] "[" OpaqueTokenTree* "]" ;
```

Las invocaciones de macros y el contenido de atributos son opacos: no se traducen alias, identificadores, cadenas ni rutas internas. La sintaxis de macro invocation se basa en los árboles delimitados de Rust ([Rust Reference: Macro Invocation](https://doc.rust-lang.org/reference/macros.html#macro-invocation)). El nombre de macro también se preserva; Óxido 0.1 no define alias de macros.

## 7. Normalización antes de Syn

1. Tokenizar sin alterar literales/comentarios y reconocer grupos balanceados.
2. Reconocer alias solo como tokens de keyword no crudos y fuera de macro/attribute opacos.
3. Normalizar bloques indentados a grupos `{}` conservando un mapa de origen.
4. Normalizar terminadores de instrucciones indentadas conforme a la sección 5.
5. Construir Rust tokens/AST y parsear el archivo con Syn (`full`, `parsing`).
6. Dejar validación semántica y de tipos a Cargo/rustc.

La gramática de expresiones y tipos es la de la edición Rust fijada, salvo las normalizaciones explícitas de esta especificación. No existe traducción textual global.
