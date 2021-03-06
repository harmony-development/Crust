use harmony_rust_sdk::client::api::chat::channel;

use crate::{
    client::{error::ClientError, Client},
    label, label_button, length, space,
    ui::{
        component::*,
        style::{Theme, ERROR_COLOR, PADDING, SUCCESS_COLOR},
    },
};

#[derive(Debug, Clone)]
pub enum ChannelState {
    Created {
        guild_id: u64,
        channel_id: u64,
        name: String,
    },
    Creating {
        name: String,
    },
    None,
}

impl Default for ChannelState {
    fn default() -> Self {
        ChannelState::None
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    ChannelNameChanged(String),
    CreateChannel,
    CreatedChannel { guild_id: u64, channel_id: u64 },
    GoBack,
}

#[derive(Default, Debug)]
pub struct ChannelCreationModal {
    create_channel_back_but_state: button::State,
    channel_name_textedit_state: text_input::State,
    channel_create_but_state: button::State,
    channel_creation_state: ChannelState,
    channel_name_field: String,
    error_text: String,
}

impl ChannelCreationModal {
    pub fn view(&mut self, theme: Theme) -> Element<Message> {
        let mut create_text_edit = TextInput::new(
            &mut self.channel_name_textedit_state,
            "Enter a channel name...",
            &self.channel_name_field,
            Message::ChannelNameChanged,
        )
        .padding(PADDING / 2)
        .width(length!(= 400))
        .style(theme);

        let mut create = label_button!(&mut self.channel_create_but_state, "Create").style(theme);
        let mut back = label_button!(&mut self.create_channel_back_but_state, "Back").style(theme);

        if let ChannelState::None | ChannelState::Created { .. } = &self.channel_creation_state {
            back = back.on_press(Message::GoBack);

            if !self.channel_name_field.is_empty() {
                create_text_edit = create_text_edit.on_submit(Message::CreateChannel);
                create = create.on_press(Message::CreateChannel);
            }
        }

        let mut create_widgets = Vec::with_capacity(3);
        match &self.channel_creation_state {
            ChannelState::Created { name, .. } => {
                create_widgets.push(
                    label!("Successfully created channel {}", name)
                        .color(SUCCESS_COLOR)
                        .into(),
                );
            }
            ChannelState::Creating { name } => {
                create_widgets.push(label!("Creating channel {}", name).into())
            }
            _ => {}
        }

        if !self.error_text.is_empty() {
            create_widgets.push(label!(&self.error_text).color(ERROR_COLOR).into());
        }

        create_widgets.push(create_text_edit.into());
        create_widgets.push(
            row(vec![
                create.width(length!(= 80)).into(),
                space!(w = 200).into(),
                back.width(length!(= 80)).into(),
            ])
            .into(),
        );

        Container::new(column(create_widgets))
            .style(theme.round())
            .center_x()
            .center_y()
            .into()
    }

    pub fn update(
        &mut self,
        msg: Message,
        guild_id: u64,
        client: &Client,
    ) -> (Command<super::super::Message>, bool) {
        let mut go_back = false;
        match msg {
            super::create_channel::Message::ChannelNameChanged(new_name) => {
                self.channel_name_field = new_name;
            }
            super::create_channel::Message::CreateChannel => {
                let channel_name = self.channel_name_field.clone();

                self.error_text.clear();
                self.channel_creation_state = ChannelState::Creating {
                    name: channel_name.clone(),
                };
                let inner = client.inner().clone();

                return (
                    Command::perform(
                        async move {
                            let result = channel::create_channel(
                                &inner,
                                channel::CreateChannel::new(
                                    guild_id,
                                    channel_name,
                                    harmony_rust_sdk::api::chat::Place::Top { before: 0 },
                                ),
                            )
                            .await;
                            result.map_or_else(
                                |e| super::super::Message::Error(Box::new(e.into())),
                                |response| {
                                    super::super::Message::MainScreen(
                                        super::Message::ChannelCreationMessage(
                                            Message::CreatedChannel {
                                                guild_id,
                                                channel_id: response.channel_id,
                                            },
                                        ),
                                    )
                                },
                            )
                        },
                        |msg| msg,
                    ),
                    go_back,
                );
            }
            super::create_channel::Message::CreatedChannel {
                guild_id,
                channel_id,
            } => {
                self.channel_creation_state = ChannelState::Created {
                    guild_id,
                    channel_id,
                    name: self.channel_name_field.clone(),
                };
                self.channel_name_field.clear();
            }
            super::create_channel::Message::GoBack => {
                self.channel_creation_state = ChannelState::None;
                self.channel_name_field.clear();
                self.error_text.clear();
                go_back = true;
            }
        }

        (Command::none(), go_back)
    }

    pub fn on_error(&mut self, error: &ClientError) -> Command<super::super::Message> {
        self.error_text = error.to_string();
        self.channel_creation_state = ChannelState::None;

        Command::none()
    }
}
