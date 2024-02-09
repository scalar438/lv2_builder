use activity::ProcessDescription;
use futures::FutureExt;
use futures::{pin_mut, select, StreamExt};
use serde::Serialize;
use std::collections::HashMap;
use teloxide::requests::Requester;
use teloxide::types::{ChatId, MediaKind, MessageId, MessageKind, UpdateKind, User, UserId};
use teloxide::update_listeners::AsUpdateStream;

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
	/help: prints help message.
	/check: checks the build/deploy status.
	/subscribe: sends a notification when the build/deploy process completes.
	"
	.to_string()
}

type UserActions = HashMap<sysinfo::Pid, (activity::ActivityKind, String)>;
type AllActions = HashMap<UserId, UserActions>;

struct BotData {
	owner_id: Option<UserId>,

	api_new: teloxide::Bot,
	subscribers: AllActions,

	msg_storage: msg_storage::MessageStorage<(ChatId, i32)>,
}

impl BotData {
	async fn process_message(&mut self, msg: &str, chat: &User) {
		let request_type = Request::from(msg);
		let s = match request_type {
			Request::Help => (chat, get_string_help()),

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
						msg += "When action have been completed you will be notified";

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

				(chat, s)
			}

			Request::UnknownRequest(_) => (
				chat,
				format!("Unknown command: {}. \n{}", msg, get_string_help()),
			),
		};
		let u = s.0.id.0 as u64;
		self.send_message_new_api(ChatId(u as i64), s.1).await
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
					};
					let chat_id = chat.0;
					msg_list.push((chat_id, msg));
				}
				actions.remove(&pid);
			}
		}
		for (chat, msg) in msg_list {
			self.send_message_new_api(ChatId(chat as i64), msg).await;
		}

		self.subscribers.retain(|_, actions| actions.len() != 0);
	}

	async fn delete_old_messages(&mut self) {
		let old_msg = self
			.msg_storage
			.get_old_messages(&std::time::Duration::from_secs(60 * 60 * 24));
		let mut deleted_msg = Vec::new();
		let mut is_first_iter = true;
		for (chat_id, msg_id) in old_msg.iter() {
			if !is_first_iter {
				tokio::time::sleep(std::time::Duration::from_millis(500)).await;
			} else {
				is_first_iter = false;
			}
			let res = self
				.api_new
				.delete_message(chat_id.clone(), MessageId(*msg_id))
				.await;
			if res.is_ok() {
				deleted_msg.push((chat_id.clone(), msg_id.clone()));
			}
		}
		self.msg_storage.remove_messages(deleted_msg);
	}

	async fn send_message_new_api<M: ToString + Send>(&mut self, chat_id: ChatId, s: M) {
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
		.map(UserId);

	println!("{} {:?}", token, owner_id);

	(token.to_owned(), owner_id)
}

fn main() {
	let runtime = tokio::runtime::Builder::new_current_thread()
		.enable_time()
		.enable_io()
		.build()
		.unwrap();

	runtime.block_on(async {
		let (token, owner_id) = read_config();

		let subscribers = HashMap::new();
		let api2 = teloxide::Bot::new(token);

		let mut api2_updates_stream =
			teloxide::update_listeners::polling_default(api2.clone()).await;
		let mut api2_updates_stream_2 = api2_updates_stream.as_stream();

		let mut check_timer = tokio::time::interval(std::time::Duration::from_secs(10));
		// Clear the chat from old messages every 4 hours
		let mut delete_msg_timer =
			tokio::time::interval(std::time::Duration::from_secs(60 * 40 * 4));

		let mut bot_data = BotData {
			api_new: api2,
			subscribers,
			owner_id,
			msg_storage: msg_storage::MessageStorage::new(),
		};

		if let Some(owner_id) = bot_data.owner_id {
			let chat_id_new = ChatId(owner_id.0 as i64);
			bot_data
				.send_message_new_api(chat_id_new, "Bot has started")
				.await;
		}
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

				/*_ = delete_msg_tick => {
					println!("Del Msg tick");
					bot_data.delete_old_messages().await;},*/

				_ = check_tick => {println!("check_tick");
				bot_data.process_check_timer().await;},
			}
		}
	});
}
