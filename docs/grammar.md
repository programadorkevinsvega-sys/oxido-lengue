# Gramática inicial de Óxido

**Versión:** Óxido 0.1. Las producciones son normativas para estructura propia. `RustTokens` representa un fragmento balanceado preservado y validado con Syn/rustc; no pretende redefinir la gramática de Rust.

La gramática describe el alcance previsto de 0.1; no significa que todas las producciones estén implementadas en cada fase. El subconjunto comprobado está enumerado en los informes de [Fase 4](fase-4-parser.md), [Fase 6](fase-6-sintaxis-bilinguismo.md), [Fase 7](fase-7-lenguaje-inicial.md) y [Fase 8](fase-8-integracion-rust.md), además de la matriz de [compatibilidad](compatibility.md). La Fase 8 incorpora la primera sintaxis indentada de `impl`, `trait`, módulos y brazos `match`; véase el informe para límites.

## 1. Convenciones

- `A ::= B` significa producción.
- `{ X }` significa cero o más repeticiones; `[ X ]`, opcional.
- `NL` es fin de línea lógico.
- Los comentarios no generan `NL` dentro de cadenas ni árboles de tokens.
- Los identificadores y literales siguen la gramática de tokens de Rust 2024 salvo alias y raw identifiers documentados.

## 2. Archivo y modo

```ebnf
SourceFile       ::= [Shebang] [Header NL] {FileInnerAttribute | BlankOrComment | Item | NL} EOF ;
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
IndentedImplBody ::= ":" NL INDENT {Method | AssociatedItem | NL} DEDENT ;
IndentedTraitBody ::= ":" NL INDENT {TraitMethod | AssociatedItem | NL} DEDENT ;
IndentedModuleBody ::= ":" NL INDENT {StructuralItem | NL} DEDENT ;
TraitMethod      ::= FunctionHeader ";" | FunctionHeader FunctionBody ;
```

`RustFunctionHeader`, `RustItemHeader` y `RustUseTree` son secuencias de tokens Rust balanceadas hasta el delimitador estructural. En modo indentación, `impl`, `trait` y módulos admiten miembros estructurales y funciones con cuerpo indentado. Declaraciones de métodos de trait pueden terminar en `;`. Fuera de macros estándar que procesan expresiones y atributos allowlisted, el frontend normaliza tokens keyword que tengan alias español. Syn valida el fragmento en su contexto final.

En modo indentación, el `:` final solo abre un bloque después de un encabezado reconocido y al nivel de delimitadores cero. Los dos puntos de parámetros, tipos, paths y expresiones no abren bloques.

## 4. Instrucciones y bloques

```ebnf
Statement        ::= LetStatement | ReturnStatement | BreakStatement
                   | ContinueStatement | ControlStatement | ExprLine ;
LetStatement     ::= ("dejar" | "variable" | "let") ["mutable" | "mut"] RustPattern [":" RustType]
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

`RustExpr`, `RustPattern` y `RustType` son fragmentos balanceados Rust tras normalizar keywords fuera de regiones opacas. Syn valida la categoría esperada; `rustc` valida tipos, nombres y semántica. `coincidir` admite una expresión scrutinee seguida de brazos indentados `Patron => Expresion`; un brazo también puede abrir un bloque indentado omitiendo la expresión de la misma línea. Los patrones, guardas y expresiones conservan gramática Rust.

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
FileInnerAttribute ::= "#![" OpaqueTokenTree* "]" | InnerDocComment ;
```

Los comentarios de documentación externos e internos (`///`, `/** */`, `//!`, `/*! */`) se emiten como atributos Rust `doc`; su semántica se conserva, aunque Rust puede normalizar su representación. Los comentarios ordinarios se conservan en exportación e importación cuando están fuera de grupos opacos; su texto no se traduce. El layout circundante puede normalizarse. Un comentario ordinario dentro de argumentos opacos de macros o atributos genera un diagnóstico, no se descarta ni se reescribe. En macros estándar allowlisted que reciben expresiones se normalizan recursivamente aliases de Óxido; literales y cadenas siguen intactos. Los nombres y excepciones se detallan en [aliases](aliases.md). La sintaxis de invocación se basa en los árboles delimitados de Rust ([Rust Reference: Macro Invocation](https://doc.rust-lang.org/reference/macros.html#macro-invocation)).

## 7. Normalización antes de Syn

1. Tokenizar sin alterar literales/comentarios y reconocer grupos balanceados.
2. Reconocer keywords/prelude fuera de regiones opacas; normalizar nombres allowlisted de macros y solo los argumentos de macros estándar expresivas.
3. Normalizar bloques indentados a grupos `{}` conservando un mapa de origen.
4. Normalizar terminadores de instrucciones indentadas conforme a la sección 5.
5. Construir Rust tokens/AST y parsear el archivo con Syn (`full`, `parsing`).
6. Dejar validación semántica y de tipos a Cargo/rustc.

La gramática de expresiones y tipos es la de la edición Rust fijada, salvo las normalizaciones explícitas de esta especificación. No existe traducción textual global.
