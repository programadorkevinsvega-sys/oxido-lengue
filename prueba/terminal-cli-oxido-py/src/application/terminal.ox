usar std::io;
usar std::path::Path;

usar crate::biblioteca::domain::command::Command;

usar super::TerminalIo;

const HELP: &'static str = r#"Comandos disponibles:
  help             Muestra esta ayuda
  pwd              Muestra el directorio actual
  ls [ruta]        Lista el contenido de un directorio
  cd <ruta>        Cambia el directorio actual
  cat <archivo>    Muestra el contenido de un archivo de texto
  echo <texto>     Muestra texto
  clear            Limpia la pantalla
  exit             Cierra la terminal
No se ejecutan comandos externos.
"#;

publico estructura Terminal<I>:
    io: I

implementar<I: TerminalIo> Terminal<I>:
    publico funcion new(io: I) -> Mismo:
        Mismo { io }

    publico funcion run(&mutable mismo) -> io::Resultado<()>:
        bucle:
            dejar prompt = formato!("{}> ", mismo.io.current_directory()?.display())
            dejar input = coincidir mismo.io.read_line(&prompt)?:
                Alguno(input) => input
                Ninguno => retornar Bien(())

            dejar command = coincidir Command::parse(&input):
                Bien(Alguno(command)) => command
                Bien(Ninguno) => continuar
                Falla(error) =>
                    mismo.io.write_error(&formato!("Error: {}\n", error.message()))?
                    continuar

            coincidir mismo.execute(command):
                Bien(verdadero) => continuar
                Bien(falso) => retornar Bien(())
                Falla(error) => mismo.io.write_error(&formato!("Error: {error}\n"))?

    funcion execute(&mutable mismo, command: Command) -> io::Resultado<booleano>:
        coincidir command:
            Command::Help => mismo.io.write_output(HELP)?
            Command::PrintWorkingDirectory =>
                dejar directory = mismo.io.current_directory()?
                mismo.io.write_output(&formato!("{}\n", directory.display()))?
            Command::ListDirectory(path) =>
                dejar path = path.as_deref().map_or(Path::new("."), Path::new)
                para entry en mismo.io.list_directory(path)?:
                    si entry.is_directory:
                        mismo.io.write_output(&formato!("{}/\n", entry.name))?
                    sino:
                        mismo.io.write_output(&formato!("{}\n", entry.name))?
            Command::ChangeDirectory(path) =>
                mismo.io.change_directory(Path::new(&path))?
            Command::ShowFile(path) =>
                dejar content = mismo.io.read_file(Path::new(&path))?
                mismo.io.write_output(&content)?
            Command::Echo(text) =>
                mismo.io.write_output(&formato!("{text}\n"))?
            Command::Clear => mismo.io.clear_screen()?
            Command::Exit => retornar Bien(falso)
            Command::Unknown(command) =>
                mismo.io.write_error(&formato!("Comando desconocido: {command}. Escribe 'help' para ver la ayuda.\n"))?

        Bien(verdadero)
