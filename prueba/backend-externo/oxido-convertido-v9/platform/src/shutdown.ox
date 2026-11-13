sintaxis llaves
pub async funcion wait_for_termination() {
    #[cfg(unix)]
    wait_for_unix_termination().await;
    #[cfg(not(unix))]
    wait_for_interrupt().await;
}

#[cfg(unix)]
async funcion wait_for_unix_termination() {
    let mut termination =
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(termination) => termination,
            Err(error) => {
                tracing::error!(error = %error, "Cannot register SIGTERM handler");
                wait_for_interrupt().await;
                return;
            }
        };
    tokio::select! {
        interrupt = tokio::signal::ctrl_c() => match interrupt {
            Ok(()) => tracing::info!(signal = "SIGINT", "Graceful shutdown requested"),
            Err(error) => tracing::error!(error = %error, "Cannot receive SIGINT"),
        },
        termination_event = termination.recv() => match termination_event {
            Some(()) => tracing::info!(signal = "SIGTERM", "Graceful shutdown requested"),
            None => tracing::error!("SIGTERM listener closed before receiving a signal"),
        },
    }
}

async funcion wait_for_interrupt() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => tracing::info!(signal = "SIGINT", "Graceful shutdown requested"),
        Err(error) => tracing::error!(error = %error, "Cannot receive SIGINT"),
    }
}
