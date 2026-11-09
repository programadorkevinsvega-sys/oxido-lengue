sintaxis llaves
#[derive(Clone, Copy)]
pub struct BoundedContext {
    pub name: &'static str,
    pub schema: &'static str,
}
