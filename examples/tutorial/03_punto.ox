sintaxis indentacion

// Lección 3: estructuras, métodos y campos.
estructura Punto:
    x: i32
    y: i32

implementar Punto:
    funcion nuevo(x: i32, y: i32) -> Mismo:
        Mismo { x, y }
    funcion suma(&mismo) -> i32:
        mismo.x + mismo.y

funcion principal():
    dejar punto = Punto::nuevo(2, 3)
    imprimir!("({}, {}) suma {}", punto.x, punto.y, punto.suma())
