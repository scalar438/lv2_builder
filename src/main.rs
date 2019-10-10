extern crate futures;
extern crate telegram_bot;
extern crate tokio_core;

use futures::Stream;
use telegram_bot::*;
use tokio_core::reactor::Core;

use futures::Future;

mod logger;

pub enum Request {
	Help,
	Check,
	Subscribe,
	UnknownRequest(String),
}

impl Request {
	pub fn new(command: &str) -> Request {
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
	"This is a simple bot for logviz2 build progress notification. List of supported commands:
	/help: print this message.
	/check: check the build status. Success status request is not implemented yet.
	/subscribe: send a notification when the build process complete.
	"
	.to_string()
}

extern crate sysinfo;

fn is_building_just_now() -> bool {
	use std::collections::HashSet;
	use sysinfo::{ProcessExt, RefreshKind, System, SystemExt};

	let sys = System::new_with_specifics(RefreshKind::new().with_processes());

	let builders_list = sys.get_process_by_name("qtcreator_ctrlc_stub");
	let qtc_list: HashSet<_> = sys
		.get_process_by_name("qtcreator")
		.into_iter()
		.map(|x| x.pid())
		.collect();

	builders_list.iter().any(|x| qtc_list.contains(&x.pid()))
}

fn main() {
	let creator_id = telegram_bot::types::refs::UserId::new(211101859i64);

	let mut core = Core::new().unwrap();

	let token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap();
	let api = Api::configure(token).build(core.handle()).unwrap();

	let subscribers = std::cell::RefCell::new(std::collections::HashSet::new());

	let mut logger = logger::Logger::new();

	{
		let user = telegram_bot::chat::User {
			first_name: "".to_string(), 
			id: creator_id, 
			is_bot: false, 
			language_code: None,
			last_name: None, 
			username: None
		};
		let chat = telegram_bot::chat::MessageChat::Private(user);
		api.spawn(SendMessage::new(chat, "Bot started"));

		logger.write("-----------------------------\nBot started");
	}

	// Fetch new updates via long poll method
	let message_process = api
		.stream()
		.for_each(|update| {
			// If the received update contains a new message...
			if let UpdateKind::Message(message) = update.kind {
				if let MessageKind::Text { ref data, .. } = message.kind {
					let logger_msg = if message.from.id == creator_id {
						"Creator message".to_string()
					} else {
						format!(
							"User message, name: {}, id: {}",
							message.from.first_name, message.from.id
						)
					};
					let logger_msg = format!("<{}>: {}", logger_msg, data);
					let s = match Request::new(data) {
						Request::Help => SendMessage::new(message.chat, get_string_help()),

						Request::Check => {
							let s = if is_building_just_now() {
								"Building in progress"
							} else {
								"Build completed"
							};
							SendMessage::new(message.chat, s)
						}

						Request::Subscribe => {
							let s = if is_building_just_now() {
								subscribers.borrow_mut().insert(message.chat.clone());
								"You have subscribed on notification about end of building"
							} else {
								"There is no building process"
							};
							SendMessage::new(message.chat, s)
						}

						Request::UnknownRequest(_) => SendMessage::new(
							message.chat,
							format!(
								"Unknown command: {}. Try /help to get list of available commands",
								data
							),
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
			let mut subscribers = subscribers.borrow_mut();
			if is_building_just_now() || subscribers.is_empty() {
				return;
			}
			for s in std::mem::replace(&mut *subscribers, std::collections::HashSet::new()) {
				api.spawn(SendMessage::new(s, "Build completed"));
			}
		})
		.for_each(|_| Ok(()))
		.map_err(|_| ());

	let joined = message_process.join(status_timer);

	core.run(joined).unwrap();
}
