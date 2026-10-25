sintaxis llaves
pub estructura Alumno {
    pub nombre: Cadena,
    pub nota: f64,
}

implementar Alumno {
    pub funcion nuevo(nombre: Cadena, nota: f64) -> Self {
        Self { nombre, nota }
    }
}
