use std::io;
use std::path::Path;

use crate::domain::command::Command;

use super::TerminalIo;

const HELP: &str = "\
Comandos disponibles:
  help             Muestra esta ayuda
  pwd              Muestra el directorio actual
  ls [ruta]        Lista el contenido de un directorio
  cd <ruta>        Cambia el directorio actual
  cat <archivo>    Muestra el contenido de un archivo de texto
  echo <texto>     Muestra texto
  clear            Limpia la pantalla
  exit             Cierra la terminal
No se ejecutan comandos externos.
";

pub struct Terminal<I> {
    io: I,
}

impl<I: TerminalIo> Terminal<I> {
    pub fn new(io: I) -> Self {
        Self { io }
    }

    pub fn run(&mut self) -> io::Result<()> {
        loop {
            let prompt = format!("{}> ", self.io.current_directory()?.display());
            let Some(input) = self.io.read_line(&prompt)? else {
                return Ok(());
            };

            let command = match Command::parse(&input) {
                Ok(Some(command)) => command,
                Ok(None) => continue,
                Err(error) => {
                    self.io
                        .write_error(&format!("Error: {}\n", error.message()))?;
                    continue;
                }
            };

            match self.execute(command) {
                Ok(true) => {}
                Ok(false) => return Ok(()),
                Err(error) => self.io.write_error(&format!("Error: {error}\n"))?,
            }
        }
    }

    fn execute(&mut self, command: Command) -> io::Result<bool> {
        match command {
            Command::Help => self.io.write_output(HELP)?,
            Command::PrintWorkingDirectory => {
                let directory = self.io.current_directory()?;
                self.io
                    .write_output(&format!("{}\n", directory.display()))?;
            }
            Command::ListDirectory(path) => {
                let path = path.as_deref().map_or(Path::new("."), Path::new);
                for entry in self.io.list_directory(path)? {
                    if entry.is_directory {
                        self.io.write_output(&format!("{}/\n", entry.name))?;
                    } else {
                        self.io.write_output(&format!("{}\n", entry.name))?;
                    }
                }
            }
            Command::ChangeDirectory(path) => {
                self.io.change_directory(Path::new(&path))?;
            }
            Command::ShowFile(path) => {
                let content = self.io.read_file(Path::new(&path))?;
                self.io.write_output(&content)?;
            }
            Command::Echo(text) => {
                self.io.write_output(&format!("{text}\n"))?;
            }
            Command::Clear => self.io.clear_screen()?,
            Command::Exit => return Ok(false),
            Command::Unknown(command) => {
                self.io.write_error(&format!(
                    "Comando desconocido: {command}. Escribe 'help' para ver la ayuda.\n"
                ))?;
            }
        }

        Ok(true)
    }
}
