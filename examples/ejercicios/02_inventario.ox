sintaxis indentacion

// Ejercicio 2 (inicio): completa el inventario.
// Criterio: imprimir cada producto y un total de 13.
estructura Producto:
    nombre: Cadena
    cantidad: i32
    precio: i32

implementar Producto:
    funcion subtotal(&mismo) -> i32:
        0

funcion principal():
    dejar productos = vector![
        Producto { nombre: "te".a_cadena(), cantidad: 2, precio: 3 },
        Producto { nombre: "cafe".a_cadena(), cantidad: 1, precio: 7 },
    ]
    // TODO: recorre los productos, imprime "nombre: subtotal" y suma el total.
    imprimir!("total: {}", 0)
