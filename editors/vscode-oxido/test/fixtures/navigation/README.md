# Fixture de navegación entre módulos

`main.ox` importa dos funciones llamadas `repetida` desde `alpha` y `beta`, y
usa `dominio::modelos::Alumno::nuevo` definido en `dominio/modelos.ox`.

Las pruebas del LSP comprueban que el cualificador `alpha::`/`beta::` y la ruta
`dominio::modelos` seleccionan la declaración correcta en lugar de devolver la
primera coincidencia por nombre.
