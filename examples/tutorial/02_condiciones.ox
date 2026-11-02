sintaxis indentacion

// Lección 2: condiciones, bucles y coincidencia de patrones.
funcion clasificar(valor: i32) -> &'static str:
    si valor >= 10:
        "alto"
    sino:
        "bajo"

funcion principal():
    dejar valores = [3, 12, 5]
    para valor en valores:
        dejar etiqueta = coincidir valor:
            12 => "doce"
            _ => "otro"
        imprimir!("{}: {} ({})", valor, clasificar(valor), etiqueta)
