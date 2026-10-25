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
use std::{
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize)]
struct Product {
    id: i64,
    #[serde(rename = "nombre")]
    name: String,
    #[serde(rename = "precio")]
    price: f64,
}

#[derive(Deserialize)]
struct NewProduct {
    #[serde(rename = "nombre")]
    name: String,
    #[serde(rename = "precio")]
    price: f64,
}

#[derive(Clone, Debug, Serialize)]
struct ServerIdentity {
    pid: u32,
    #[serde(rename = "instancia")]
    instance: String,
}

static SERVER_IDENTITY: OnceLock<ServerIdentity> = OnceLock::new();

async fn identity() -> Json<ServerIdentity> {
    Json(SERVER_IDENTITY.get().expect("server identity not initialized").clone())
}

fn valid(product: &NewProduct) -> bool {
    !product.name.trim().is_empty() && product.price.is_finite() && product.price >= 0.0
}

fn product_from_row(row: sqlx::sqlite::SqliteRow) -> Product {
    Product {
        id: row.get("id"),
        name: row.get("nombre"),
        price: row.get("precio"),
    }
}

fn database_status(error: &sqlx::Error) -> StatusCode {
    match error {
        sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
            StatusCode::CONFLICT
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn list(State(pool): State<SqlitePool>) -> Result<Json<Vec<Product>>, StatusCode> {
    let rows = sqlx::query("SELECT id, nombre, precio FROM productos ORDER BY id")
        .fetch_all(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let products = rows.into_iter().map(product_from_row).collect();
    Ok(Json(products))
}

async fn create(
    State(pool): State<SqlitePool>,
    Json(product): Json<NewProduct>,
) -> Result<(StatusCode, Json<Product>), StatusCode> {
    if !valid(&product) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let result = sqlx::query("INSERT INTO productos (nombre, precio) VALUES (?, ?)")
        .bind(&product.name)
        .bind(product.price)
        .execute(&mut *transaction)
        .await;
    match result {
        Ok(result) => {
            let saved = Product {
                id: result.last_insert_rowid(),
                name: product.name,
                price: product.price,
            };
            transaction
                .commit()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok((StatusCode::CREATED, Json(saved)))
        }
        Err(error) => {
            let status = database_status(&error);
            transaction
                .rollback()
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Err(status)
        }
    }
}

async fn get_product(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<Product>, StatusCode> {
    let row = sqlx::query("SELECT id, nombre, precio FROM productos WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    row.map(product_from_row)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn update(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Json(product): Json<NewProduct>,
) -> Result<Json<Product>, StatusCode> {
    if !valid(&product) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let result = sqlx::query("UPDATE productos SET nombre = ?, precio = ? WHERE id = ?")
        .bind(&product.name)
        .bind(product.price)
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(Json(Product {
        id,
        name: product.name,
        price: product.price,
    }))
}

async fn delete(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query("DELETE FROM productos WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[tokio::main]
async fn main() {
    let database_path = std::env::var("CRUD_SQLITE_PATH")
        .unwrap_or_else(|_| "./crud_persistencia.sqlite".to_string());
    let options = SqliteConnectOptions::new()
        .filename(&database_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(10));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .expect("could not open SQLite database");
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before Unix epoch")
        .as_nanos();
    SERVER_IDENTITY
        .set(ServerIdentity {
            pid: std::process::id(),
            instance: format!("{}-{started_at}", std::process::id()),
        })
        .expect("server identity initialized more than once");
    sqlx::query("CREATE TABLE IF NOT EXISTS productos (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL UNIQUE CHECK(length(trim(nombre)) > 0), precio REAL NOT NULL CHECK(precio >= 0))")
        .execute(&pool)
        .await
        .expect("could not create productos table");
    if std::env::var("CRUD_RESET_DB").unwrap_or_default() == "1" {
        sqlx::query("DELETE FROM productos")
            .execute(&pool)
            .await
            .expect("could not clear products for benchmark");
        sqlx::query("DELETE FROM sqlite_sequence WHERE name = 'productos'")
            .execute(&pool)
            .await
            .expect("could not reset product IDs");
    }
    let app = Router::new()
        .route("/__test/instance", get(identity))
        .route("/productos", get(list).post(create))
        .route(
            "/productos/{id}",
            get(get_product).put(update).delete(delete),
        )
        .with_state(pool);
    let address = std::env::var("CRUD_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38212".to_string());
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("could not bind API listener");
    axum::serve(listener, app)
        .await
        .expect("API server failed");
}
