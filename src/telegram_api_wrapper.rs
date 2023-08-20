extern crate async_trait;

use serde::{Deserialize, Serialize};
use teloxide::{prelude::*, update_listeners::AsUpdateStream};
use tokio_old::stream::StreamExt;

pub enum TelegramError {
	GeneralError(String),
}

type TlgResult<T> = Result<T, TelegramError>;

#[derive(PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct ChatId(pub i64);

impl From<teloxide::prelude::ChatId> for ChatId {
	fn from(chat: teloxide::prelude::ChatId) -> ChatId {
		ChatId(chat.0)
	}
}

impl From<telegram_bot::types::UserId> for ChatId {
	fn from(chat: telegram_bot::types::UserId) -> ChatId {
		return ChatId(chat.into());
	}
}

#[derive(PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct UserId(pub i64);

impl From<&teloxide::types::UserId> for UserId {
	fn from(user: &teloxide::types::UserId) -> UserId {
		return UserId(user.0 as i64);
	}
}

#[derive(PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct MessageId(pub i64);

pub struct SentMessage {
	chat_id: ChatId,
	user_id: UserId,
	pub message_id: MessageId,
}

pub struct BotMessage {
	user_id: UserId,
	msg: String,
}

#[async_trait::async_trait]
pub trait Api {
	async fn send_message<M: ToString + Send>(
		&mut self,
		chat_id: ChatId,
		msg: M,
	) -> TlgResult<SentMessage>;

	async fn delete_message(&mut self, chat_id: ChatId, msg_id: MessageId) -> TlgResult<()>;

	async fn get_message(&mut self) -> Option<BotMessage>;

	fn get_msg_stream(&self) -> teloxide::update_listeners::Polling<Bot>;
}

pub fn create_api(token: &str) -> impl Api {
	ApiWrapper::create(token)
}

struct ApiWrapper {
	api: teloxide::Bot,
	pub stream: teloxide::update_listeners::Polling<teloxide::Bot>,
	tokio_runtime: tokio_new::runtime::Runtime,
}

impl ApiWrapper {
	fn create(token: &str) -> Self {
		let api = teloxide::Bot::new(token);

		let api_for_stream = api.clone();
		let updates_stream = teloxide::update_listeners::polling_default(api_for_stream);

		let runtime = tokio_new::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.unwrap();

		let stream = runtime.block_on(async { updates_stream.await });

		Self {
			api,
			stream,
			tokio_runtime: runtime,
		}
	}
}

// TODO: trait From doesn't work here
fn tmp_convert(user: &teloxide::types::User) -> UserId {
	UserId(user.id.0 as i64)
}

#[async_trait::async_trait]
impl Api for ApiWrapper {
	async fn send_message<M: ToString + Send>(
		&mut self,
		chat_id: ChatId,
		msg: M,
	) -> TlgResult<SentMessage> {
		let msg_str = msg.to_string();
		let chat_id = teloxide::types::Recipient::Id(teloxide::prelude::ChatId(chat_id.0));
		let req = self.api.send_message(chat_id, msg_str);
		self.tokio_runtime.block_on(async {
			let res = req.send().await;
			match res {
				Ok(msg) => Ok(SentMessage {
					chat_id: ChatId(msg.chat.id.0),
					user_id: UserId(msg.chat.id.0),
					message_id: MessageId(msg.id.0 as i64),
				}),
				Err(_) => Err(TelegramError::GeneralError("Error string".to_owned())),
			}
		})
	}

	async fn delete_message(&mut self, chat_id: ChatId, msg_id: MessageId) -> TlgResult<()> {
		let req = self.api.delete_message(
			teloxide::prelude::ChatId(chat_id.0),
			teloxide::types::MessageId(msg_id.0.try_into().unwrap()),
		);

		self.tokio_runtime.block_on(async {
			let res = req.send().await;
			match res {
				Ok(_) => Ok(()),
				Err(_) => Err(TelegramError::GeneralError("Error string".to_owned())),
			}
		})
	}

	async fn get_message(&mut self) -> Option<BotMessage> {
		let stream = self.stream.as_stream().next().await;
		if let Some(stream) = stream {
			if let Ok(upd) = stream {
				let user = upd.user();
				let msg = match &upd.kind {
					teloxide::types::UpdateKind::Message(msg) => msg.text().map(|x| x.to_string()),

					_ => None,
				};

				return match (user, msg) {
					(Some(user), Some(msg)) => Some(BotMessage {
						msg,
						user_id: tmp_convert(user),
					}),
					_ => None,
				};
			}
		}

		None
	}

	fn get_msg_stream(&self) -> teloxide::update_listeners::Polling<Bot> {
		let updates_stream = teloxide::update_listeners::polling_default(self.api.clone());

		let stream = self.tokio_runtime.block_on(async { updates_stream.await });
		stream
	}
}
