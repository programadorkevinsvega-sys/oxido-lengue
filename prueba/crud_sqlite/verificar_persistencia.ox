sintaxis indentacion
usar std::fs
usar std::io::{Read, Write}
usar std::net::TcpStream
usar std::sync::atomic::{AtomicBool, AtomicU64, Ordering}
usar std::sync::{Arc, Condvar, Mutex}
usar std::thread::{self, JoinHandle}
usar std::time::{Duration, Instant, SystemTime, UNIX_EPOCH}

funcion solicitar(direccion: &str, metodo: &str, ruta: &str, cuerpo: &str) -> (u16, Cadena):
    dejar mutable flujo = TcpStream::connect(direccion).expect("no se pudo conectar con la API SQLite")
    escribir!(flujo, "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", metodo, ruta, direccion, cuerpo.len(), cuerpo).expect("no se pudo escribir la solicitud")
    dejar mutable respuesta = Cadena::nuevo()
    flujo.read_to_string(&mut respuesta).expect("no se pudo leer la respuesta")
    dejar (cabeceras, contenido) = respuesta.split_once("\r\n\r\n").expect("respuesta HTTP incompleta")
    dejar codigo = cabeceras.lines().next().unwrap().split_whitespace().nth(1).unwrap().parse::<u16>().unwrap()
    (codigo, contenido.to_string())

funcion id_de(contenido: &str) -> i64:
    dejar inicio = contenido.find("\"id\":").expect("la respuesta no contiene id") + 5
    dejar digitos = contenido[inicio..].chars().take_while(|caracter| caracter.is_ascii_digit()).collect::<Cadena>()
    digitos.parse::<i64>().expect("id inválido")

funcion ruta_estado() -> Cadena:
    std::env::var("CRUD_TEST_STATE_PATH")
        .unwrap_or_else(|_| "/tmp/oxido-crud-sqlite-instance.txt".to_string())

funcion instancia_actual(direccion: &str) -> Cadena:
    dejar (codigo, contenido) = solicitar(direccion, "GET", "/__test/instance", "")
    assert_eq!(codigo, 200, "no se pudo consultar la identidad del servidor")
    dejar inicio = contenido.find("\"instancia\":\"").expect("falta instancia") + 13
    dejar resto = &contenido[inicio..]
    resto[..resto.find('"').unwrap()].to_string()

funcion registrar_instancia(direccion: &str):
    fs::write(ruta_estado(), instancia_actual(direccion)).expect("no se pudo guardar la identidad del servidor")

funcion verificar_instancia(direccion: &str, reiniciada: bool):
    dejar anterior = fs::read_to_string(ruta_estado()).expect("falta checkpoint; ejecuta primero el modo preparar")
    dejar actual = instancia_actual(direccion)
    si reiniciada:
        assert_ne!(actual, anterior, "el PID/identificador no cambió: el servidor no se reinició entre fases")
    sino:
        assert_eq!(actual, anterior, "el servidor cambió durante una fase que debía usar la misma instancia")
    fs::write(ruta_estado(), actual).expect("no se pudo actualizar el checkpoint de instancia")

funcion crear_uno(direccion: Cadena, numero: u32) -> i64:
    dejar cuerpo = formato!("{{\"nombre\":\"Concurrente {}\",\"precio\":1.25}}", numero)
    dejar (codigo, contenido) = solicitar(&direccion, "POST", "/productos", &cuerpo)
    assert_eq!(codigo, 201, "falló una inserción concurrente: {}", contenido)
    id_de(&contenido)

funcion crear_lote(direccion: &str, cantidad: u32) -> Vector<i64>:
    dejar puerta = Arc::new((Mutex::new(falso), Condvar::new()))
    dejar mutable tareas: Vector<JoinHandle<i64>> = Vector::nuevo()
    para numero en 0..cantidad:
        dejar puerta_hilo = puerta.clone()
        dejar direccion_hilo = direccion.to_string()
        tareas.agregar(thread::spawn(move || {
            let (candado, condicion) = &*puerta_hilo;
            let listo = candado.lock().unwrap();
            let guardia = condicion.wait_while(listo, |iniciada| !*iniciada).unwrap();
            drop(guardia);
            crear_uno(direccion_hilo, numero)
        }))
    dejar (candado, condicion) = &*puerta
    dejar mutable iniciada = candado.lock().unwrap()
    *iniciada = verdadero
    condicion.notify_all()
    drop(iniciada)
    dejar mutable ids = Vector::nuevo()
    para tarea en tareas:
        ids.agregar(tarea.join().expect("falló un hilo de inserción"))
    ids

funcion cpu_ticks(pid: u32) -> u64:
    dejar datos = std::fs::read_to_string(formato!("/proc/{}/stat", pid)).unwrap_or_default()
    dejar cierre = coincidir datos.rfind(')'):
        Some(posicion) => posicion + 2
        None => retornar 0
    dejar campos = datos[cierre..].split_whitespace().collect::<Vector<&str>>()
    si campos.len() < 13:
        retornar 0
    campos[11].parse::<u64>().unwrap_or(0) + campos[12].parse::<u64>().unwrap_or(0)

funcion rss_kib(pid: u32) -> u64:
    dejar datos = std::fs::read_to_string(formato!("/proc/{}/status", pid)).unwrap_or_default()
    para linea en datos.lines():
        si linea.starts_with("VmRSS:"):
            retornar linea.split_whitespace().nth(1).unwrap_or("0").parse::<u64>().unwrap_or(0)
    0

funcion medir_lote(direccion: &str, prefijo: &str, cantidad: u32) -> (usize, Vector<u128>, Duration):
    dejar puerta = Arc::new((Mutex::new(falso), Condvar::new()))
    dejar mutable tareas: Vector<JoinHandle<(u16, u128)>> = Vector::nuevo()
    para indice en 0..cantidad:
        dejar puerta_hilo = puerta.clone()
        dejar direccion_hilo = direccion.to_string()
        dejar nombre = formato!("{}-{}", prefijo, indice)
        tareas.agregar(thread::spawn(move || {
            let (candado, condicion) = &*puerta_hilo;
            let listo = candado.lock().unwrap();
            let guardia = condicion.wait_while(listo, |iniciada| !*iniciada).unwrap();
            drop(guardia);
            let cuerpo = format!("{{\"nombre\":\"{}\",\"precio\":2.5}}", nombre);
            let inicio = Instant::now();
            let (codigo, _) = solicitar(&direccion_hilo, "POST", "/productos", &cuerpo);
            (codigo, inicio.elapsed().as_nanos())
        }))
    dejar cronometro = Instant::now()
    dejar (candado, condicion) = &*puerta
    dejar mutable iniciada = candado.lock().unwrap()
    *iniciada = verdadero
    condicion.notify_all()
    drop(iniciada)
    dejar mutable correctas = 0
    dejar mutable latencias = Vector::nuevo()
    para tarea en tareas:
        dejar (codigo, latencia) = tarea.join().expect("falló un hilo del benchmark")
        si codigo == 201:
            correctas += 1
        latencias.push(latencia)
    (correctas, latencias, cronometro.elapsed())

funcion percentil_ms(latencias: &Vector<u128>, percentil: usize) -> f64:
    dejar mutable ordenadas = latencias.clone()
    ordenadas.sort_unstable()
    dejar indice = (ordenadas.len() * percentil + 99) / 100 - 1
    ordenadas[indice] as f64 / 1_000_000.0

funcion ejecutar_benchmark(direccion: &str):
    dejar pid = std::process::id()
    dejar (codigo_identidad, identidad) = solicitar(direccion, "GET", "/__test/instance", "")
    assert_eq!(codigo_identidad, 200)
    dejar inicio_pid = identidad.find("\"pid\":").expect("no se recibió el PID del servidor") + 6
    dejar pid_texto = identidad[inicio_pid..].chars().take_while(|caracter| caracter.is_ascii_digit()).collect::<Cadena>()
    dejar pid_servidor = pid_texto.parse::<u32>().unwrap()
    dejar prefijo = formato!("Bench-{}-{}", pid, SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos())
    dejar cpu_inicial = cpu_ticks(pid_servidor)
    dejar pico_rss = Arc::new(AtomicU64::new(rss_kib(pid_servidor)))
    dejar detener_muestreo = Arc::new(AtomicBool::new(falso))
    dejar pico_hilo = pico_rss.clone()
    dejar detener_hilo = detener_muestreo.clone()
    dejar muestreador = thread::spawn(move || {
        while !detener_hilo.load(Ordering::Relaxed) {
            let actual = rss_kib(pid_servidor);
            pico_hilo.fetch_max(actual, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(10));
        }
    })
    dejar mutable solicitudes = 0usize
    para ronda en 0..2:
        para cantidad en [50u32, 100, 200]:
            dejar nombre_lote = formato!("{}-{}-{}", prefijo, ronda, cantidad)
            dejar (correctas, latencias, tiempo) = medir_lote(direccion, &nombre_lote, cantidad)
            solicitudes += cantidad as usize
            dejar errores = cantidad as usize - correctas
            dejar qps = correctas as f64 / tiempo.as_secs_f64()
            imprimir!("benchmark carga={} exito={} error={} qps={:.2} p50_ms={:.2} p95_ms={:.2} p99_ms={:.2}", cantidad, correctas, errores, qps, percentil_ms(&latencias, 50), percentil_ms(&latencias, 95), percentil_ms(&latencias, 99))
    detener_muestreo.store(verdadero, Ordering::Relaxed)
    muestreador.join().unwrap()
    dejar delta_cpu_ticks = cpu_ticks(pid_servidor).saturating_sub(cpu_inicial)
    dejar memoria_pico = pico_rss.load(Ordering::Relaxed)
    si memoria_pico == 0:
        imprimir!("recursos del servidor: /proc no disponible; solicitudes medidas={}", solicitudes)
    sino:
        imprimir!("recursos del servidor durante la carga: delta_cpu_ticks={} rss_muestreado_max_kib={}", delta_cpu_ticks, memoria_pico)
    imprimir!("Benchmark funcional terminado; no es un objetivo de rendimiento. Compilación excluida: el cronómetro vive dentro del ejecutable.")

funcion verificar_inicial(direccion: &str):
    registrar_instancia(direccion)
    dejar (codigo, lista) = solicitar(direccion, "GET", "/productos", "")
    assert_eq!(codigo, 200)
    assert_eq!(lista, "[]", "la prueba debe empezar con una base vacía")
    dejar (codigo_crear, contenido) = solicitar(direccion, "POST", "/productos", "{\"nombre\":\"Producto persistente\",\"precio\":12.5}")
    assert_eq!(codigo_crear, 201)
    assert_eq!(id_de(&contenido), 1, "el primer ID SQLite debe ser 1")
    imprimir!("Producto inicial insertado y confirmado en SQLite")

funcion verificar_reinicio(direccion: &str):
    verificar_instancia(direccion, verdadero)
    dejar (codigo, producto) = solicitar(direccion, "GET", "/productos/1", "")
    assert_eq!(codigo, 200, "el producto no sobrevivió al reinicio del servidor")
    assert!(producto.contains("Producto persistente"), "se perdió el contenido persistido")
    imprimir!("Persistencia confirmada tras reiniciar el servidor")

funcion ejecutar_carga_y_mutaciones(direccion: &str):
    verificar_instancia(direccion, falso)
    dejar mut ids = crear_lote(direccion, 40)
    ids.sort_unstable()
    assert_eq!(ids.len(), 40)
    para (indice, id) en ids.iter().enumerate():
        assert_eq!(*id, indice as i64 + 2, "ID duplicado o inesperado en la carga concurrente")
    dejar (codigo_antes, filas_antes) = solicitar(direccion, "GET", "/productos", "")
    assert_eq!(codigo_antes, 200)
    dejar cantidad_antes = filas_antes.matches("\"id\":").count()
    dejar (codigo_duplicado, _) = solicitar(direccion, "POST", "/productos", "{\"nombre\":\"Producto persistente\",\"precio\":99.0}")
    assert_eq!(codigo_duplicado, 409, "la restricción SQLite UNIQUE debe convertirse en conflicto HTTP")
    dejar (codigo_despues, filas_despues) = solicitar(direccion, "GET", "/productos", "")
    assert_eq!(codigo_despues, 200)
    assert_eq!(filas_despues.matches("\"id\":").count(), cantidad_antes, "la inserción rechazada no debe dejar ninguna fila")
    dejar (codigo_actualizar, actualizado) = solicitar(direccion, "PUT", "/productos/1", "{\"nombre\":\"Producto actualizado\",\"precio\":20.5}")
    assert_eq!(codigo_actualizar, 200)
    assert!(actualizado.contains("Producto actualizado"))
    dejar (codigo_eliminar, _) = solicitar(direccion, "DELETE", "/productos/2", "")
    assert_eq!(codigo_eliminar, 204)
    dejar (codigo_lista, lista) = solicitar(direccion, "GET", "/productos", "")
    assert_eq!(codigo_lista, 200)
    assert_eq!(lista.matches("\"id\":").count(), 40, "el error UNIQUE no debe dejar una fila parcial y el DELETE debe quitar una fila")
    imprimir!("40 inserciones concurrentes únicas; conflicto SQL sin cambios en el recuento; actualización y eliminación comprobadas")

funcion verificar_mutaciones_persistidas(direccion: &str):
    verificar_instancia(direccion, verdadero)
    dejar (codigo_actualizado, actualizado) = solicitar(direccion, "GET", "/productos/1", "")
    assert_eq!(codigo_actualizado, 200)
    assert!(actualizado.contains("Producto actualizado"), "la actualización no sobrevivió al reinicio")
    dejar (codigo_eliminado, _) = solicitar(direccion, "GET", "/productos/2", "")
    assert_eq!(codigo_eliminado, 404, "la eliminación no sobrevivió al reinicio")
    dejar (codigo_lista, lista) = solicitar(direccion, "GET", "/productos", "")
    assert_eq!(codigo_lista, 200)
    assert_eq!(lista.matches("\"id\":").count(), 40)
    imprimir!("Resultados observables de actualización, eliminación y conflicto SQL comprobados tras reiniciar")

funcion principal():
    dejar direccion = std::env::var("CRUD_TEST_ADDR").unwrap_or_else(|_| "127.0.0.1:38212".to_string())
    dejar modo = std::env::var("CRUD_TEST_MODE").expect("define CRUD_TEST_MODE: preparar, reinicio, mutaciones o final")
    coincidir modo.as_str():
        "preparar" => verificar_inicial(&direccion)
        "reinicio" => verificar_reinicio(&direccion)
        "mutaciones" => ejecutar_carga_y_mutaciones(&direccion)
        "final" => verificar_mutaciones_persistidas(&direccion)
        "benchmark" => ejecutar_benchmark(&direccion)
        _ => panic!("CRUD_TEST_MODE no reconocido")
