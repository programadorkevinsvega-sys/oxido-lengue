sintaxis indentacion
estructura Punto:
    x: i32
    y: i32

enumeracion Estado:
    Listo(i32)

usar std::collections::VecDeque

funcion main():
    dejar punto = Punto { x: 2, y: 3 }
    dejar estado = Estado::Listo(punto.x + punto.y)
    dejar valor = coincidir estado { Estado::Listo(numero) => numero, }
    dejar mut cola = VecDeque::from([1, 2, 3])
    cola.push_back(4)
    println!("{}:{}", valor, cola.pop_front().unwrap());
