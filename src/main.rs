extern crate futures;
extern crate sysinfo;
extern crate telegram_bot;
extern crate tokio_core;

use futures::{Future, Stream};
use telegram_bot::types::refs::UserId;
use telegram_bot::*;
use tokio_core::reactor::Core;

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
	/check: check the build/deploy status. Success status request is not implemented yet.
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

fn main() {
	let token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap();
	let creator_id = try_get_creator_id();

	let mut core = Core::new().unwrap();
	let subscribers = std::cell::RefCell::new(std::collections::HashMap::new());
	let mut logger = logger::Logger::new();
	let api = Api::configure(token.clone()).build(core.handle()).unwrap();

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

	loop {
		logger.write("-----------------------------\nBot started");
		// Fetch new updates via long poll method
		let message_process = api
			.stream()
			.for_each(|update| {
				// If the received update contains a new message...
				if let UpdateKind::Message(message) = update.kind {
					if let MessageKind::Text { ref data, .. } = message.kind {
						let logger_msg = if Some(message.from.id) == creator_id {
							"Creator message".to_string()
						} else {
							format!(
								"User message, name: {}, id: {}",
								message.from.first_name, message.from.id
							)
						};

						let logger_msg = format!("<{}>: {}", logger_msg, data);
						let request_type = Request::new(data);
						let s = match request_type {
							Request::Help => SendMessage::new(message.chat, get_string_help()),

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

										let h: std::collections::HashMap<_, _> = act_list
											.into_iter()
											.map(|a| (a.pid, a.activity))
											.collect();

										subscribers.borrow_mut().insert(message.chat.clone(), h);
									}

									msg
								} else {
									"There no current actions".to_owned()
								};

								SendMessage::new(message.chat, s)
							}

							Request::UnknownRequest(_) => SendMessage::new(
								message.chat,
								format!("Unknown command: {}. \n{}", data, get_string_help()),
							),
						};
						logger.write(&logger_msg);

						api.spawn(s);
					}
				}

				Ok(())
			})
			.map_err(|_| ());

		let status_timer = tokio::timer::Interval::new_interval(std::time::Duration::from_secs(10))
			.map(|_| {
				let current_actions = activity::get_activity_list();
				let pid_list_new = current_actions.iter().map(|a| &a.pid).collect();

				let mut subscribers = subscribers.borrow_mut();

				for (chat, actions) in subscribers.iter_mut() {
					let pid_list_old: std::collections::HashSet<_> = actions.keys().collect();
					assert_ne!(pid_list_old.len(), 0);

					let completed_list: Vec<_> = pid_list_old
						.difference(&pid_list_new)
						.map(|a| **a)
						.collect();
					for pid in completed_list {
						if let Some(act) = actions.get(&pid) {
							api.spawn(SendMessage::new(chat.clone(), format!("{} completed", act)));
						}
						actions.remove(&pid);
					}
				}
				subscribers.retain(|_, actions| actions.len() != 0);
			})
			.for_each(|_| Ok(()))
			.map_err(|_| ());

		let joined = message_process.join(status_timer);

		if let Err(e) = core.run(joined) {
			eprintln!("Error occured: {:?}. Try to restart...", e);
		}
	}
}
