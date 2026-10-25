use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
fn solicitar(direccion: &str, metodo: &str, ruta: &str, cuerpo: &str) -> (u16, String) {
    let mut flujo = TcpStream::connect(direccion)
        .expect("no se pudo conectar con la API SQLite");
    write!(
        flujo,
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        metodo, ruta, direccion, cuerpo.len(), cuerpo
    )
        .expect("no se pudo escribir la solicitud");
    let mut respuesta = String::new();
    flujo.read_to_string(&mut respuesta).expect("no se pudo leer la respuesta");
    let (cabeceras, contenido) = respuesta
        .split_once("\r\n\r\n")
        .expect("respuesta HTTP incompleta");
    let codigo = cabeceras
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse::<u16>()
        .unwrap();
    (codigo, contenido.to_string())
}
fn id_de(contenido: &str) -> i64 {
    let inicio = contenido.find("\"id\":").expect("la respuesta no contiene id") + 5;
    let digitos = contenido[inicio..]
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect::<String>();
    digitos.parse::<i64>().expect("id inválido")
}
fn ruta_estado() -> String {
    std::env::var("CRUD_TEST_STATE_PATH")
        .unwrap_or_else(|_| "/tmp/oxido-crud-sqlite-instance.txt".to_string())
}
fn instancia_actual(direccion: &str) -> String {
    let (codigo, contenido) = solicitar(direccion, "GET", "/__test/instance", "");
    assert_eq!(codigo, 200, "no se pudo consultar la identidad del servidor");
    let inicio = contenido.find("\"instancia\":\"").expect("falta instancia") + 13;
    let resto = &contenido[inicio..];
    resto[..resto.find('"').unwrap()].to_string()
}
fn registrar_instancia(direccion: &str) {
    fs::write(ruta_estado(), instancia_actual(direccion))
        .expect("no se pudo guardar la identidad del servidor")
}
fn verificar_instancia(direccion: &str, reiniciada: bool) {
    let anterior = fs::read_to_string(ruta_estado())
        .expect("falta checkpoint; ejecuta primero el modo preparar");
    let actual = instancia_actual(direccion);
    if reiniciada {
        assert_ne!(
            actual, anterior,
            "el PID/identificador no cambió: el servidor no se reinició entre fases"
        )
    } else {
        assert_eq!(
            actual, anterior,
            "el servidor cambió durante una fase que debía usar la misma instancia"
        )
    }
    fs::write(ruta_estado(), actual)
        .expect("no se pudo actualizar el checkpoint de instancia")
}
fn crear_uno(direccion: String, numero: u32) -> i64 {
    let cuerpo = format!("{{\"nombre\":\"Concurrente {}\",\"precio\":1.25}}", numero);
    let (codigo, contenido) = solicitar(&direccion, "POST", "/productos", &cuerpo);
    assert_eq!(codigo, 201, "falló una inserción concurrente: {}", contenido);
    id_de(&contenido)
}
fn crear_lote(direccion: &str, cantidad: u32) -> Vec<i64> {
    let puerta = Arc::new((Mutex::new(false), Condvar::new()));
    let mut tareas: Vec<JoinHandle<i64>> = Vec::new();
    for numero in 0..cantidad {
        let puerta_hilo = puerta.clone();
        let direccion_hilo = direccion.to_string();
        tareas
            .push(
                thread::spawn(move || {
                    let (candado, condicion) = &*puerta_hilo;
                    let listo = candado.lock().unwrap();
                    let guardia = condicion
                        .wait_while(listo, |iniciada| !*iniciada)
                        .unwrap();
                    drop(guardia);
                    crear_uno(direccion_hilo, numero)
                }),
            )
    }
    let (candado, condicion) = &*puerta;
    let mut iniciada = candado.lock().unwrap();
    *iniciada = true;
    condicion.notify_all();
    drop(iniciada);
    let mut ids = Vec::new();
    for tarea in tareas {
        ids.push(tarea.join().expect("falló un hilo de inserción"))
    }
    ids
}
fn cpu_ticks(pid: u32) -> u64 {
    let datos = std::fs::read_to_string(format!("/proc/{}/stat", pid))
        .unwrap_or_default();
    let cierre = match datos.rfind(')') {
        Some(posicion) => posicion + 2,
        None => return 0,
    };
    let campos = datos[cierre..].split_whitespace().collect::<Vec<&str>>();
    if campos.len() < 13 {
        return 0;
    }
    campos[11].parse::<u64>().unwrap_or(0) + campos[12].parse::<u64>().unwrap_or(0)
}
fn rss_kib(pid: u32) -> u64 {
    let datos = std::fs::read_to_string(format!("/proc/{}/status", pid))
        .unwrap_or_default();
    for linea in datos.lines() {
        if linea.starts_with("VmRSS:") {
            return linea
                .split_whitespace()
                .nth(1)
                .unwrap_or("0")
                .parse::<u64>()
                .unwrap_or(0);
        }
    }
    0
}
fn medir_lote(
    direccion: &str,
    prefijo: &str,
    cantidad: u32,
) -> (usize, Vec<u128>, Duration) {
    let puerta = Arc::new((Mutex::new(false), Condvar::new()));
    let mut tareas: Vec<JoinHandle<(u16, u128)>> = Vec::new();
    for indice in 0..cantidad {
        let puerta_hilo = puerta.clone();
        let direccion_hilo = direccion.to_string();
        let nombre = format!("{}-{}", prefijo, indice);
        tareas
            .push(
                thread::spawn(move || {
                    let (candado, condicion) = &*puerta_hilo;
                    let listo = candado.lock().unwrap();
                    let guardia = condicion
                        .wait_while(listo, |iniciada| !*iniciada)
                        .unwrap();
                    drop(guardia);
                    let cuerpo = format!("{{\"nombre\":\"{}\",\"precio\":2.5}}", nombre);
                    let inicio = Instant::now();
                    let (codigo, _) = solicitar(
                        &direccion_hilo,
                        "POST",
                        "/productos",
                        &cuerpo,
                    );
                    (codigo, inicio.elapsed().as_nanos())
                }),
            )
    }
    let cronometro = Instant::now();
    let (candado, condicion) = &*puerta;
    let mut iniciada = candado.lock().unwrap();
    *iniciada = true;
    condicion.notify_all();
    drop(iniciada);
    let mut correctas = 0;
    let mut latencias = Vec::new();
    for tarea in tareas {
        let (codigo, latencia) = tarea.join().expect("falló un hilo del benchmark");
        if codigo == 201 {
            correctas += 1;
        }
        latencias.push(latencia)
    }
    (correctas, latencias, cronometro.elapsed())
}
fn percentil_ms(latencias: &Vec<u128>, percentil: usize) -> f64 {
    let mut ordenadas = latencias.clone();
    ordenadas.sort_unstable();
    let indice = (ordenadas.len() * percentil + 99) / 100 - 1;
    ordenadas[indice] as f64 / 1_000_000.0
}
fn ejecutar_benchmark(direccion: &str) {
    let pid = std::process::id();
    let (codigo_identidad, identidad) = solicitar(
        direccion,
        "GET",
        "/__test/instance",
        "",
    );
    assert_eq!(codigo_identidad, 200);
    let inicio_pid = identidad
        .find("\"pid\":")
        .expect("no se recibió el PID del servidor") + 6;
    let pid_texto = identidad[inicio_pid..]
        .chars()
        .take_while(|char| char.is_ascii_digit())
        .collect::<String>();
    let pid_servidor = pid_texto.parse::<u32>().unwrap();
    let prefijo = format!(
        "Bench-{}-{}", pid, SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
        .as_nanos()
    );
    let cpu_inicial = cpu_ticks(pid_servidor);
    let pico_rss = Arc::new(AtomicU64::new(rss_kib(pid_servidor)));
    let detener_muestreo = Arc::new(AtomicBool::new(false));
    let pico_hilo = pico_rss.clone();
    let detener_hilo = detener_muestreo.clone();
    let muestreador = thread::spawn(move || {
        while !detener_hilo.load(Ordering::Relaxed) {
            let actual = rss_kib(pid_servidor);
            pico_hilo.fetch_max(actual, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(10));
        }
    });
    let mut solicitudes = 0usize;
    for ronda in 0..2 {
        for cantidad in [50u32, 100, 200] {
            let nombre_lote = format!("{}-{}-{}", prefijo, ronda, cantidad);
            let (correctas, latencias, tiempo) = medir_lote(
                direccion,
                &nombre_lote,
                cantidad,
            );
            solicitudes += cantidad as usize;
            let errores = cantidad as usize - correctas;
            let qps = correctas as f64 / tiempo.as_secs_f64();
            println!(
                "benchmark carga={} exito={} error={} qps={:.2} p50_ms={:.2} p95_ms={:.2} p99_ms={:.2}",
                cantidad, correctas, errores, qps, percentil_ms(& latencias, 50),
                percentil_ms(& latencias, 95), percentil_ms(& latencias, 99)
            )
        }
    }
    detener_muestreo.store(true, Ordering::Relaxed);
    muestreador.join().unwrap();
    let delta_cpu_ticks = cpu_ticks(pid_servidor).saturating_sub(cpu_inicial);
    let memoria_pico = pico_rss.load(Ordering::Relaxed);
    if memoria_pico == 0 {
        println!(
            "recursos del servidor: /proc no disponible; solicitudes medidas={}",
            solicitudes
        )
    } else {
        println!(
            "recursos del servidor durante la carga: delta_cpu_ticks={} rss_muestreado_max_kib={}",
            delta_cpu_ticks, memoria_pico
        )
    }
    println!(
        "Benchmark funcional terminado; no es un objetivo de rendimiento. Compilación excluida: el cronómetro vive dentro del ejecutable."
    )
}
fn verificar_inicial(direccion: &str) {
    registrar_instancia(direccion);
    let (codigo, lista) = solicitar(direccion, "GET", "/productos", "");
    assert_eq!(codigo, 200);
    assert_eq!(lista, "[]", "la prueba debe empezar con una base vacía");
    let (codigo_crear, contenido) = solicitar(
        direccion,
        "POST",
        "/productos",
        "{\"nombre\":\"Producto persistente\",\"precio\":12.5}",
    );
    assert_eq!(codigo_crear, 201);
    assert_eq!(id_de(& contenido), 1, "el primer ID SQLite debe ser 1");
    println!("Producto inicial insertado y confirmado en SQLite")
}
fn verificar_reinicio(direccion: &str) {
    verificar_instancia(direccion, true);
    let (codigo, producto) = solicitar(direccion, "GET", "/productos/1", "");
    assert_eq!(codigo, 200, "el producto no sobrevivió al reinicio del servidor");
    assert!(
        producto.contains("Producto persistente"), "se perdió el contenido persistido"
    );
    println!("Persistencia confirmada tras reiniciar el servidor")
}
fn ejecutar_carga_y_mutaciones(direccion: &str) {
    verificar_instancia(direccion, false);
    let mut ids = crear_lote(direccion, 40);
    ids.sort_unstable();
    assert_eq!(ids.len(), 40);
    for (indice, id) in ids.iter().enumerate() {
        assert_eq!(
            * id, indice as i64 + 2, "ID duplicado o inesperado en la carga concurrente"
        )
    }
    let (codigo_antes, filas_antes) = solicitar(direccion, "GET", "/productos", "");
    assert_eq!(codigo_antes, 200);
    let cantidad_antes = filas_antes.matches("\"id\":").count();
    let (codigo_duplicado, _) = solicitar(
        direccion,
        "POST",
        "/productos",
        "{\"nombre\":\"Producto persistente\",\"precio\":99.0}",
    );
    assert_eq!(
        codigo_duplicado, 409,
        "la restricción SQLite UNIQUE debe convertirse en conflicto HTTP"
    );
    let (codigo_despues, filas_despues) = solicitar(direccion, "GET", "/productos", "");
    assert_eq!(codigo_despues, 200);
    assert_eq!(
        filas_despues.matches("\"id\":").count(), cantidad_antes,
        "la inserción rechazada no debe dejar ninguna fila"
    );
    let (codigo_actualizar, actualizado) = solicitar(
        direccion,
        "PUT",
        "/productos/1",
        "{\"nombre\":\"Producto actualizado\",\"precio\":20.5}",
    );
    assert_eq!(codigo_actualizar, 200);
    assert!(actualizado.contains("Producto actualizado"));
    let (codigo_eliminar, _) = solicitar(direccion, "DELETE", "/productos/2", "");
    assert_eq!(codigo_eliminar, 204);
    let (codigo_lista, lista) = solicitar(direccion, "GET", "/productos", "");
    assert_eq!(codigo_lista, 200);
    assert_eq!(
        lista.matches("\"id\":").count(), 40,
        "el error UNIQUE no debe dejar una fila parcial y el DELETE debe quitar una fila"
    );
    println!(
        "40 inserciones concurrentes únicas; conflicto SQL sin cambios en el recuento; actualización y eliminación comprobadas"
    )
}
fn verificar_mutaciones_persistidas(direccion: &str) {
    verificar_instancia(direccion, true);
    let (codigo_actualizado, actualizado) = solicitar(
        direccion,
        "GET",
        "/productos/1",
        "",
    );
    assert_eq!(codigo_actualizado, 200);
    assert!(
        actualizado.contains("Producto actualizado"),
        "la actualización no sobrevivió al reinicio"
    );
    let (codigo_eliminado, _) = solicitar(direccion, "GET", "/productos/2", "");
    assert_eq!(codigo_eliminado, 404, "la eliminación no sobrevivió al reinicio");
    let (codigo_lista, lista) = solicitar(direccion, "GET", "/productos", "");
    assert_eq!(codigo_lista, 200);
    assert_eq!(lista.matches("\"id\":").count(), 40);
    println!(
        "Resultados observables de actualización, eliminación y conflicto SQL comprobados tras reiniciar"
    )
}
fn main() {
    let direccion = std::env::var("CRUD_TEST_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:38212".to_string());
    let modo = std::env::var("CRUD_TEST_MODE")
        .expect("define CRUD_TEST_MODE: preparar, reinicio, mutaciones o final");
    match modo.as_str() {
        "preparar" => verificar_inicial(&direccion),
        "reinicio" => verificar_reinicio(&direccion),
        "mutaciones" => ejecutar_carga_y_mutaciones(&direccion),
        "final" => verificar_mutaciones_persistidas(&direccion),
        "benchmark" => ejecutar_benchmark(&direccion),
        _ => panic!("CRUD_TEST_MODE no reconocido"),
    }
}
