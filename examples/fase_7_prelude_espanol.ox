sintaxis indentacion

funcion medir(valor: &texto) -> usize:
    valor.largo()

funcion incrementar(mutable valor: usize) -> usize:
    valor += 1
    valor

funcion actualizar(valor: &mutable i32):
    *valor = 4

funcion principal():
    variable mutable nombres: Vector<Cadena> = vector![String::from("Ada")]
    nombres.agregar(String::from("Lovelace"))
    variable cantidad = incrementar(medir(&nombres[0]))
    variable mutable numero = 1
    actualizar(&mutable numero)
    variable nombre = nombres[0].a_cadena()
    imprimirln!("{}:{}", nombre, cantidad)
