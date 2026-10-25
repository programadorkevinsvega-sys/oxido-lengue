sintaxis indentacion
usar std::collections::{BTreeMap, HashSet, VecDeque}
usar std::fs
usar std::io::{Cursor, Read}
usar std::path::PathBuf
usar std::sync::{mpsc, Arc, Mutex}
usar std::thread
usar std::time::{Duration, Instant}

funcion probar_colecciones() -> bool:
    dejar mutable puntuaciones = BTreeMap::new()
    puntuaciones.insert("Ana".to_string(), 8)
    puntuaciones.insert("Luis".to_string(), 5)
    si puntuaciones.len() != 2 || puntuaciones.get("Ana") != Some(&8):
        retornar falso
    dejar mutable etiquetas = HashSet::new()
    etiquetas.insert("compilador")
    etiquetas.insert("prueba")
    si !etiquetas.contains("prueba") || etiquetas.len() != 2:
        retornar falso
    dejar mutable cola = VecDeque::new()
    cola.push_back("primero")
    cola.push_back("segundo")
    cola.pop_front() == Some("primero") && cola.front() == Some(&"segundo")

funcion probar_archivos_y_lectura() -> bool:
    dejar directorio = std::env::temp_dir().join(formato!("oxido-prueba-std-{}", std::process::id()))
    fs::create_dir_all(&directorio).expect("no se pudo crear el directorio temporal")
    dejar ruta = directorio.join("datos.txt")
    dejar contenido = "Óxido conserva Unicode y líneas.\nsegunda línea\n"
    fs::write(&ruta, contenido).expect("no se pudo escribir el archivo temporal")
    dejar leido = fs::read_to_string(&ruta).expect("no se pudo leer el archivo temporal")
    dejar mut cursor = Cursor::new(leido.as_bytes())
    dejar mutable buffer = Cadena::nuevo()
    cursor.read_to_string(&mut buffer).expect("Cursor no pudo leer los bytes")
    dejar archivo_eliminado = fs::remove_file(&ruta).is_ok()
    dejar directorio_eliminado = fs::remove_dir(&directorio).is_ok()
    buffer == contenido && archivo_eliminado && directorio_eliminado

funcion probar_hilos_y_canales() -> bool:
    dejar compartido = Arc::new(Mutex::new(Vector::nuevo()))
    dejar mutable hilos = Vector::nuevo()
    para numero en 0..8:
        dejar compartido_hilo = compartido.clone()
        hilos.push(thread::spawn(move || {
            compartido_hilo.lock().unwrap().push(numero * numero);
        }))
    para hilo en hilos:
        si hilo.join().is_err():
            retornar falso
    dejar valores = compartido.lock().unwrap()
    si valores.len() != 8 || !valores.contains(&49):
        retornar falso
    dejar (emisor, receptor) = mpsc::channel()
    emisor.send("resultado-del-hilo").unwrap()
    receptor.recv_timeout(Duration::from_secs(1)).unwrap() == "resultado-del-hilo"

funcion probar_rutas_y_tiempo() -> bool:
    dejar relativa = PathBuf::from("salida").join("resultado.txt")
    dejar inicio = Instant::now()
    thread::sleep(Duration::from_millis(2))
    relativa.ends_with("resultado.txt") && inicio.elapsed() >= Duration::from_millis(1)

funcion principal():
    dejar pruebas: [(&str, fn() -> bool); 4] = [
        ("colecciones", probar_colecciones),
        ("archivos e io", probar_archivos_y_lectura),
        ("hilos y canales", probar_hilos_y_canales),
        ("rutas y tiempo", probar_rutas_y_tiempo),
    ]
    dejar mutable correctas = 0
    para (nombre, prueba) en pruebas:
        dejar inicio = Instant::now()
        dejar resultado = prueba()
        si resultado:
            correctas += 1
            imprimir!("OK {} ({:?})", nombre, inicio.elapsed())
        sino:
            imprimir!("FALLO {}", nombre)
    afirmar_igual!(correctas, pruebas.len(), "falló una prueba de interoperabilidad con std")
    imprimir!("{} pruebas de biblioteca estándar completadas", correctas)
