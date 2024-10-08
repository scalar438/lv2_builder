use activity::ProcessDescription;
use futures::FutureExt;
use futures::{pin_mut, select, StreamExt};
use std::collections::{HashMap, HashSet};
use teloxide::requests::Requester;
use teloxide::types::{ChatId, MediaKind, MessageId, MessageKind, UpdateKind, User, UserId};
use teloxide::update_listeners::AsUpdateStream;

mod activity;
mod config;
mod msg_storage;

#[derive(PartialEq, Eq)]
enum Request {
	Help,
	Subscribe,
	UnknownRequest(String),
}

impl From<&str> for Request {
	fn from(command: &str) -> Request {
		let mut vs: Vec<_> = command
			.split_ascii_whitespace()
			.filter_map(|s| {
				if s.len() == 0 {
					None
				} else {
					Some(s.to_ascii_lowercase())
				}
			})
			.collect();

		if vs.is_empty() {
			return Request::UnknownRequest(command.to_string());
		}

		vs[0] = vs[0].chars().skip_while(|c| *c == '/').collect();

		if vs[0] == "help" {
			return Request::Help;
		}
		if vs[0] == "subscribe" {
			return Request::Subscribe;
		}
		Request::UnknownRequest(command.to_string())
	}
}

fn get_string_help() -> String {
	"This is a simple bot for sbis build/deploy progress notification. List of supported commands:
	/help: prints help message.
	/subscribe: sends a notification when the build/deploy process completes.
	"
	.to_string()
}

type UserActions = HashMap<sysinfo::Pid, (activity::ActivityKind, Option<String>)>;
type AllActions = HashMap<UserId, UserActions>;

struct BotData {
	owner_id: UserId,

	api_new: teloxide::Bot,
	subscribers: AllActions,

	msg_storage: msg_storage::MessageStorage<(ChatId, i32)>,
	auto_subscribe: bool,
}

impl BotData {
	fn subscribe(&mut self, chat_id: UserId) -> Option<String> {
		let act_list = activity::get_activity_list();
		if let Some(elem) = act_list.first() {
			// There is at least one element
			let mut msg = if act_list.len() == 1 {
				format!("Current action: {}", elem.activity_kind())
			} else {
				"There are several running actions".to_owned()
			};
			msg += "\n";
			msg += "When action have been completed you will be notified";

			let h: std::collections::HashMap<_, _> = act_list
				.into_iter()
				.map(|a| {
					(
						*a.pid(),
						(
							a.activity_kind().clone(),
							a.description().map(|x| x.to_owned()),
						),
					)
				})
				.collect();

			self.subscribers.insert(chat_id, h);

			Some(msg)
		} else {
			None
		}
	}

	async fn process_message(&mut self, msg: &str, chat: &User) {
		let request_type = Request::from(msg);
		let s = match request_type {
			Request::Help => (chat, get_string_help()),

			Request::Subscribe => {
				let s = self
					.subscribe(chat.id)
					.or(Some("There is no current action".to_owned()))
					.unwrap();
				(chat, s)
			}

			Request::UnknownRequest(_) => (
				chat,
				format!("Unknown command: {}. \n{}", msg, get_string_help()),
			),
		};
		let u = s.0.id.0 as u64;
		self.send_message(ChatId(u as i64), s.1).await
	}

	async fn process_check_timer(&mut self) {
		self.process_auto_subscribe_timer().await;

		let current_actions = activity::get_activity_list();
		let pid_list_new = current_actions.iter().map(|a| a.pid()).collect();

		let mut msg_list = Vec::new();

		for (chat, actions) in self.subscribers.iter_mut() {
			let pid_list_old: std::collections::HashSet<_> = actions.keys().collect();
			assert_ne!(pid_list_old.len(), 0);

			// List of completed actions
			let completed_list: Vec<_> = pid_list_old
				.difference(&pid_list_new)
				.map(|a| **a)
				.collect();
			for pid in completed_list {
				if let Some(act) = actions.get(&pid) {
					let mut msg = format!("{} completed", act.0);
					if let Some(s) = &act.1 {
						msg += ", path = `\"";
						msg += &s;
						msg += "\"`";
					};
					let chat_id = chat.0;
					msg_list.push((chat_id, msg));
				}
				actions.remove(&pid);
			}
		}
		for (chat, msg) in msg_list {
			self.send_message(ChatId(chat as i64), msg).await;
		}
		self.subscribers.retain(|_, actions| actions.len() != 0);
	}

	async fn process_auto_subscribe_timer(&mut self) {
		if !self.auto_subscribe {
			return;
		}

		let current_subscribers = self
			.subscribers
			.get(&self.owner_id)
			.unwrap_or(&HashMap::new())
			.clone();

		for action in activity::get_activity_list() {
			if !current_subscribers.contains_key(action.pid()) {
				self.send_message(
					ChatId(self.owner_id.0 as i64),
					format!(
						r#"New action: {}
Path: {}"#,
						action.activity_kind(),
						action.description().unwrap_or("")
					),
				)
				.await;
			}
		}

		self.subscribe(self.owner_id);
	}

	async fn delete_old_messages(&mut self) {
		let old_msg = self
			.msg_storage
			.get_old_messages(&std::time::Duration::from_secs(60 * 60 * 24));
		let mut deleted_msg = HashSet::new();
		let mut err_messages = HashSet::new();
		let mut is_first_iter = true;
		for (chat_id, msg_id) in old_msg.iter() {
			if !is_first_iter {
				tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
			} else {
				is_first_iter = false;
			}
			let res = self
				.api_new
				.delete_message(*chat_id, MessageId(*msg_id))
				.await;
			if res.is_ok() {
				deleted_msg.insert((chat_id.clone(), msg_id.clone()));
			} else {
				err_messages.insert((chat_id.clone(), msg_id.clone()));
			}
		}
		if !err_messages.is_empty() {
			// Get a list of very old messages - older than 10 days
			let old_msg = self
				.msg_storage
				.get_old_messages(&std::time::Duration::from_secs(60 * 60 * 24 * 10));
			for v in old_msg.iter() {
				if err_messages.contains(v) {
					deleted_msg.insert(v.clone());
				}
			}
		}
		self.msg_storage
			.remove_messages(deleted_msg.into_iter().collect());
	}

	async fn send_message<M: ToString + Send>(&mut self, chat_id: ChatId, s: M) {
		if let Ok(msg) = self
			.api_new
			.send_message(chat_id.clone(), s.to_string())
			.await
		{
			let msg_id = msg.id;
			self.msg_storage.add_message((chat_id, msg_id.0));
		}
	}
}

fn main() {
	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_time()
		.enable_io()
		.build()
		.unwrap();

	runtime.block_on(async {
		let config = config::read_config();

		let subscribers = HashMap::new();
		let api2 = teloxide::Bot::new(config.token);

		let mut api2_updates_stream =
			teloxide::update_listeners::polling_default(api2.clone()).await;
		let mut api2_updates_stream_2 = api2_updates_stream.as_stream();

		let mut check_timer = tokio::time::interval(std::time::Duration::from_secs(10));
		// Clear the chat from old messages every 4 hours
		let mut delete_msg_timer =
			tokio::time::interval(std::time::Duration::from_secs(60 * 60 * 4));

		let mut bot_data = BotData {
			api_new: api2,
			subscribers,
			owner_id: config.owner_id,
			msg_storage: msg_storage::MessageStorage::new(),
			auto_subscribe: config.auto_subscribe,
		};

		let chat_id_new = ChatId(bot_data.owner_id.0 as i64);
		bot_data.send_message(chat_id_new, "Bot has started").await;

		loop {
			let check_tick = check_timer.tick().fuse();
			let delete_msg_tick = delete_msg_timer.tick().fuse();
			let msg = api2_updates_stream_2.next().fuse();

			pin_mut!(check_tick, msg, delete_msg_tick);

			select! {
				msg = msg => {
					if let Some(Ok(msg)) = msg {
						if let UpdateKind::Message(message) = msg.kind
						{
							let chat_id = message.chat.id;
							let msg_id = message.id.0;

							bot_data.msg_storage.add_message((chat_id, msg_id));
							if let MessageKind::Common ( msg_common ) = message.kind {

								if let MediaKind::Text(media_text) = msg_common.media_kind{
									let user = msg_common.from.unwrap();
									bot_data.process_message(&media_text.text, &user).await
								}
							}
						}
					}
				},

				_ = delete_msg_tick =>
					bot_data.delete_old_messages().await,

				_ = check_tick => bot_data.process_check_timer().await,
			}
		}
	});
}
