sintaxis indentacion
use axum::{Router, Json, extract::{State, Path}, http::StatusCode, routing::get}
use serde::{Serialize, Deserialize}
use std::sync::{Arc, Mutex}

#[derive(Clone, Serialize)]
struct Producto:
    id: u32
    nombre: String
    precio: f64

#[derive(Deserialize)]
struct NuevoProducto:
    nombre: String
    precio: f64

struct Almacen:
    productos: Vec<Producto>
    siguiente_id: u32

tipo Estado = Arc<Mutex<Almacen>>;


impl Producto:
    fn desde_datos(id: u32, datos: NuevoProducto) -> Producto:
        Producto { id, nombre: datos.nombre, precio: datos.precio }

    fn aplicar(&mut self, datos: NuevoProducto) -> Producto:
        self.nombre = datos.nombre
        self.precio = datos.precio
        self.clone()

async fn listar(State(estado): State<Estado>) -> Json<Vec<Producto>>:
    let almacen = estado.lock().unwrap()
    Json(almacen.productos.clone())

async fn crear(
    State(estado): State<Estado>,
    Json(datos): Json<NuevoProducto>,
) -> Result<(StatusCode, Json<Producto>), StatusCode>:
    if datos.nombre.is_empty() || datos.precio < 0.0:
        return Err(StatusCode::BAD_REQUEST)
    let mut almacen = estado.lock().unwrap()
    let producto = Producto::desde_datos(almacen.siguiente_id, datos)
    almacen.siguiente_id += 1
    almacen.productos.push(producto.clone())
    Ok((StatusCode::CREATED, Json(producto)))

async fn obtener(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
) -> Result<Json<Producto>, StatusCode>:
    let almacen = estado.lock().unwrap()
    match almacen.productos.iter().find(|p| p.id == id):
        Some(p) => Ok(Json(p.clone()))
        None => Err(StatusCode::NOT_FOUND)

async fn actualizar(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
    Json(datos): Json<NuevoProducto>,
) -> Result<Json<Producto>, StatusCode>:
    let mut almacen = estado.lock().unwrap()
    match almacen.productos.iter_mut().find(|p| p.id == id):
        Some(p) => Ok(Json(p.aplicar(datos)))
        None => Err(StatusCode::NOT_FOUND)

async fn eliminar(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
) -> StatusCode:
    let mut almacen = estado.lock().unwrap()
    let antes = almacen.productos.len()
    almacen.productos.retain(|p| p.id != id)
    if almacen.productos.len() == antes:
        StatusCode::NOT_FOUND
    else:
        StatusCode::NO_CONTENT

#[tokio::main]
async fn main():
    let estado: Estado = Arc::new(Mutex::new(Almacen {
        productos: Vec::new(),
        siguiente_id: 1,
    }))
    let app = Router::new()
        .route("/productos", get(listar).post(crear))
        .route("/productos/{id}", get(obtener).put(actualizar).delete(eliminar))
        .with_state(estado)
    let escucha = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap()
    axum::serve(escucha, app).await.unwrap()
