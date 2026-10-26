# Crates

El workspace contiene seis crates funcionales: `oxido-language`, `oxido-rust-backend`, `oxido-compiler`, `oxido-cargo`, `oxido-cli` y `oxido-lsp`. Sus responsabilidades están resumidas en [la arquitectura](../docs/architecture.md). El LSP actual ofrece el transporte stdio básico y semantic tokens; la integración de compilación con Workspace Trust sigue siendo responsabilidad del cliente.
