extern crate futures;
extern crate sysinfo;
extern crate telegram_bot;

use futures::FutureExt;
use futures::{pin_mut, select, StreamExt};
use std::collections::HashMap;
use telegram_bot::types::refs::UserId;
use telegram_bot::*;

mod activity;
mod logger;

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

fn try_get_creator_id() -> Option<UserId> {
	std::env::var("CREATOR_ID")
		.ok()
		.and_then(|s| s.parse().ok())
		.map(UserId::new)
}

type UserActions = HashMap<sysinfo::Pid, activity::ActivityKind>;
type AllActions = HashMap<UserId, UserActions>;

struct BotData {
	creator_id: Option<UserId>,
	logger: logger::Logger,

	api: telegram_bot::Api,
	subscribers: AllActions,
}

impl BotData {
	fn process_message(&mut self, msg: &str, chat: &telegram_bot::types::User) {
		let logger_msg = if Some(chat.id) == self.creator_id {
			"Creator message".to_string()
		} else {
			format!("User message, name: {}, id: {}", chat.first_name, chat.id)
		};

		let logger_msg = format!("<{}>: {}", logger_msg, msg);
		let request_type = Request::new(msg);
		let s = match request_type {
			Request::Help => SendMessage::new(chat, get_string_help()),

			Request::Check | Request::Subscribe => {
				let act_list = activity::get_activity_list();
				let s = if let Some(elem) = act_list.first() {
					// Has at least one element

					let mut msg = if act_list.len() == 1 {
						format!("Current action: {}", elem.activity)
					} else {
						"There are many actions".to_owned()
					};
					if request_type == Request::Subscribe {
						msg += "\n";
						msg += "When the action completed you will be notified";

						let h: std::collections::HashMap<_, _> =
							act_list.into_iter().map(|a| (a.pid, a.activity)).collect();

						self.subscribers.insert(chat.id, h);
					}

					msg
				} else {
					"There is no current actions".to_owned()
				};

				SendMessage::new(chat, s)
			}

			Request::UnknownRequest(_) => SendMessage::new(
				chat,
				format!("Unknown command: {}. \n{}", msg, get_string_help()),
			),
		};
		self.logger.write(&logger_msg);

		self.api.spawn(s);
	}

	fn process_timer(&mut self) {
		let current_actions = activity::get_activity_list();
		let pid_list_new = current_actions.iter().map(|a| &a.pid).collect();

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

#[tokio::main]
async fn main() {
	let token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap();
	let creator_id = try_get_creator_id();

	let subscribers = HashMap::new();
	let logger = logger::Logger::new(std::env::args().find(|a| a == "-no_log_file").is_none());
	let api = Api::new(token);

	if let Some(creator_id) = creator_id {
		let user = telegram_bot::chat::User {
			first_name: "".to_string(),
			id: creator_id,
			is_bot: false,
			language_code: None,
			last_name: None,
			username: None,
		};
		let chat = telegram_bot::chat::MessageChat::Private(user);
		api.spawn(SendMessage::new(chat, "Bot started"));
	}
	let mut msg_stream = api.stream();
	let mut timer = tokio::time::interval(std::time::Duration::from_secs(10));

	let mut bot_data = BotData {
		api,
		subscribers,
		creator_id,
		logger,
	};

	loop {
		bot_data
			.logger
			.write("-----------------------------\nBot started");

		let tick = timer.tick().fuse();
		let msg = msg_stream.next().fuse();

		pin_mut!(tick, msg);

		select! {
			msg = msg => {
				if let Some(msg) = msg {
					if let Ok(msg) = msg {
						if let UpdateKind::Message(message) = msg.kind {
							if let MessageKind::Text { ref data, .. } = message.kind {
								bot_data.process_message(&data, &message.from);
							}
						}
					}
				}
			},

			_ = tick => bot_data.process_timer(),
		}
	}
}
