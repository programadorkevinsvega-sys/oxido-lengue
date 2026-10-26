#![forbid(unsafe_code)]

use oxido_lsp::serve_stdio;
use std::io;

fn main() -> io::Result<()> {
    serve_stdio(io::stdin(), io::stdout().lock())
}
