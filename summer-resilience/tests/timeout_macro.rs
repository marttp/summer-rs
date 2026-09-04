use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use summer::App;
use summer_resilience::{timeout, ResiliencePlugin, TimeoutElapsed};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
enum ServiceError {
    #[error(transparent)]
    TimedOut(#[from] TimeoutElapsed),
}

#[timeout(name = "backend")]
async fn slow_call(completed: Arc<AtomicBool>) -> Result<(), ServiceError> {
    tokio::time::sleep(Duration::from_millis(50)).await;
    completed.store(true, Ordering::SeqCst);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn macro_returns_a_typed_timeout_and_drops_the_operation() {
    App::new()
        .use_config_str(
            r#"
            [resilience.timeout.instances.backend]
            timeout_duration = 10
            "#,
        )
        .add_plugin(ResiliencePlugin)
        .build()
        .await
        .expect("app should build");

    let completed = Arc::new(AtomicBool::new(false));
    let error = slow_call(completed.clone()).await.unwrap_err();

    assert!(matches!(error, ServiceError::TimedOut(_)));
    assert!(!completed.load(Ordering::SeqCst));
}
