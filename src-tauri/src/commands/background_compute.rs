use std::{
    fmt::Display,
    sync::{Arc, LazyLock},
};

use tokio::sync::Semaphore;

// Bound CPU-heavy IPC work independently of the serial actor and realtime path.
static POOL: LazyLock<ComputePool> = LazyLock::new(|| ComputePool::new(2));

pub(super) async fn run<T, E, F>(label: &'static str, work: F) -> Result<T, String>
where
    T: Send + 'static,
    E: Display + Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
{
    POOL.run(label, work).await
}

struct ComputePool {
    permits: Arc<Semaphore>,
}

impl ComputePool {
    fn new(capacity: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(capacity)),
        }
    }

    async fn run<T, E, F>(&self, label: &'static str, work: F) -> Result<T, String>
    where
        T: Send + 'static,
        E: Display + Send + 'static,
        F: FnOnce() -> Result<T, E> + Send + 'static,
    {
        let permit = self.permits.clone().try_acquire_owned().map_err(|_| {
            "Расчёт уже выполняется. Дождитесь результата и повторите действие.".to_owned()
        })?;
        tokio::task::spawn_blocking(move || {
            // Dropping the caller does not cancel spawn_blocking: hold admission
            // inside the worker until it actually exits, including unwinding.
            let _permit = permit;
            work()
        })
        .await
        .map_err(|error| format!("{label}: {error}"))?
        .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_excess_work_and_keeps_permit_after_caller_cancellation() {
        let pool = Arc::new(ComputePool::new(1));
        let (started, start) = tokio::sync::oneshot::channel();
        let (finish, wait) = std::sync::mpsc::channel();
        let worker_pool = pool.clone();
        let caller = tokio::spawn(async move {
            worker_pool
                .run("test", move || {
                    let _ = started.send(());
                    wait.recv().unwrap();
                    Ok::<_, String>(())
                })
                .await
        });
        start.await.unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        let rejected = pool
            .run("test", || -> Result<(), String> {
                panic!("must not start")
            })
            .await;
        assert!(rejected.unwrap_err().contains("Расчёт уже выполняется"));
        assert_eq!(pool.permits.available_permits(), 0);
        finish.send(()).unwrap();
        let permit =
            tokio::time::timeout(std::time::Duration::from_secs(5), pool.permits.acquire())
                .await
                .unwrap()
                .unwrap();
        drop(permit);
        assert_eq!(pool.run("test", || Ok::<_, String>(42)).await.unwrap(), 42);
    }

    #[tokio::test]
    async fn releases_admission_after_errors_and_panics() {
        let pool = ComputePool::new(1);
        assert_eq!(
            pool.run("test", || Err::<(), _>("invalid geometry"))
                .await
                .unwrap_err(),
            "invalid geometry"
        );
        let failure = pool
            .run("test", || -> Result<(), String> { panic!("worker panic") })
            .await
            .unwrap_err();
        assert!(failure.contains("test:"));
        assert_eq!(pool.run("test", || Ok::<_, String>(7)).await.unwrap(), 7);
    }
}
