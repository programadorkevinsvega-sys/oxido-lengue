use std::time::{Duration, SystemTime};
use tokio::sync::watch;
use xovyra_jobs::scratch::ScratchSpace;
pub async fn run(scratch: ScratchSpace, mut shutdown: watch::Receiver<bool>) {
    let mut cleanup_interval = tokio::time::interval(Duration::from_secs(600));
    loop {
        tokio::select! {
            _ = cleanup_interval.tick() => match scratch.clean_expired(SystemTime::now())
            { Ok(removed) if removed > 0 => tracing::info!(removed,
            "Expired job scratch directories were removed"), Ok(_) => {} Err(error) =>
            tracing::error!(error_type =% std::any::type_name_of_val(& error),
            "Scratch retention pass failed and will retry"), }, result = shutdown
            .changed() => match result { Ok(()) if * shutdown.borrow() => return, Ok(())
            => {} Err(error) => { tracing::warn!(error =% error,
            "Scratch cleanup shutdown channel closed"); return; } }
        }
    }
}
