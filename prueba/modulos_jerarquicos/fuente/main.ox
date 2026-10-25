sintaxis llaves
modulo aplicacion;
modulo dominio;

use crate::aplicacion::servicios::esta_aprobado;
use crate::dominio::modelos::Alumno;

funcion principal() {
    let alumno = Alumno::nuevo("Ana".to_string(), 8.5);
    println!("{} aprobó: {}", alumno.nombre, esta_aprobado(&alumno));
}
