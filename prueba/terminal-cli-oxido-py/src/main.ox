modulo biblioteca;

usar biblioteca::application::Terminal;
usar biblioteca::infrastructure::Console;

funcion principal() -> Resultado<(), std::io::Error>:
    Terminal::new(Console).run()
