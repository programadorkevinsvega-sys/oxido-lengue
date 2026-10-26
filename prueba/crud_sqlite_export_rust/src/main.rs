use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[derive(Clone, Serialize)]
struct Producto {
    id: i64,
    nombre: String,
    precio: f64,
}
#[derive(Deserialize)]
struct NuevoProducto {
    nombre: String,
    precio: f64,
}
type Estado = SqlitePool;
#[derive(Clone, Debug, Serialize)]
struct IdentidadServidor {
    pid: u32,
    instancia: String,
}
static IDENTIDAD_SERVIDOR: OnceLock<IdentidadServidor> = OnceLock::new();
async fn consultar_instancia() -> Json<IdentidadServidor> {
    Json(IDENTIDAD_SERVIDOR.get().unwrap().clone())
}
fn datos_validos(datos: &NuevoProducto) -> bool {
    !datos.nombre.trim().is_empty() && datos.precio.is_finite() && datos.precio >= 0.0
}
fn producto_de_fila(fila: sqlx::sqlite::SqliteRow) -> Producto {
    Producto {
        id: fila.get("id"),
        nombre: fila.get("nombre"),
        precio: fila.get("precio"),
    }
}
fn estado_error_base_datos(error: &sqlx::Error) -> StatusCode {
    match error {
        sqlx::Error::Database(error_db) => {
            if error_db.is_unique_violation() {
                StatusCode::CONFLICT
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
async fn listar(State(pool): State<Estado>) -> Result<Json<Vec<Producto>>, StatusCode> {
    let filas = sqlx::query("SELECT id, nombre, precio FROM productos ORDER BY id")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let productos = filas.into_iter().map(producto_de_fila).collect();
    Ok(Json(productos))
}
async fn crear(
    State(pool): State<Estado>,
    Json(datos): Json<NuevoProducto>,
) -> Result<(StatusCode, Json<Producto>), StatusCode> {
    if !datos_validos(&datos) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut transaccion = pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let resultado = sqlx::query("INSERT INTO productos (nombre, precio) VALUES (?, ?)")
        .bind(&datos.nombre)
        .bind(datos.precio)
        .execute(&mut *transaccion)
        .await;
    match resultado {
        Ok(resultado) => {
            let producto = Producto {
                id: resultado.last_insert_rowid(),
                nombre: datos.nombre,
                precio: datos.precio,
            };
            if transaccion.commit().await.is_err() {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Ok((StatusCode::CREATED, Json(producto)))
        }
        Err(error) => {
            let codigo = estado_error_base_datos(&error);
            if transaccion.rollback().await.is_err() {
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            Err(codigo)
        }
    }
}
async fn obtener(
    State(pool): State<Estado>,
    Path(id): Path<i64>,
) -> Result<Json<Producto>, StatusCode> {
    let fila = sqlx::query("SELECT id, nombre, precio FROM productos WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match fila {
        Some(fila) => Ok(Json(producto_de_fila(fila))),
        None => Err(StatusCode::NOT_FOUND),
    }
}
async fn actualizar(
    State(pool): State<Estado>,
    Path(id): Path<i64>,
    Json(datos): Json<NuevoProducto>,
) -> Result<Json<Producto>, StatusCode> {
    if !datos_validos(&datos) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let resultado = sqlx::query("UPDATE productos SET nombre = ?, precio = ? WHERE id = ?")
        .bind(&datos.nombre)
        .bind(datos.precio)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if resultado.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(Producto {
        id,
        nombre: datos.nombre,
        precio: datos.precio,
    }))
}
async fn eliminar(
    State(pool): State<Estado>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    let resultado = sqlx::query("DELETE FROM productos WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if resultado.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
#[tokio::main]
async fn main() {
    let ruta_db = std::env::var("CRUD_SQLITE_PATH")
        .unwrap_or_else(|_| "./crud_persistencia.sqlite".to_string());
    let opciones = SqliteConnectOptions::new()
        .filename(&ruta_db)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(10));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opciones)
        .await
        .expect("no se pudo abrir la base SQLite");
    let inicio = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    IDENTIDAD_SERVIDOR
        .set(IdentidadServidor {
            pid: std::process::id(),
            instancia: format!("{}-{}", std::process::id(), inicio),
        })
        .unwrap();
    sqlx::query(
            "CREATE TABLE IF NOT EXISTS productos (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL UNIQUE CHECK(length(trim(nombre)) > 0), precio REAL NOT NULL CHECK(precio >= 0))",
        )
        .execute(&pool)
        .await
        .expect("no se pudo crear la tabla productos");
    if std::env::var("CRUD_RESET_DB").unwrap_or_default() == "1" {
        sqlx::query("DELETE FROM productos")
            .execute(&pool)
            .await
            .expect("no se pudo vaciar productos para el benchmark");
        sqlx::query("DELETE FROM sqlite_sequence WHERE name = 'productos'")
            .execute(&pool)
            .await
            .expect("no se pudo reiniciar la secuencia de IDs");
    }
    let app = Router::new()
        .route("/__test/instance", get(consultar_instancia))
        .route("/productos", get(listar).post(crear))
        .route(
            "/productos/{id}",
            get(obtener).put(actualizar).delete(eliminar),
        )
        .with_state(pool);
    let direccion = std::env::var("CRUD_ADDR").unwrap_or_else(|_| "127.0.0.1:38212".to_string());
    let escucha = tokio::net::TcpListener::bind(direccion).await.unwrap();
    axum::serve(escucha, app).await.unwrap()
}
