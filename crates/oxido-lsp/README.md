# oxido-lsp

Servidor LSP mínimo sobre stdio. Responde a `initialize`, `shutdown` y semantic tokens sin iniciar Cargo ni ejecutar código al abrir un documento. La integración de compilación requiere Workspace Trust del cliente.

```sh
cargo run --locked -p oxido-lsp
```
