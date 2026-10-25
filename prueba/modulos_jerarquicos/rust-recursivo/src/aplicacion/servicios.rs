use crate::dominio::modelos::Alumno;
pub fn esta_aprobado(alumno: &Alumno) -> bool {
    alumno.nota >= 6.0
}
