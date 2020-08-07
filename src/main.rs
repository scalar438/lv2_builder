extern crate futures;
extern crate ini;
extern crate sysinfo;
extern crate telegram_bot;

use futures::FutureExt;
use futures::{pin_mut, select, StreamExt};
use std::collections::HashMap;
use telegram_bot::types::refs::UserId;
use telegram_bot::*;

mod activity;
use activity::ProcessDescription;

#[derive(PartialEq, Eq)]
enum Request {
	Help,
	Check,
	Subscribe,
	UnknownRequest(String),
}

impl Request {
	fn new(command: &str) -> Request {
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

type UserActions = HashMap<sysinfo::Pid, activity::ActivityKind>;
type AllActions = HashMap<UserId, UserActions>;

struct BotData {
	owner_id: Option<UserId>,

	api: telegram_bot::Api,
	subscribers: AllActions,
}

impl BotData {
	fn process_message(&mut self, msg: &str, chat: &telegram_bot::types::User) {
		let request_type = Request::new(msg);
		let s = match request_type {
			Request::Help => SendMessage::new(chat, get_string_help()),

			Request::Check | Request::Subscribe => {
				let act_list = activity::get_activity_list();
				let s = if let Some(elem) = act_list.first() {
					// There is at least one element

					let mut msg = if act_list.len() == 1 {
						format!("Current action: {}", elem.activity_kind())
					} else {
						"There are many actions".to_owned()
					};
					if request_type == Request::Subscribe {
						msg += "\n";
						msg += "When the action completed you will be notified";

						let h: std::collections::HashMap<_, _> = act_list
							.into_iter()
							.map(|a| (*a.pid(), a.activity_kind().clone()))
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

		self.api.spawn(s);
	}

	fn process_timer(&mut self) {
		let current_actions = activity::get_activity_list();
		let pid_list_new = current_actions.iter().map(|a| a.pid()).collect();

		for (chat, actions) in self.subscribers.iter_mut() {
			let pid_list_old: std::collections::HashSet<_> = actions.keys().collect();
			assert_ne!(pid_list_old.len(), 0);

			let completed_list: Vec<_> = pid_list_old
				.difference(&pid_list_new)
				.map(|a| **a)
				.collect();
			for pid in completed_list {
				if let Some(act) = actions.get(&pid) {
					self.api
						.spawn(SendMessage::new(chat.clone(), format!("{} completed", act)));
				}
				actions.remove(&pid);
			}
		}
		self.subscribers.retain(|_, actions| actions.len() != 0);
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
	let mut timer = tokio::time::interval(std::time::Duration::from_secs(10));

	let mut bot_data = BotData {
		api,
		subscribers,
		owner_id,
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
		bot_data.api.spawn(SendMessage::new(chat, "Bot started"));
	}
	loop {
		let tick = timer.tick().fuse();
		let msg = msg_stream.next().fuse();

		pin_mut!(tick, msg);

		select! {
			msg = msg => {
				if let Some(Ok(msg)) = msg {
					if let UpdateKind::Message(message) = msg.kind {
						if let MessageKind::Text { ref data, .. } = message.kind {
							bot_data.process_message(&data, &message.from);
						}
					}
				}
			},

			_ = tick => bot_data.process_timer(),
		}
	}
}
