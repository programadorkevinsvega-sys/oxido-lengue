sintaxis indentacion
usar axum::{Router, Json, extract::{State, Path}, http::StatusCode, routing::get}
usar serde::{Serialize, Deserialize}
usar std::sync::{Arc, Mutex}

#[derive(Clone, Serialize)]
estructura Producto:
    id: u32
    nombre: Cadena
    precio: f64

#[derive(Deserialize)]
estructura NuevoProducto:
    nombre: Cadena
    precio: f64

estructura Almacen:
    productos: Vector<Producto>
    siguiente_id: u32

tipo Estado = Arc<Mutex<Almacen>>;


implementar Producto:
    funcion desde_datos(id: u32, datos: NuevoProducto) -> Producto:
        Producto { id, nombre: datos.nombre, precio: datos.precio }

    funcion aplicar(&mutable mismo, datos: NuevoProducto) -> Producto:
        mismo.nombre = datos.nombre
        mismo.precio = datos.precio
        mismo.clone()

asincrono funcion listar(State(estado): State<Estado>) -> Json<Vector<Producto>>:
    dejar almacen = estado.lock().unwrap()
    Json(almacen.productos.clone())

asincrono funcion crear(
    State(estado): State<Estado>,
    Json(datos): Json<NuevoProducto>,
) -> Resultado<(StatusCode, Json<Producto>), StatusCode>:
    si datos.nombre.is_empty() || datos.precio < 0.0:
        retornar Err(StatusCode::BAD_REQUEST)
    dejar mutable almacen = estado.lock().unwrap()
    dejar producto = Producto::desde_datos(almacen.siguiente_id, datos)
    almacen.siguiente_id += 1
    almacen.productos.agregar(producto.clone())
    Ok((StatusCode::CREATED, Json(producto)))

asincrono funcion obtener(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
) -> Resultado<Json<Producto>, StatusCode>:
    dejar almacen = estado.lock().unwrap()
    coincidir almacen.productos.iter().find(|p| p.id == id):
        Some(p) => Ok(Json(p.clone()))
        None => Err(StatusCode::NOT_FOUND)

asincrono funcion actualizar(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
    Json(datos): Json<NuevoProducto>,
) -> Resultado<Json<Producto>, StatusCode>:
    dejar mutable almacen = estado.lock().unwrap()
    coincidir almacen.productos.iter_mut().find(|p| p.id == id):
        Some(p) => Ok(Json(p.aplicar(datos)))
        None => Err(StatusCode::NOT_FOUND)

asincrono funcion eliminar(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
) -> StatusCode:
    dejar mutable almacen = estado.lock().unwrap()
    dejar antes = almacen.productos.largo()
    almacen.productos.retain(|p| p.id != id)
    si almacen.productos.largo() == antes:
        StatusCode::NOT_FOUND
    sino:
        StatusCode::NO_CONTENT

#[tokio::main]
asincrono funcion main():
    dejar estado: Estado = Arc::new(Mutex::new(Almacen {
        productos: Vector::nuevo(),
        siguiente_id: 1,
    }))
    dejar app = Router::new()
        .route("/productos", get(listar).post(crear))
        .route("/productos/{id}", get(obtener).put(actualizar).delete(eliminar))
        .with_state(estado)
    dejar escucha = tokio::net::TcpListener::bind("127.0.0.1:3000").esperar.unwrap()
    axum::serve(escucha, app).esperar.unwrap()
