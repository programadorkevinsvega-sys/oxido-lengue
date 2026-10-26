# Extensión de VS Code — estado

No existe todavía una extensión funcional y la integración de VS Code de la Fase 11 no se ha iniciado. El crate `oxido-lsp` es un prototipo stdio: prueba sincronización de documentos completa/incremental, diagnósticos léxicos, semantic tokens heurísticos, framing acotado y cancelación cooperativa de semantic tokens. No se integra con `rust-analyzer`, no inicia Cargo ni mapea sus diagnósticos, y no implementa Workspace Trust. Una futura extensión deberá respetar Workspace Trust antes de invocar compilación o análisis que ejecute código.
