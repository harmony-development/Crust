#![allow(clippy::field_reassign_with_default)]

pub mod channel;
pub mod content;
pub mod error;
pub mod guild;
pub mod member;
pub mod message;

use channel::Channel;
use guild::Guild;
pub use harmony_rust_sdk::{
    api::exports::http::Uri,
    client::{api::auth::Session as InnerSession, AuthStatus, Client as InnerClient},
};
use harmony_rust_sdk::{
    api::{chat::event::Event, harmonytypes::Message as HarmonyMessage},
    client::api::{chat::EventSource, rest::FileId},
};

use content::ContentStore;
use error::{ClientError, ClientResult};
use member::Member;
use message::{harmony_messages_to_ui_messages, MessageId};
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Debug, Formatter},
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

use self::{guild::Guilds, message::Message};

/// A sesssion struct with our requirements (unlike the `InnerSession` type)
#[derive(Clone, Deserialize, Serialize)]
pub struct Session {
    pub session_token: String,
    pub user_id: u64,
    pub homeserver: String,
}

impl Debug for Session {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("user_id", &self.user_id.to_string())
            .field("homeserver", &self.homeserver)
            .finish()
    }
}

impl Into<InnerSession> for Session {
    fn into(self) -> InnerSession {
        InnerSession {
            user_id: self.user_id,
            session_token: self.session_token,
        }
    }
}

pub struct Client {
    inner: InnerClient,
    pub guilds: Guilds,
    pub user_id: Option<u64>,
    pub should_subscribe_to_events: AtomicBool,
    content_store: Arc<ContentStore>,
}

impl Debug for Client {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Client")
            .field(
                "user_id",
                &format!(
                    "{:?}",
                    self.auth_status().session().map_or(0, |s| s.user_id)
                ),
            )
            .field("session_file", &self.content_store.session_file())
            .finish()
    }
}

impl Client {
    pub async fn new(
        homeserver_url: Uri,
        session: Option<InnerSession>,
        content_store: Arc<ContentStore>,
    ) -> ClientResult<Self> {
        Ok(Self {
            guilds: Guilds::new(),
            user_id: session.as_ref().map(|s| s.user_id),
            should_subscribe_to_events: AtomicBool::new(false),
            content_store,
            inner: InnerClient::new(homeserver_url, session).await?,
        })
    }

    pub async fn logout(_inner: InnerClient, session_file: PathBuf) -> ClientResult<()> {
        tokio::fs::remove_file(session_file).await?;
        Ok(())
    }

    pub fn content_store(&self) -> &ContentStore {
        &self.content_store
    }

    pub fn content_store_arc(&self) -> Arc<ContentStore> {
        self.content_store.clone()
    }

    pub fn auth_status(&self) -> AuthStatus {
        self.inner.auth_status()
    }

    pub fn inner(&self) -> &InnerClient {
        &self.inner
    }

    pub fn get_guild(&mut self, guild_id: u64) -> Option<&mut Guild> {
        self.guilds.get_mut(&guild_id)
    }

    pub fn get_channel(&mut self, guild_id: u64, channel_id: u64) -> Option<&mut Channel> {
        self.get_guild(guild_id)
            .map(|guild| guild.channels.get_mut(&channel_id))
            .flatten()
    }

    pub fn get_member(&mut self, guild_id: u64, user_id: u64) -> Option<&mut Member> {
        self.get_guild(guild_id)
            .map(|guild| guild.members.get_member_mut(&user_id))
            .flatten()
    }

    pub fn process_event(&mut self, event: Event) -> Vec<FileId> {
        match event {
            Event::SentMessage(message_sent) => {
                let echo_id = message_sent.echo_id;

                if let Some(message) = message_sent.message {
                    let guild_id = message.guild_id;
                    let channel_id = message.channel_id;

                    if let Some(channel) = self.get_channel(guild_id, channel_id) {
                        let message = Message::from(message);
                        if let Some(msg) = channel
                            .messages
                            .iter_mut()
                            .find(|message| message.id == MessageId::Unack(echo_id))
                        {
                            *msg = message;
                        } else {
                            channel.messages.push(message);
                        }
                    }
                }
            }
            Event::DeletedMessage(message_deleted) => {
                let guild_id = message_deleted.guild_id;
                let channel_id = message_deleted.channel_id;
                let message_id = message_deleted.message_id;

                if let Some(channel) = self.get_channel(guild_id, channel_id) {
                    if let Some(pos) = channel
                        .messages
                        .iter()
                        .position(|msg| msg.id == MessageId::Ack(message_id))
                    {
                        channel.messages.remove(pos);
                    }
                }
            }
            Event::EditedMessage(message_updated) => {
                let guild_id = message_updated.guild_id;
                let channel_id = message_updated.channel_id;

                if let Some(channel) = self.get_channel(guild_id, channel_id) {
                    if let Some(msg) = channel
                        .messages
                        .iter_mut()
                        .find(|message| message.id == MessageId::Ack(message_updated.message_id))
                    {
                        if message_updated.update_content {
                            msg.content = message_updated.content;
                        }
                    }
                }
            }
            Event::DeletedChannel(channel_deleted) => {
                let guild_id = channel_deleted.guild_id;
                let channel_id = channel_deleted.channel_id;

                if let Some(guild) = self.get_guild(guild_id) {
                    guild.channels.remove(&channel_id);
                }
            }
            Event::EditedChannel(channel_updated) => {
                let guild_id = channel_updated.guild_id;
                let channel_id = channel_updated.channel_id;

                if let Some(channel) = self.get_channel(guild_id, channel_id) {
                    if channel_updated.update_name {
                        channel.name = channel_updated.name;
                    }
                }
            }
            Event::CreatedChannel(channel_created) => {
                let guild_id = channel_created.guild_id;
                let channel_id = channel_created.channel_id;

                if let Some(guild) = self.get_guild(guild_id) {
                    guild.channels.insert(
                        channel_id,
                        Channel {
                            is_category: channel_created.is_category,
                            name: channel_created.name,
                            loading_messages_history: false,
                            looking_at_message: 0,
                            messages: Vec::new(),
                        },
                    );
                }
            }
            Event::Typing(typing) => {
                let guild_id = typing.guild_id;
                let channel_id = typing.channel_id;
                let user_id = typing.user_id;

                if let Some(member) = self.get_member(guild_id, user_id) {
                    member.typing_in_channel = Some(channel_id);
                }
            }
            x => todo!("implement {:?}", x),
        }

        Vec::new()
    }

    pub fn process_get_message_history_response(
        &mut self,
        guild_id: u64,
        channel_id: u64,
        messages: Vec<HarmonyMessage>,
        _reached_top: bool,
    ) -> Vec<FileId> {
        let mut messages = harmony_messages_to_ui_messages(messages);

        if let Some(channel) = self.get_channel(guild_id, channel_id) {
            messages.append(&mut channel.messages);
            channel.messages = messages;
        }

        Vec::new()
    }

    pub fn subscribe_to(&self) -> Vec<EventSource> {
        self.guilds
            .keys()
            .map(|guild_id| EventSource::Guild(*guild_id))
            .collect()
    }
}
