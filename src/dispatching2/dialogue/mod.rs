//! Support for user dialogues.
//!
//! The main type is (surprise!) [`Dialogue`]. Under the hood, it is just a
//! wrapper over [`Storage`] and a chat ID. All it does is provides convenient
//! method for manipulating the dialogue state. [`Storage`] is where all
//! dialogue states are stored; it can be either [`InMemStorage`], which is a
//! simple hash map, or database wrappers such as [`SqliteStorage`]. In the
//! latter case, your dialogues are _persistent_, meaning that you can safely
//! restart your bot and all dialogues will remain in the database -- this is a
//! preferred method for production bots.
//!
//! [`examples/dialogue.rs`] clearly demonstrates the typical usage of
//! dialogues. Your dialogue state can be represented as an enumeration:
//!
//! ```ignore
//! #[derive(DialogueState, Clone)]
//! #[handler_out(anyhow::Result<()>)]
//! pub enum State {
//!     #[handler(handle_start)]
//!     Start,
//!
//!     #[handler(handle_receive_full_name)]
//!     ReceiveFullName,
//!
//!     #[handler(handle_receive_age)]
//!     ReceiveAge { full_name: String },
//!
//!     #[handler(handle_receive_location)]
//!     ReceiveLocation { full_name: String, age: u8 },
//! }
//! ```
//!
//! Each state is associated with its respective handler: e.g., when a dialogue
//! state is `ReceiveAge`, `handle_receive_age` is invoked:
//!
//! ```ignore
//! async fn handle_receive_age(
//!     bot: AutoSend<Bot>,
//!     msg: Message,
//!     dialogue: MyDialogue,
//!     (full_name,): (String,), // Available from `State::ReceiveAge`.
//! ) -> anyhow::Result<()> {
//!     match msg.text().map(|text| text.parse::<u8>()) {
//!         Some(Ok(age)) => {
//!             bot.send_message(msg.chat.id, "What's your location?").await?;
//!             dialogue.update(State::ReceiveLocation { full_name, age }).await?;
//!         }
//!         _ => {
//!             bot.send_message(msg.chat.id, "Send me a number.").await?;
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! Variant's fields are passed to state handlers as tuples: `(full_name,):
//! (String,)`. Using [`Dialogue::update`], you can update the dialogue with a
//! new state, in our case -- `State::ReceiveLocation { full_name, age }`. To
//! exit the dialogue, just call [`Dialogue::exit`] and it will be removed from
//! the inner storage:
//!
//! ```ignore
//! async fn handle_receive_location(
//!     bot: AutoSend<Bot>,
//!     msg: Message,
//!     dialogue: MyDialogue,
//!     (full_name, age): (String, u8), // Available from `State::ReceiveLocation`.
//! ) -> anyhow::Result<()> {
//!     match msg.text() {
//!         Some(location) => {
//!             let message =
//!                 format!("Full name: {}\nAge: {}\nLocation: {}", full_name, age, location);
//!             bot.send_message(msg.chat.id, message).await?;
//!             dialogue.exit().await?;
//!         }
//!         None => {
//!             bot.send_message(msg.chat.id, "Send me a text message.").await?;
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! [`examples/dialogue.rs`]: https://github.com/teloxide/teloxide/blob/master/examples/dialogue.rs

#[cfg(feature = "redis-storage")]
#[cfg_attr(all(docsrs, feature = "nightly"), doc(cfg(feature = "redis-storage")))]
pub use crate::dispatching::dialogue::{RedisStorage, RedisStorageError};

#[cfg(feature = "sqlite-storage")]
pub use crate::dispatching::dialogue::{SqliteStorage, SqliteStorageError};

pub use crate::dispatching::dialogue::{
    serializer, InMemStorage, InMemStorageError, Serializer, Storage, TraceStorage,
};
pub use get_chat_id::GetChatId;

use std::{marker::PhantomData, sync::Arc};

mod get_chat_id;

/// A handle for controlling dialogue state.
#[derive(Debug)]
pub struct Dialogue<D, S> {
    storage: Arc<S>,
    chat_id: i64,
    _phantom: PhantomData<D>,
}

// `#[derive]` requires generics to implement `Clone`, but `S` is wrapped around
// `Arc`, and `D` is wrapped around PhantomData.
impl<D, S> Clone for Dialogue<D, S> {
    fn clone(&self) -> Self {
        Dialogue { storage: self.storage.clone(), chat_id: self.chat_id, _phantom: PhantomData }
    }
}

impl<D, S> Dialogue<D, S>
where
    D: Send + 'static,
    S: Storage<D>,
{
    /// Constructs a new dialogue with `storage` (where dialogues are stored)
    /// and `chat_id` of a current dialogue.
    pub fn new(storage: Arc<S>, chat_id: i64) -> Self {
        Self { storage, chat_id, _phantom: PhantomData }
    }

    /// Retrieves the current state of the dialogue or `None` if there is no
    /// dialogue.
    pub async fn get(&self) -> Result<Option<D>, S::Error> {
        self.storage.clone().get_dialogue(self.chat_id).await
    }

    /// Like [`Dialogue::get`] but returns a default value if there is no
    /// dialogue.
    pub async fn get_or_default(&self) -> Result<D, S::Error>
    where
        D: Default,
    {
        match self.get().await? {
            Some(d) => Ok(d),
            None => {
                self.storage.clone().update_dialogue(self.chat_id, D::default()).await?;
                Ok(D::default())
            }
        }
    }

    /// Updates the dialogue state.
    ///
    /// The dialogue type `D` must implement `From<State>` to allow implicit
    /// conversion from `State` to `D`.
    pub async fn update<State>(&self, state: State) -> Result<(), S::Error>
    where
        D: From<State>,
    {
        let new_dialogue = state.into();
        self.storage.clone().update_dialogue(self.chat_id, new_dialogue).await?;
        Ok(())
    }

    /// Updates the dialogue with a default value.
    pub async fn reset(&self) -> Result<(), S::Error>
    where
        D: Default,
    {
        self.update(D::default()).await
    }

    /// Removes the dialogue from the storage provided to [`Dialogue::new`].
    pub async fn exit(&self) -> Result<(), S::Error> {
        self.storage.clone().remove_dialogue(self.chat_id).await
    }
}
