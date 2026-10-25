sintaxis llaves
use crate::dominio::modelos::Alumno;

pub funcion esta_aprobado(alumno: &Alumno) -> bool {
    alumno.nota >= 6.0
}
