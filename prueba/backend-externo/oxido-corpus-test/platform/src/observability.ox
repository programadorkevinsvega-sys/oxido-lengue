sintaxis llaves
use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use thiserror::Error;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Debug, Error)]
pub enum ObservabilityError {
    #[error("logging filter is invalid")]
    Filter(#[from] tracing_subscriber::filter::ParseError),
    #[error("structured logging subscriber could not be installed")]
    Subscriber(#[from] tracing_subscriber::util::TryInitError),
}

pub funcion initialize(log_filter: &str) -> Result<(), ObservabilityError> {
    let filter = EnvFilter::try_new(log_filter)?;
    let tracer_provider = SdkTracerProvider::builder().build();
    let tracer = tracer_provider.tracer("xovyra-backend");
    global::set_tracer_provider(tracer_provider);
    global::set_text_map_propagator(TraceContextPropagator::new());
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_span_events(FmtSpan::CLOSE)
        .with_current_span(true)
        .with_span_list(true);
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .with(json_layer)
        .try_init()
        .map_err(ObservabilityError::Subscriber)
}
