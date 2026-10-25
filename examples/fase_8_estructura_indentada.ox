sintaxis indentacion

estructura Contador:
    valor: i32

implementar Contador:
    funcion nuevo(valor: i32) -> Mismo:
        Mismo { valor }
    funcion leer(&mismo) -> i32:
        mismo.valor

rasgo Describible:
    funcion describir(&mismo) -> Cadena;
    funcion categoria() -> Cadena:
        "contador".to_string()

implementar Describible para Contador:
    funcion describir(&mismo) -> Cadena:
        dejar valor = mismo.valor
        formato!("valor {}", valor)

modulo utilidades:
    publico modulo operaciones:
        publico funcion sumar(a: i32, b: i32) -> i32:
            a + b

funcion principal():
    dejar contador = Contador::nuevo(utilidades::operaciones::sumar(2, 2))
    dejar etiqueta = coincidir contador.leer():
        4 => "cuatro"
        _ => "otro"
    coincidir contador.leer():
        4 =>
            dejar detalle = contador.describir()
            imprimir!("{}: {}", detalle, etiqueta)
        _ =>
            imprimir!("{}: {}", contador.describir(), etiqueta)
