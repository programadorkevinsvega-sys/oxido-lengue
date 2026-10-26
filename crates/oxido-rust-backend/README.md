# oxido-rust-backend

Backend que emite Rust desde el AST con `quote`/`proc_macro2`, valida con Syn, usa prettyplease como formato intermedio y aplica `rustfmt` 1.99.1 (edición 2024) al Rust final. Las posiciones generadas se calculan sobre ese resultado formateado; los comentarios ordinarios se reinsertan y se conservan fuera de grupos opacos. Requiere el componente `rustfmt` del toolchain fijado por el workspace. Incluye la importación reversible restringida a funciones libres.
