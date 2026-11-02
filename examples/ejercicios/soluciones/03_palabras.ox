sintaxis indentacion

// Ejercicio 3 (solución): iteradores y closures.
funcion contar_largas(palabras: &[&str], minimo: usize) -> usize:
    palabras
        .iter()
        .filter(|palabra| palabra.largo() >= minimo)
        .count()

funcion principal():
    dejar palabras = ["oxido", "rust", "metal", "aprender"]
    imprimir!("{}", contar_largas(&palabras, 6))
