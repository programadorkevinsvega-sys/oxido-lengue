usar std::io;
usar std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
publico estructura DirectoryEntry:
    publico name: Cadena
    publico is_directory: booleano

publico rasgo TerminalIo:
    funcion read_line(&mutable mismo, prompt: &texto) -> io::Resultado<Opcion<Cadena>>;
    funcion write_output(&mutable mismo, text: &texto) -> io::Resultado<()>;
    funcion write_error(&mutable mismo, text: &texto) -> io::Resultado<()>;
    funcion current_directory(&mismo) -> io::Resultado<PathBuf>;
    funcion list_directory(&mismo, path: &Path) -> io::Resultado<Vector<DirectoryEntry>>;
    funcion change_directory(&mutable mismo, path: &Path) -> io::Resultado<()>;
    funcion read_file(&mismo, path: &Path) -> io::Resultado<Cadena>;
    funcion clear_screen(&mutable mismo) -> io::Resultado<()>;
