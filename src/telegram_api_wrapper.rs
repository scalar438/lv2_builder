extern crate async_trait;

use serde::{Deserialize, Serialize};
use teloxide::prelude::*;

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

#[derive(PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct MessageId(pub i64);

pub struct ReseivedMessage {
	chat_id: ChatId,
	user_id: UserId,
	pub message_id: MessageId,
}

#[async_trait::async_trait]
pub trait Api {
	async fn send_message<M: ToString + Send>(
		&mut self,
		chat_id: ChatId,
		msg: M,
	) -> TlgResult<ReseivedMessage>;

	async fn delete_message(&mut self, chat_id: ChatId, msg_id: MessageId) -> TlgResult<()>;
}

pub fn create_api(token: &str) -> impl Api {
	ApiWrapper::create(token)
}

struct ApiWrapper {
	api: teloxide::Bot,
	tokio_runtime: tokio_new::runtime::Runtime,
}

impl ApiWrapper {
	fn create(token: &str) -> Self {
		let api = teloxide::Bot::new(token);
		let runtime = tokio_new::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.unwrap();
		Self {
			api,
			tokio_runtime: runtime,
		}
	}
}

#[async_trait::async_trait]
impl Api for ApiWrapper {
	async fn send_message<M: ToString + Send>(
		&mut self,
		chat_id: ChatId,
		msg: M,
	) -> TlgResult<ReseivedMessage> {
		let msg_str = msg.to_string();
		let chat_id = teloxide::types::Recipient::Id(teloxide::prelude::ChatId(chat_id.0));
		let req = self.api.send_message(chat_id, msg_str);
		self.tokio_runtime.block_on(async {
			let res = req.send().await;
			match res {
				Ok(msg) => Ok(ReseivedMessage {
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
}
