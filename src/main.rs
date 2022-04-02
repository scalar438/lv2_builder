use activity::ProcessDescription;
use futures::FutureExt;
use futures::{pin_mut, select, StreamExt};
use std::collections::HashMap;
use telegram_bot::types::refs::MessageId;
use telegram_bot::types::refs::UserId;
use telegram_bot::types::MessageOrChannelPost;
use telegram_bot::*;

mod activity;
mod msg_storage;

#[derive(PartialEq, Eq)]
enum Request {
	Help,
	Check,
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
		if vs[0] == "check" {
			return Request::Check;
		}
		if vs[0] == "subscribe" {
			return Request::Subscribe;
		}
		Request::UnknownRequest(command.to_string())
	}
}

fn get_string_help() -> String {
	"This is a simple bot for sbis build/deploy progress notification. List of supported commands:
	/help: print help message.
	/check: check the build/deploy status.
	/subscribe: send a notification when the build/deploy process complete.
	"
	.to_string()
}

type UserActions = HashMap<sysinfo::Pid, (activity::ActivityKind, String)>;
type AllActions = HashMap<UserId, UserActions>;

struct BotData {
	owner_id: Option<UserId>,

	api: telegram_bot::Api,
	subscribers: AllActions,

	msg_storage: msg_storage::MessageStorage<(ChatId, MessageId)>,
}

impl BotData {
	async fn process_message(&mut self, msg: &str, chat: &telegram_bot::types::User) {
		let request_type = Request::from(msg);
		let s = match request_type {
			Request::Help => SendMessage::new(chat, get_string_help()),

			Request::Check | Request::Subscribe => {
				let act_list = activity::get_activity_list();
				let s = if let Some(elem) = act_list.first() {
					// There is at least one element

					let mut msg = if act_list.len() == 1 {
						format!("Current action: {}", elem.activity_kind())
					} else {
						"There are several running actions".to_owned()
					};
					if request_type == Request::Subscribe {
						msg += "\n";
						msg += "When action completed you will be notified";

						let h: std::collections::HashMap<_, _> = act_list
							.into_iter()
							.map(|a| {
								(
									*a.pid(),
									(a.activity_kind().clone(), a.description().to_owned()),
								)
							})
							.collect();

						self.subscribers.insert(chat.id, h);
					}

					msg
				} else {
					"There is no current action".to_owned()
				};

				SendMessage::new(chat, s)
			}

			Request::UnknownRequest(_) => SendMessage::new(
				chat,
				format!("Unknown command: {}. \n{}", msg, get_string_help()),
			),
		};

		self.send_message(s).await
	}

	async fn process_check_timer(&mut self) {
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
					if !act.1.is_empty() {
						msg += ", path = `\"";
						msg += &act.1;
						msg += "\"`";
					}
					msg_list.push(SendMessage::new(chat.clone(), msg));
				}
				actions.remove(&pid);
			}
		}
		for msg in msg_list {
			self.send_message(msg).await;
		}

		self.subscribers.retain(|_, actions| actions.len() != 0);
	}

	async fn delete_old_messages(&mut self) {
		let old_msg = self
			.msg_storage
			.get_old_messages_std(&std::time::Duration::from_secs(60 * 60 * 24));
		let mut deleted_msg = Vec::new();
		for (chat_id, msg_id) in old_msg.iter() {
			let req = telegram_bot::types::delete_message::DeleteMessage::new(chat_id, msg_id);
			let res = self.api.send(req).await;
			if res.is_ok() {
				deleted_msg.push((*chat_id, *msg_id));
			}
		}
		self.msg_storage.remove_messages(deleted_msg);
	}

	async fn send_message<'a>(&mut self, msg: SendMessage<'a>) {
		if let Ok(MessageOrChannelPost::Message(msg)) = self.api.send(msg).await {
			self.msg_storage.add_message((msg.chat.id(), msg.id));
		}
	}
}

// Return token and (optional) owner id
fn read_config() -> (String, Option<UserId>) {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.push("config.ini");

	let inifile = ini::Ini::load_from_file(path).unwrap();
	let section = inifile.section::<String>(None).unwrap();
	let token = section.get("token").unwrap();
	let owner_id = section
		.get("owner_id")
		.and_then(|s| s.parse().ok())
		.map(UserId::new);

	println!("{} {:?}", token, owner_id);

	(token.to_owned(), owner_id)
}

#[tokio::main]
async fn main() {
	let (token, owner_id) = read_config();

	let subscribers = HashMap::new();
	let api = Api::new(token);

	let mut msg_stream = api.stream();
	let mut check_timer = tokio::time::interval(std::time::Duration::from_secs(10));
	// Clear chat from old messages every 4 hours
	let mut delete_msg_timer = tokio::time::interval(std::time::Duration::from_secs(60 * 60 * 4));

	let mut bot_data = BotData {
		api,
		subscribers,
		owner_id,
		msg_storage: msg_storage::MessageStorage::new(),
	};

	if let Some(owner_id) = bot_data.owner_id {
		let user = telegram_bot::chat::User {
			first_name: "".to_string(),
			id: owner_id,
			is_bot: false,
			language_code: None,
			last_name: None,
			username: None,
		};
		let chat = telegram_bot::chat::MessageChat::Private(user);
		bot_data
			.send_message(SendMessage::new(chat, "Bot started"))
			.await;
	}
	loop {
		let check_tick = check_timer.tick().fuse();
		let delete_msg_tick = delete_msg_timer.tick().fuse();
		let msg = msg_stream.next().fuse();

		pin_mut!(check_tick, msg, delete_msg_tick);

		select! {
			msg = msg => {
				if let Some(Ok(msg)) = msg {
					if let UpdateKind::Message(message) = msg.kind {
						bot_data.msg_storage.add_message((message.chat.id(), message.id));
						if let MessageKind::Text { ref data, .. } = message.kind {
							bot_data.process_message(&data, &message.from).await;
						}
					}
				}
			},

			_ = delete_msg_tick => bot_data.delete_old_messages().await,

			_ = check_tick => bot_data.process_check_timer().await,
		}
	}
}
