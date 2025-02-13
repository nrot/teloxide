use crate::{
    dispatching::{update_listeners, update_listeners::UpdateListener},
    dispatching2::UpdateFilterExt,
    error_handlers::{LoggingErrorHandler, OnError},
    types::Update,
};
use dptree::di::{DependencyMap, Injectable};
use std::fmt::Debug;
use teloxide_core::requests::Requester;

/// A [REPL] for messages.
///
/// All errors from an update listener and a handler will be logged.
///
/// # Caution
/// **DO NOT** use this function together with [`Dispatcher`] and other REPLs,
/// because Telegram disallow multiple requests at the same time from the same
/// bot.
///
/// [REPL]: https://en.wikipedia.org/wiki/Read-eval-print_loop
/// [`Dispatcher`]: crate::dispatching::Dispatcher
#[cfg(feature = "ctrlc_handler")]
pub async fn repl<R, H, E, Args>(bot: R, handler: H)
where
    H: Injectable<DependencyMap, Result<(), E>, Args> + Send + Sync + 'static,
    Result<(), E>: OnError<E>,
    E: Debug + Send + Sync + 'static,
    R: Requester + Send + Sync + Clone + 'static,
    <R as Requester>::GetUpdates: Send,
{
    let cloned_bot = bot.clone();
    repl_with_listener(bot, handler, update_listeners::polling_default(cloned_bot).await).await;
}

/// Like [`repl`], but with a custom [`UpdateListener`].
///
/// All errors from an update listener and handler will be logged.
///
/// # Caution
/// **DO NOT** use this function together with [`Dispatcher`] and other REPLs,
/// because Telegram disallow multiple requests at the same time from the same
/// bot.
///
/// [`Dispatcher`]: crate::dispatching::Dispatcher
/// [`repl`]: crate::dispatching::repls::repl()
/// [`UpdateListener`]: crate::dispatching::update_listeners::UpdateListener
#[cfg(feature = "ctrlc_handler")]
pub async fn repl_with_listener<'a, R, H, E, L, ListenerE, Args>(bot: R, handler: H, listener: L)
where
    H: Injectable<DependencyMap, Result<(), E>, Args> + Send + Sync + 'static,
    L: UpdateListener<ListenerE> + Send + 'a,
    ListenerE: Debug,
    Result<(), E>: OnError<E>,
    E: Debug + Send + Sync + 'static,
    R: Requester + Clone + Send + Sync + 'static,
{
    use crate::dispatching2::Dispatcher;

    #[allow(unused_mut)]
    let mut dispatcher =
        Dispatcher::builder(bot, Update::filter_message().branch(dptree::endpoint(handler)))
            .build();

    #[cfg(feature = "ctrlc_handler")]
    dispatcher.setup_ctrlc_handler();

    dispatcher
        .dispatch_with_listener(
            listener,
            LoggingErrorHandler::with_custom_text("An error from the update listener"),
        )
        .await;
}
