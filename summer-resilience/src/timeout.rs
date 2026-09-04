use crate::config::TimeoutConfig;
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;
use thiserror::Error;

/// A validated timeout policy.
#[derive(Debug, Clone)]
pub struct TimeoutPolicy {
    timeout_duration: Duration,
}

impl TimeoutPolicy {
    /// Maximum duration allowed for the guarded operation.
    pub fn timeout_duration(&self) -> Duration {
        self.timeout_duration
    }
}

impl TryFrom<TimeoutConfig> for TimeoutPolicy {
    type Error = TimeoutConfigError;

    fn try_from(config: TimeoutConfig) -> Result<Self, Self::Error> {
        if config.timeout_duration == 0 {
            return Err(TimeoutConfigError::ZeroTimeoutDuration);
        }
        Ok(Self {
            timeout_duration: Duration::from_millis(config.timeout_duration),
        })
    }
}

/// Invalid timeout policy configuration.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TimeoutConfigError {
    #[error("timeout_duration must be at least 1 millisecond")]
    ZeroTimeoutDuration,
}

/// Error returned when an operation exceeds its configured deadline.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
#[error("timeout policy `{name}` elapsed after {timeout_duration_ms} milliseconds")]
pub struct TimeoutElapsed {
    name: String,
    timeout_duration_ms: u128,
}

impl TimeoutElapsed {
    /// Name of the timeout policy whose deadline elapsed.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Configured timeout duration.
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_millis(self.timeout_duration_ms as u64)
    }
}

/// Result error from the programmatic timeout API.
#[derive(Debug, Error)]
pub enum TimeoutError<E> {
    #[error(transparent)]
    Elapsed(#[from] TimeoutElapsed),
    #[error("guarded operation failed")]
    Operation(E),
}

/// Executes one asynchronous operation with a deadline.
///
/// When the deadline elapses, the operation future is dropped.
pub async fn execute<F, T, E>(
    name: &str,
    policy: TimeoutPolicy,
    operation: F,
) -> Result<T, TimeoutError<E>>
where
    F: Future<Output = Result<T, E>>,
{
    match tokio::time::timeout(policy.timeout_duration, operation).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(TimeoutError::Operation(error)),
        Err(_) => Err(TimeoutElapsed {
            name: name.to_owned(),
            timeout_duration_ms: policy.timeout_duration.as_millis(),
        }
        .into()),
    }
}

/// Registry of named timeout policies.
#[derive(Debug, Clone, Default)]
pub struct TimeoutRegistry {
    policies: HashMap<String, TimeoutPolicy>,
}

impl TimeoutRegistry {
    pub(crate) fn from_configs(
        configs: HashMap<String, TimeoutConfig>,
    ) -> Result<Self, NamedTimeoutConfigError> {
        let policies = configs
            .into_iter()
            .map(|(name, config)| {
                TimeoutPolicy::try_from(config)
                    .map(|policy| (name.clone(), policy))
                    .map_err(|source| NamedTimeoutConfigError { name, source })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { policies })
    }

    /// Gets a named timeout policy.
    pub fn get(&self, name: &str) -> Option<TimeoutPolicy> {
        self.policies.get(name).cloned()
    }
}

#[derive(Debug, Error)]
#[error("invalid timeout policy `{name}`: {source}")]
pub(crate) struct NamedTimeoutConfigError {
    name: String,
    #[source]
    source: TimeoutConfigError,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn returns_elapsed_when_the_deadline_is_exceeded() {
        let policy = TimeoutPolicy::try_from(TimeoutConfig {
            timeout_duration: 100,
        })
        .unwrap();

        let result = execute("slow", policy, async {
            tokio::time::sleep(Duration::from_millis(101)).await;
            Ok::<_, &str>(42)
        })
        .await;

        let TimeoutError::Elapsed(error) = result.unwrap_err() else {
            panic!("expected elapsed timeout")
        };
        assert_eq!(error.name(), "slow");
        assert_eq!(error.timeout_duration(), Duration::from_millis(100));
    }

    #[tokio::test]
    async fn preserves_the_operation_error() {
        let policy = TimeoutPolicy::try_from(TimeoutConfig::default()).unwrap();
        let result = execute("test", policy, async { Err::<(), _>("failed") }).await;
        assert!(matches!(result, Err(TimeoutError::Operation("failed"))));
    }

    #[test]
    fn rejects_a_zero_duration() {
        let result = TimeoutPolicy::try_from(TimeoutConfig {
            timeout_duration: 0,
        });
        assert_eq!(result.unwrap_err(), TimeoutConfigError::ZeroTimeoutDuration);
    }
}
