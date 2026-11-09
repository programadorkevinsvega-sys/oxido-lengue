sintaxis llaves
use std::collections::HashMap;

use opentelemetry::Context;
use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};

pub struct TraceCarrier {
    values: HashMap<String, String>,
}

impl TraceCarrier {
    pub fn from_values(values: HashMap<String, String>) -> Self {
        Self { values }
    }

    pub fn into_values(self) -> HashMap<String, String> {
        self.values
    }
}

impl Extractor for TraceCarrier {
    fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.values.keys().map(String::as_str).collect()
    }
}

impl Injector for TraceCarrier {
    fn set(&mut self, key: &str, value: String) {
        self.values.insert(key.to_ascii_lowercase(), value);
    }
}

pub funcion extract_trace_context(values: HashMap<String, String>) -> Context {
    let carrier = TraceCarrier::from_values(values);
    global::get_text_map_propagator(|propagator| propagator.extract(&carrier))
}

pub funcion inject_trace_context(context: &Context) -> HashMap<String, String> {
    let mut carrier = TraceCarrier::from_values(HashMap::new());
    global::get_text_map_propagator(|propagator| propagator.inject_context(context, &mut carrier));
    carrier.into_values()
}
