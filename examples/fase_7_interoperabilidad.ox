sintaxis llaves
use std::collections::VecDeque;

mod calculo { pub fn suma(a: i32, b: i32) -> i32 { a + b } }
struct Punto { x: i32, y: i32 }
enum Estado { Listo(i32) }
impl Punto { fn distancia(&self) -> i32 { self.x + self.y } }

fn opcion() -> Result<Option<i32>, &'static str> { Ok(Some(3)) }

fn main() -> Result<(), &'static str> {
    let punto = Punto { x: 2, y: 3 };
    let par = ([1, 2], (punto.x, punto.y));
    let ref_valor = &par.1.0;
    let mut cola = VecDeque::from([1, 2]);
    cola.push_back(3);
    let estado = Estado::Listo(punto.distancia());
    println!("{}:{}:{}:{}", calculo::suma(*ref_valor, par.1.1), cola.pop_front().unwrap(), opcion()?.ok_or("falta")?, match estado { Estado::Listo(valor) => valor });
    Ok(())
}
