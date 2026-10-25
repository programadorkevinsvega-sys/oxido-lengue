usar std::fs;
usar std::io::{self, Write};
usar std::path::{Path, PathBuf};

usar crate::biblioteca::application::{DirectoryEntry, TerminalIo};

publico estructura Console;

implementar TerminalIo para Console:
    funcion read_line(&mutable mismo, prompt: &texto) -> io::Resultado<Opcion<Cadena>>:
        imprimir_sin_salto!("{prompt}")
        std::io::stdout().flush()?

        dejar mutable input = Cadena::new()
        si std::io::stdin().read_line(&mutable input)? == 0:
            retornar Bien(Ninguno)

        Bien(Alguno(input.trim_end().to_owned()))

    funcion write_output(&mutable mismo, text: &texto) -> io::Resultado<()>:
        imprimir_sin_salto!("{text}")
        std::io::stdout().flush()

    funcion write_error(&mutable mismo, text: &texto) -> io::Resultado<()>:
        imprimir_error_sin_salto!("{text}")
        std::io::stderr().flush()

    funcion current_directory(&mismo) -> io::Resultado<PathBuf>:
        std::env::current_dir()

    funcion list_directory(&mismo, path: &Path) -> io::Resultado<Vector<DirectoryEntry>>:
        dejar mutable entries = fs::read_dir(path)?.mapear(|entry| {
            dejar entry = entry?;
            Bien(DirectoryEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_directory: entry.file_type()?.is_dir(),
            })
        }).recolectar::<io::Resultado<Vector<_>>>()?

        entries.sort_by(|left, right| left.name.cmp(&right.name))
        Bien(entries)

    funcion change_directory(&mutable mismo, path: &Path) -> io::Resultado<()>:
        std::env::set_current_dir(path)

    funcion read_file(&mismo, path: &Path) -> io::Resultado<Cadena>:
        fs::read_to_string(path)

    funcion clear_screen(&mutable mismo) -> io::Resultado<()>:
        imprimir_sin_salto!("\x1B[2J\x1B[H")
        std::io::stdout().flush()
