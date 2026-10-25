sintaxis indentacion
usar std::io::{Read, Write}
usar std::net::TcpStream
usar std::sync::{Arc, Condvar, Mutex}
usar std::thread::{self, JoinHandle}

funcion solicitar(direccion: &str, metodo: &str, ruta: &str, cuerpo: &str) -> (u16, Cadena):
    dejar mutable flujo = TcpStream::connect(direccion).expect("no se pudo conectar con la API")
    escribir!(flujo, "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", metodo, ruta, direccion, cuerpo.len(), cuerpo).expect("no se pudo escribir la solicitud")
    dejar mutable respuesta = Cadena::nuevo()
    flujo.read_to_string(&mut respuesta).expect("no se pudo leer la respuesta")
    dejar (cabeceras, contenido) = respuesta.split_once("\r\n\r\n").expect("respuesta HTTP incompleta")
    dejar linea_estado = cabeceras.lines().next().expect("falta estado HTTP")
    dejar codigo = linea_estado.split_whitespace().nth(1).unwrap().parse::<u16>().unwrap()
    (codigo, contenido.to_string())

funcion id_de(contenido: &str) -> u32:
    dejar inicio = contenido.find("\"id\":").expect("la respuesta no contiene id") + 5
    dejar digitos = contenido[inicio..].chars().take_while(|caracter| caracter.is_ascii_digit()).collect::<Cadena>()
    digitos.parse::<u32>().expect("id inválido en la respuesta")

funcion crear_uno(direccion: Cadena, numero: u32) -> u32:
    dejar cuerpo = formato!("{{\"nombre\":\"Concurrente {}\",\"precio\":1.25}}", numero)
    dejar (codigo, contenido) = solicitar(&direccion, "POST", "/productos", &cuerpo)
    assert_eq!(codigo, 201, "la API rechazó la creación concurrente: {}", contenido)
    id_de(&contenido)

funcion crear_lote(direccion: &str, cantidad: u32, primer_id: u32) -> bool:
    dejar puerta = Arc::new((Mutex::new(falso), Condvar::new()))
    dejar mutable tareas: Vector<JoinHandle<u32>> = Vector::nuevo()
    para numero en 0..cantidad:
        dejar puerta_tarea = puerta.clone()
        dejar direccion_tarea = direccion.to_string()
        dejar tarea = thread::Builder::new().stack_size(256 * 1024).spawn(move || {
            let (candado, condicion) = &*puerta_tarea;
            let listo = candado.lock().unwrap();
            let guardia = condicion.wait_while(listo, |iniciada| !*iniciada).unwrap();
            drop(guardia);
            crear_uno(direccion_tarea, numero)
        })
        coincidir tarea:
            Ok(tarea) => tareas.agregar(tarea)
            Err(error) =>
                dejar (candado, condicion) = &*puerta
                dejar mutable iniciada = candado.lock().unwrap()
                *iniciada = verdadero
                condicion.notify_all()
                drop(iniciada)
                para tarea_existente en tareas:
                    tarea_existente.join().unwrap();
                eprintln!("no se pudo iniciar un trabajador: {}", error)
                retornar falso
    dejar (candado, condicion) = &*puerta
    dejar mutable iniciada = candado.lock().unwrap()
    *iniciada = verdadero
    condicion.notify_all()
    drop(iniciada)
    dejar mutable ids = Vector::nuevo()
    para tarea en tareas:
        coincidir tarea.join():
            Ok(id) => ids.agregar(id)
            Err(_) => retornar falso
    ids.sort_unstable()
    si ids.len() != cantidad como usize:
        retornar falso
    para (indice, id) en ids.iter().enumerate():
        si *id != primer_id + indice como u32:
            retornar falso
    verdadero

funcion comprobar_error(direccion: &str, cuerpo: &str, esperado: u16) -> bool:
    solicitar(direccion, "POST", "/productos", cuerpo).0 == esperado

funcion principal():
    dejar direccion = std::env::var("CRUD_TEST_ADDR").unwrap_or_else(|_| "127.0.0.1:38211".to_string())
    si std::env::var("CRUD_TEST_MODE").unwrap_or_default() == "overflow":
        dejar (codigo, _) = solicitar(&direccion, "POST", "/productos", "{\"nombre\":\"limite\",\"precio\":1.0}")
        assert_eq!(codigo, 409, "el desbordamiento debe responder 409")
        dejar (codigo_lista, contenido) = solicitar(&direccion, "GET", "/productos", "")
        assert_eq!(codigo_lista, 200)
        assert_eq!(contenido, "[]", "el desbordamiento no debe mutar el almacén")
        imprimir!("Prueba de desbordamiento: correcta")
        retornar
    assert!(comprobar_error(&direccion, "{", 400), "JSON inválido debe responder 400")
    assert!(comprobar_error(&direccion, "{\"nombre\":\"x\",\"precio\":1e9999}", 400), "precio no finito debe responder 400")
    assert!(comprobar_error(&direccion, "{\"nombre\":\"   \",\"precio\":1.0}", 400), "nombre vacío debe responder 400")
    assert!(comprobar_error(&direccion, "{\"nombre\":\"x\",\"precio\":-1.0}", 400), "precio negativo debe responder 400")
    dejar mutable total = 0u32
    para _ en 0..2:
        para cantidad en [100u32, 500, 1000]:
            assert!(crear_lote(&direccion, cantidad, total + 1), "IDs ausentes, duplicados o fuera de orden para lote {}", cantidad)
            total += cantidad
            dejar (codigo, contenido) = solicitar(&direccion, "GET", "/productos", "")
            assert_eq!(codigo, 200)
            dejar esperados = formato!("\"id\":").to_string()
            assert_eq!(contenido.matches(&esperados).count(), total as usize, "cantidad incorrecta de registros tras {} creaciones", total)
    dejar (codigo_obtener, _) = solicitar(&direccion, "GET", "/productos/1", "")
    assert_eq!(codigo_obtener, 200)
    dejar (codigo_actualizar, _) = solicitar(&direccion, "PUT", "/productos/1", "{\"nombre\":\"Actualizado\",\"precio\":9.5}")
    assert_eq!(codigo_actualizar, 200)
    dejar (codigo_eliminar, _) = solicitar(&direccion, "DELETE", "/productos/1", "")
    assert_eq!(codigo_eliminar, 204)
    dejar (codigo_ausente, _) = solicitar(&direccion, "GET", "/productos/1", "")
    assert_eq!(codigo_ausente, 404)
    imprimir!("Pruebas CRUD y concurrencia completadas: {} creaciones", total)
