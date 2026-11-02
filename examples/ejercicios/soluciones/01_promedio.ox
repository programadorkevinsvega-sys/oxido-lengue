sintaxis indentacion

// Ejercicio 1 (solución): promedio de un array.
funcion promedio(valores: &[i32]) -> f64:
    dejar mut suma = 0
    para valor en valores:
        suma += *valor
    suma como f64 / valores.largo() como f64

funcion principal():
    dejar valores = [8, 6, 10]
    imprimir!("{:.2}", promedio(&valores))
