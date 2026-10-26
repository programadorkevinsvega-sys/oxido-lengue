# Extensión de VS Code — estado

No existe todavía una extensión funcional. El crate `oxido-lsp` es un prototipo stdio con diagnósticos léxicos iniciales y semantic tokens experimentales; no se integra con `rust-analyzer`, no inicia Cargo y no implementa Workspace Trust. Una extensión futura deberá respetar Workspace Trust antes de invocar compilación o análisis que ejecute código.
