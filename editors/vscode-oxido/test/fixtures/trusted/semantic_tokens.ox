sintaxis indentacion

modulo util:
    publico funcion saludo():
        imprimir!("hola")

estructura entero:
    valor: i32

funcion identidad(valor: entero):
    dejar copia = identidad
    dejar numero = 1
    retornar valor

funcion principal():
    util::saludo()
    identidad(1)
