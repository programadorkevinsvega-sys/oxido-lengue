sintaxis indentacion
usar axum::{Router, Json, extract::{State, Path}, http::StatusCode, routing::get}
usar serde::{Serialize, Deserialize}
usar std::sync::Arc
usar tokio::sync::Mutex

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

funcion datos_validos(datos: &NuevoProducto) -> bool:
    !datos.nombre.trim().is_empty() && datos.precio.is_finite() && datos.precio >= 0.0

asincrono funcion listar(State(estado): State<Estado>) -> Json<Vector<Producto>>:
    dejar almacen = estado.lock().await
    Json(almacen.productos.clone())

asincrono funcion crear(
    State(estado): State<Estado>,
    Json(datos): Json<NuevoProducto>,
) -> Resultado<(StatusCode, Json<Producto>), StatusCode>:
    si !datos_validos(&datos):
        retornar Err(StatusCode::BAD_REQUEST)
    dejar mutable almacen = estado.lock().await
    dejar siguiente_id = almacen.siguiente_id.checked_add(1)
    si siguiente_id.is_none():
        retornar Err(StatusCode::CONFLICT)
    dejar producto = Producto::desde_datos(almacen.siguiente_id, datos)
    almacen.siguiente_id = siguiente_id.unwrap()
    almacen.productos.agregar(producto.clone())
    Ok((StatusCode::CREATED, Json(producto)))

asincrono funcion obtener(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
) -> Resultado<Json<Producto>, StatusCode>:
    dejar almacen = estado.lock().await
    coincidir almacen.productos.iter().find(|producto| producto.id == id):
        Some(producto) => Ok(Json(producto.clone()))
        None => Err(StatusCode::NOT_FOUND)

asincrono funcion actualizar(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
    Json(datos): Json<NuevoProducto>,
) -> Resultado<Json<Producto>, StatusCode>:
    si !datos_validos(&datos):
        retornar Err(StatusCode::BAD_REQUEST)
    dejar mutable almacen = estado.lock().await
    coincidir almacen.productos.iter_mut().find(|producto| producto.id == id):
        Some(producto) => Ok(Json(producto.aplicar(datos)))
        None => Err(StatusCode::NOT_FOUND)

asincrono funcion eliminar(
    State(estado): State<Estado>,
    Path(id): Path<u32>,
) -> StatusCode:
    dejar mutable almacen = estado.lock().await
    dejar antes = almacen.productos.largo()
    almacen.productos.retain(|producto| producto.id != id)
    si almacen.productos.largo() == antes:
        StatusCode::NOT_FOUND
    sino:
        StatusCode::NO_CONTENT

asincrono funcion identidad_de_prueba() -> Json<u32>:
    Json(std::process::id())

#[tokio::main]
asincrono funcion main():
    dejar direccion = std::env::var("CRUD_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38211".to_string())
    dejar id_inicial = std::env::var("CRUD_ID_INICIAL")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<u32>()
        .expect("CRUD_ID_INICIAL debe ser un entero u32")
    dejar estado: Estado = Arc::new(Mutex::new(Almacen {
        productos: Vector::nuevo(),
        siguiente_id: id_inicial,
    }))
    dejar app = Router::new()
        .route("/__test/instance", get(identidad_de_prueba))
        .route("/productos", get(listar).post(crear))
        .route("/productos/{id}", get(obtener).put(actualizar).delete(eliminar))
        .with_state(estado)
    dejar escucha = tokio::net::TcpListener::bind(direccion).await.unwrap()
    axum::serve(escucha, app).await.unwrap()
