# oxido-cargo

Adapter de procesos Cargo: invoca `Command` con argv separado, directorio explícito y `CARGO_HOME` aislado; separa eventos `--message-format=json` de stdout del programa y permite consumirlos en streaming. Las acciones `check`, `build` y `test` de workspaces importados no tienen timeout fijo; los comandos de paquete, `run`, LSP y descarga conservan el timeout de 120 segundos. Ctrl+C cancela y termina el árbol de procesos Cargo.
