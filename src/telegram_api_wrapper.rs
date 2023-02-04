extern crate async_trait;

use serde::{Deserialize, Serialize};
use teloxide::prelude::*;

pub enum TelegramError {
	GeneralError(String),
}

type TlgResult<T> = Result<T, TelegramError>;

#[derive(PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct ChatId(pub i64);

#[derive(PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct UserId(pub i64);

#[derive(PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct MessageId(pub i64);

struct ReseivedMessage {
	chat_id: ChatId,
	user_id: UserId,
	message_id: MessageId,
}

#[async_trait::async_trait]
pub trait Api {
	fn create(token: &str) -> Self;

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

#[async_trait::async_trait]
impl Api for ApiWrapper {
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

	async fn send_message<M: ToString + Send>(
		&mut self,
		chat_id: ChatId,
		msg: M,
	) -> TlgResult<ReseivedMessage> {
		unimplemented!()
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
