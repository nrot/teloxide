use std::{
    fmt,
    future::Future,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};

use tokio::sync::Notify;

use crate::dispatching::update_listeners::UpdateListener;

/// A token which used to shutdown [`Dispatcher`].
#[derive(Clone)]
pub struct ShutdownToken {
    dispatcher_state: Arc<DispatcherState>,
    shutdown_notify_back: Arc<Notify>,
}

/// This error is returned from [`ShutdownToken::shutdown`] when trying to
/// shutdown an idle [`Dispatcher`].
#[derive(Debug)]
pub struct IdleShutdownError;

impl ShutdownToken {
    /// Tries to shutdown dispatching.
    ///
    /// Returns an error if the dispatcher is idle at the moment.
    ///
    /// If you don't need to wait for shutdown, the returned future can be
    /// ignored.
    pub fn shutdown(&self) -> Result<impl Future<Output = ()> + '_, IdleShutdownError> {
        match shutdown_inner(&self.dispatcher_state) {
            Ok(()) | Err(Ok(AlreadyShuttingDown)) => Ok(async move {
                log::info!("Trying to shutdown the dispatcher...");
                self.shutdown_notify_back.notified().await
            }),
            Err(Err(err)) => Err(err),
        }
    }

    pub(crate) fn new() -> Self {
        Self {
            dispatcher_state: Arc::new(DispatcherState {
                inner: AtomicU8::new(ShutdownState::Idle as _),
            }),
            shutdown_notify_back: <_>::default(),
        }
    }

    pub(crate) fn start_dispatching(&self) {
        if let Err(actual) =
            self.dispatcher_state.compare_exchange(ShutdownState::Idle, ShutdownState::Running)
        {
            panic!(
                "Dispatching is already running: expected `{:?}` state, found `{:?}`",
                ShutdownState::Idle,
                actual
            );
        }
    }

    pub(crate) fn is_shutting_down(&self) -> bool {
        matches!(self.dispatcher_state.load(), ShutdownState::ShuttingDown)
    }

    pub(crate) fn done(&self) {
        if self.is_shutting_down() {
            // Stopped because of a `shutdown` call.

            // Notify `shutdown`s that we finished
            self.shutdown_notify_back.notify_waiters();
            log::info!("Dispatching has been shut down.");
        } else {
            log::info!("Dispatching has been stopped (listener returned `None`).");
        }

        self.dispatcher_state.store(ShutdownState::Idle);
    }
}

impl fmt::Display for IdleShutdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dispatcher was idle and as such couldn't be shut down")
    }
}

impl std::error::Error for IdleShutdownError {}

pub(crate) fn shutdown_check_timeout_for<E>(update_listener: &impl UpdateListener<E>) -> Duration {
    const MIN_SHUTDOWN_CHECK_TIMEOUT: Duration = Duration::from_secs(1);
    const DZERO: Duration = Duration::ZERO;

    let shutdown_check_timeout = update_listener.timeout_hint().unwrap_or(DZERO);
    shutdown_check_timeout.saturating_add(MIN_SHUTDOWN_CHECK_TIMEOUT)
}

struct DispatcherState {
    inner: AtomicU8,
}

impl DispatcherState {
    // Ordering::Relaxed: only one atomic variable, nothing to synchronize.

    fn load(&self) -> ShutdownState {
        ShutdownState::from_u8(self.inner.load(Ordering::Relaxed))
    }

    fn store(&self, new: ShutdownState) {
        self.inner.store(new as _, Ordering::Relaxed)
    }

    fn compare_exchange(
        &self,
        current: ShutdownState,
        new: ShutdownState,
    ) -> Result<ShutdownState, ShutdownState> {
        self.inner
            .compare_exchange(current as _, new as _, Ordering::Relaxed, Ordering::Relaxed)
            .map(ShutdownState::from_u8)
            .map_err(ShutdownState::from_u8)
    }
}

#[repr(u8)]
#[derive(Debug)]
enum ShutdownState {
    Running,
    ShuttingDown,
    Idle,
}

impl ShutdownState {
    fn from_u8(n: u8) -> Self {
        const RUNNING: u8 = ShutdownState::Running as u8;
        const SHUTTING_DOWN: u8 = ShutdownState::ShuttingDown as u8;
        const IDLE: u8 = ShutdownState::Idle as u8;

        match n {
            RUNNING => ShutdownState::Running,
            SHUTTING_DOWN => ShutdownState::ShuttingDown,
            IDLE => ShutdownState::Idle,
            _ => unreachable!(),
        }
    }
}

struct AlreadyShuttingDown;

fn shutdown_inner(
    state: &DispatcherState,
) -> Result<(), Result<AlreadyShuttingDown, IdleShutdownError>> {
    use ShutdownState::*;

    let res = state.compare_exchange(Running, ShuttingDown);

    match res {
        Ok(_) => Ok(()),
        Err(ShuttingDown) => Err(Ok(AlreadyShuttingDown)),
        Err(Idle) => Err(Err(IdleShutdownError)),
        Err(Running) => unreachable!(),
    }
}
