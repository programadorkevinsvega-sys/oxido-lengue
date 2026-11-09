sintaxis llaves
pub mod configuration;
pub mod dependencies;
pub mod grpc_telemetry;
pub mod observability;
pub mod outbox;
pub mod propagation;
pub mod shutdown;

pub mod grpc {
    tonic::include_proto!("xovyra.runtime.v1");
}
