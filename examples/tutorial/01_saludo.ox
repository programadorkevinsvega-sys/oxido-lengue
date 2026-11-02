sintaxis indentacion

// Lección 1: variables, funciones y salida por consola.
funcion saludar(nombre: &texto) -> Cadena:
    formato!("Hola, {}!", nombre)

funcion principal():
    dejar nombre = "mundo"
    imprimir!("{}", saludar(nombre))
