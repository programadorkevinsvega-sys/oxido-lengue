sintaxis indentacion

estructura Alumno:
    nombre: Cadena
    nota: f64

implementar Alumno:
    funcion aprobado(&mismo) -> bool:
        mismo.nota >= 6.0

funcion promedio(alumnos: &Vector<Alumno>) -> f64:
    dejar mutable total = 0.0
    para alumno en alumnos.iter():
        total += alumno.nota
    total / alumnos.largo() como f64

funcion principal():
    dejar mutable alumnos = Vector::nuevo()
    alumnos.agregar(Alumno { nombre: "Ana".a_cadena(), nota: 8.5 })
    alumnos.agregar(Alumno { nombre: "Luis".a_cadena(), nota: 5.0 })

    para alumno en alumnos.iter():
        si alumno.aprobado():
            imprimir!("{} aprobó", alumno.nombre)
        sino:
            imprimir!("{} reprobó", alumno.nombre)

    imprimir!("Promedio: {}", promedio(&alumnos))
