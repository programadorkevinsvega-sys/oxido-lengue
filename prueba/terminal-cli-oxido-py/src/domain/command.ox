usar std::mem;

#[derive(Debug, PartialEq, Eq)]
publico enumeracion Command:
    Help
    PrintWorkingDirectory
    ListDirectory(Opcion<Cadena>)
    ChangeDirectory(Cadena)
    ShowFile(Cadena)
    Echo(Cadena)
    Clear
    Exit
    Unknown(Cadena)

#[derive(Debug, PartialEq, Eq)]
publico estructura ParseError(&'static str);

implementar ParseError:
    publico funcion message(&mismo) -> &'static str:
        mismo.0

implementar Command:
    publico funcion parse(input: &texto) -> Resultado<Opcion<Mismo>, ParseError>:
        dejar arguments = tokenize(input)?
        si arguments.esta_vacio():
            retornar Bien(Ninguno)

        si arguments[0] == "help":
            require_argument_count(&arguments, 1)?
        si arguments[0] == "pwd":
            require_argument_count(&arguments, 1)?
        si arguments[0] == "clear":
            require_argument_count(&arguments, 1)?
        si arguments[0] == "exit":
            require_argument_count(&arguments, 1)?
        si arguments[0] == "quit":
            require_argument_count(&arguments, 1)?
        si arguments[0] == "cd":
            require_argument_count(&arguments, 2)?
        si arguments[0] == "cat":
            require_argument_count(&arguments, 2)?
        si arguments[0] == "ls":
            si arguments.len() > 2:
                retornar Falla(ParseError("Uso: ls [ruta]"))

        dejar command = coincidir arguments[0].as_str():
            "help" => Mismo::Help
            "pwd" => Mismo::PrintWorkingDirectory
            "ls" => Mismo::ListDirectory(arguments.get(1).cloned())
            "cd" => Mismo::ChangeDirectory(arguments[1].clone())
            "cat" => Mismo::ShowFile(arguments[1].clone())
            "echo" => Mismo::Echo(arguments[1..].join(" "))
            "clear" => Mismo::Clear
            "exit" => Mismo::Exit
            "quit" => Mismo::Exit
            name => Mismo::Unknown(name.to_owned())

        Bien(Alguno(command))

funcion require_argument_count(arguments: &[Cadena], expected: usize) -> Resultado<(), ParseError>:
    si arguments.len() == expected:
        Bien(())
    sino:
        Falla(ParseError("Cantidad de argumentos incorrecta"))

funcion tokenize(input: &texto) -> Resultado<Vector<Cadena>, ParseError>:
    dejar mutable arguments = Vector::nuevo()
    dejar mutable argument = Cadena::new()
    dejar mutable quote = Ninguno
    dejar mutable escaped = falso
    dejar mutable started = falso

    para character en input.chars():
        si escaped:
            argument.agregar(character)
            escaped = falso
            started = verdadero
            continuar

        si character == '\\':
            escaped = verdadero
            started = verdadero
            continuar

        si dejar Alguno(quote_character) = quote:
            si character == quote_character:
                quote = Ninguno
            sino:
                argument.agregar(character)
            continuar

        si character == '\'' || character == '"':
            quote = Alguno(character)
            started = verdadero
        sino si character.is_whitespace():
            si started:
                arguments.agregar(mem::take(&mut argument))
                started = falso
        sino:
            argument.agregar(character)
            started = verdadero

    si escaped:
        retornar Falla(ParseError("La línea termina con un escape incompleto"))
    si quote.es_alguno():
        retornar Falla(ParseError("Falta cerrar una comilla"))
    si started:
        arguments.agregar(argument)

    Bien(arguments)

#[cfg(test)]
modulo tests:
    usar super::{Command, ParseError};

    #[prueba]
    funcion parses_quoted_arguments_and_escapes():
        dejar command = Command::parse(r#"cat "archivo de prueba.txt""#).exigir("la línea debe ser válida")

        afirmar_igual!(
            command,
            Alguno(Command::ShowFile("archivo de prueba.txt".to_owned()))
        )
        afirmar_igual!(
            Command::parse(r#"echo ruta\ con\ espacios"#).exigir("la línea debe ser válida"),
            Alguno(Command::Echo("ruta con espacios".to_owned()))
        )

    #[prueba]
    funcion rejects_unclosed_quotes_and_trailing_escape():
        afirmar_igual!(
            Command::parse("echo 'texto"),
            Falla(ParseError("Falta cerrar una comilla"))
        )
        afirmar_igual!(
            Command::parse("echo texto\\"),
            Falla(ParseError("La línea termina con un escape incompleto"))
        )

    #[prueba]
    funcion recognizes_empty_input_and_unknown_commands():
        afirmar_igual!(Command::parse("  ").exigir("vacío es válido"), Ninguno)
        afirmar_igual!(
            Command::parse("date").exigir("comando es sintácticamente válido"),
            Alguno(Command::Unknown("date".to_owned()))
        )
