pub struct Alumno {
    pub nombre: String,
    pub nota: f64,
}
impl Alumno {
    pub fn nuevo(nombre: String, nota: f64) -> Self {
        Self { nombre, nota }
    }
}
