sintaxis indentacion

// Ejercicio 2 (solución): inventario con estructuras y métodos.
estructura Producto:
    nombre: Cadena
    cantidad: i32
    precio: i32

implementar Producto:
    funcion subtotal(&mismo) -> i32:
        mismo.cantidad * mismo.precio

funcion principal():
    dejar productos = vector![
        Producto { nombre: "te".a_cadena(), cantidad: 2, precio: 3 },
        Producto { nombre: "cafe".a_cadena(), cantidad: 1, precio: 7 },
    ]
    dejar mut total = 0
    para producto en productos.iter():
        imprimir!("{}: {}", producto.nombre, producto.subtotal())
        total += producto.subtotal()
    imprimir!("total: {}", total)
