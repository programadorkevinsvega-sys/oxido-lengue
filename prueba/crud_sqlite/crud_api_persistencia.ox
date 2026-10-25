sintaxis indentacion
usar axum::{Router, Json, extract::{State, Path}, http::StatusCode, routing::get}
usar serde::{Serialize, Deserialize}
usar sqlx::{sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions}, Row, SqlitePool}
usar std::sync::OnceLock
usar std::time::{Duration, SystemTime, UNIX_EPOCH}

#[derive(Clone, Serialize)]
estructura Producto:
    id: i64
    nombre: Cadena
    precio: f64

#[derive(Deserialize)]
estructura NuevoProducto:
    nombre: Cadena
    precio: f64

tipo Estado = SqlitePool;

#[derive(Clone, Debug, Serialize)]
estructura IdentidadServidor:
    pid: u32
    instancia: Cadena

static IDENTIDAD_SERVIDOR: OnceLock<IdentidadServidor> = OnceLock::new();

asincrono funcion consultar_instancia() -> Json<IdentidadServidor>:
    Json(IDENTIDAD_SERVIDOR.get().unwrap().clone())

funcion datos_validos(datos: &NuevoProducto) -> bool:
    !datos.nombre.trim().is_empty() && datos.precio.is_finite() && datos.precio >= 0.0

funcion producto_de_fila(fila: sqlx::sqlite::SqliteRow) -> Producto:
    Producto {
        id: fila.get("id"),
        nombre: fila.get("nombre"),
        precio: fila.get("precio"),
    }

funcion estado_error_base_datos(error: &sqlx::Error) -> StatusCode:
    coincidir error:
        sqlx::Error::Database(error_db) =>
            si error_db.is_unique_violation():
                StatusCode::CONFLICT
            sino:
                StatusCode::INTERNAL_SERVER_ERROR
        _ => StatusCode::INTERNAL_SERVER_ERROR

asincrono funcion listar(State(pool): State<Estado>) -> Resultado<Json<Vector<Producto>>, StatusCode>:
    dejar filas = sqlx::query("SELECT id, nombre, precio FROM productos ORDER BY id")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    dejar productos = filas.into_iter().map(producto_de_fila).collect()
    Ok(Json(productos))

asincrono funcion crear(
    State(pool): State<Estado>,
    Json(datos): Json<NuevoProducto>,
) -> Resultado<(StatusCode, Json<Producto>), StatusCode>:
    si !datos_validos(&datos):
        retornar Err(StatusCode::BAD_REQUEST)
    dejar mutable transaccion = pool.begin().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    dejar resultado = sqlx::query("INSERT INTO productos (nombre, precio) VALUES (?, ?)")
        .bind(&datos.nombre)
        .bind(datos.precio)
        .execute(&mut *transaccion)
        .await
    coincidir resultado:
        Ok(resultado) =>
            dejar producto = Producto {
                id: resultado.last_insert_rowid(),
                nombre: datos.nombre,
                precio: datos.precio,
            }
            si transaccion.commit().await.is_err():
                retornar Err(StatusCode::INTERNAL_SERVER_ERROR)
            Ok((StatusCode::CREATED, Json(producto)))
        Err(error) =>
            dejar codigo = estado_error_base_datos(&error)
            si transaccion.rollback().await.is_err():
                retornar Err(StatusCode::INTERNAL_SERVER_ERROR)
            Err(codigo)

asincrono funcion obtener(
    State(pool): State<Estado>,
    Path(id): Path<i64>,
) -> Resultado<Json<Producto>, StatusCode>:
    dejar fila = sqlx::query("SELECT id, nombre, precio FROM productos WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    coincidir fila:
        Some(fila) => Ok(Json(producto_de_fila(fila)))
        None => Err(StatusCode::NOT_FOUND)

asincrono funcion actualizar(
    State(pool): State<Estado>,
    Path(id): Path<i64>,
    Json(datos): Json<NuevoProducto>,
) -> Resultado<Json<Producto>, StatusCode>:
    si !datos_validos(&datos):
        retornar Err(StatusCode::BAD_REQUEST)
    dejar resultado = sqlx::query("UPDATE productos SET nombre = ?, precio = ? WHERE id = ?")
        .bind(&datos.nombre)
        .bind(datos.precio)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    si resultado.rows_affected() == 0:
        retornar Err(StatusCode::NOT_FOUND)
    Ok(Json(Producto { id, nombre: datos.nombre, precio: datos.precio }))

asincrono funcion eliminar(
    State(pool): State<Estado>,
    Path(id): Path<i64>,
) -> Resultado<StatusCode, StatusCode>:
    dejar resultado = sqlx::query("DELETE FROM productos WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    si resultado.rows_affected() == 0:
        retornar Err(StatusCode::NOT_FOUND)
    Ok(StatusCode::NO_CONTENT)

#[tokio::main]
asincrono funcion principal():
    dejar ruta_db = std::env::var("CRUD_SQLITE_PATH")
        .unwrap_or_else(|_| "./crud_persistencia.sqlite".to_string())
    dejar opciones = SqliteConnectOptions::new()
        .filename(&ruta_db)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(10))
    dejar pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opciones)
        .await
        .expect("no se pudo abrir la base SQLite")
    dejar inicio = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    IDENTIDAD_SERVIDOR.set(IdentidadServidor {
        pid: std::process::id(),
        instancia: formato!("{}-{}", std::process::id(), inicio),
    }).unwrap()
    sqlx::query("CREATE TABLE IF NOT EXISTS productos (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL UNIQUE CHECK(length(trim(nombre)) > 0), precio REAL NOT NULL CHECK(precio >= 0))")
        .execute(&pool)
        .await
        .expect("no se pudo crear la tabla productos")
    si std::env::var("CRUD_RESET_DB").unwrap_or_default() == "1":
        sqlx::query("DELETE FROM productos")
            .execute(&pool)
            .await
        .expect("no se pudo vaciar productos para el benchmark");
        sqlx::query("DELETE FROM sqlite_sequence WHERE name = 'productos'")
            .execute(&pool)
            .await
        .expect("no se pudo reiniciar la secuencia de IDs");
    dejar app = Router::new()
        .route("/__test/instance", get(consultar_instancia))
        .route("/productos", get(listar).post(crear))
        .route("/productos/{id}", get(obtener).put(actualizar).delete(eliminar))
        .with_state(pool)
    dejar direccion = std::env::var("CRUD_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38212".to_string())
    dejar escucha = tokio::net::TcpListener::bind(direccion).await.unwrap()
    axum::serve(escucha, app).await.unwrap()
