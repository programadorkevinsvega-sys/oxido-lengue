use std::mem;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    PrintWorkingDirectory,
    ListDirectory(Option<String>),
    ChangeDirectory(String),
    ShowFile(String),
    Echo(String),
    Clear,
    Exit,
    Unknown(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError(&'static str);

impl ParseError {
    pub fn message(&self) -> &'static str {
        self.0
    }
}

impl Command {
    pub fn parse(input: &str) -> Result<Option<Self>, ParseError> {
        let arguments = tokenize(input)?;
        if arguments.is_empty() {
            return Ok(None);
        }

        let command = match arguments[0].as_str() {
            "help" => {
                require_argument_count(&arguments, 1)?;
                Self::Help
            }
            "pwd" => {
                require_argument_count(&arguments, 1)?;
                Self::PrintWorkingDirectory
            }
            "ls" => {
                if arguments.len() > 2 {
                    return Err(ParseError("Uso: ls [ruta]"));
                }
                Self::ListDirectory(arguments.get(1).cloned())
            }
            "cd" => {
                require_argument_count(&arguments, 2)?;
                Self::ChangeDirectory(arguments[1].clone())
            }
            "cat" => {
                require_argument_count(&arguments, 2)?;
                Self::ShowFile(arguments[1].clone())
            }
            "echo" => Self::Echo(arguments[1..].join(" ")),
            "clear" => {
                require_argument_count(&arguments, 1)?;
                Self::Clear
            }
            "exit" | "quit" => {
                require_argument_count(&arguments, 1)?;
                Self::Exit
            }
            name => Self::Unknown(name.to_owned()),
        };

        Ok(Some(command))
    }
}

fn require_argument_count(arguments: &[String], expected: usize) -> Result<(), ParseError> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(ParseError("Cantidad de argumentos incorrecta"))
    }
}

fn tokenize(input: &str) -> Result<Vec<String>, ParseError> {
    let mut arguments = Vec::new();
    let mut argument = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut started = false;

    for character in input.chars() {
        if escaped {
            argument.push(character);
            escaped = false;
            started = true;
            continue;
        }

        if character == '\\' {
            escaped = true;
            started = true;
            continue;
        }

        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            } else {
                argument.push(character);
            }
            continue;
        }

        match character {
            '\'' | '"' => {
                quote = Some(character);
                started = true;
            }
            character if character.is_whitespace() => {
                if started {
                    arguments.push(mem::take(&mut argument));
                    started = false;
                }
            }
            _ => {
                argument.push(character);
                started = true;
            }
        }
    }

    if escaped {
        return Err(ParseError("La línea termina con un escape incompleto"));
    }
    if quote.is_some() {
        return Err(ParseError("Falta cerrar una comilla"));
    }
    if started {
        arguments.push(argument);
    }

    Ok(arguments)
}

#[cfg(test)]
mod tests {
    use super::{Command, ParseError};

    #[test]
    fn parses_quoted_arguments_and_escapes() {
        let command =
            Command::parse(r#"cat "archivo de prueba.txt""#).expect("la línea debe ser válida");

        assert_eq!(
            command,
            Some(Command::ShowFile("archivo de prueba.txt".to_owned()))
        );
        assert_eq!(
            Command::parse(r#"echo ruta\ con\ espacios"#).expect("la línea debe ser válida"),
            Some(Command::Echo("ruta con espacios".to_owned()))
        );
    }

    #[test]
    fn rejects_unclosed_quotes_and_trailing_escape() {
        assert_eq!(
            Command::parse("echo 'texto"),
            Err(ParseError("Falta cerrar una comilla"))
        );
        assert_eq!(
            Command::parse("echo texto\\"),
            Err(ParseError("La línea termina con un escape incompleto"))
        );
    }

    #[test]
    fn recognizes_empty_input_and_unknown_commands() {
        assert_eq!(Command::parse("  ").expect("vacío es válido"), None);
        assert_eq!(
            Command::parse("date").expect("comando es sintácticamente válido"),
            Some(Command::Unknown("date".to_owned()))
        );
    }
}
