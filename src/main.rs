extern crate futures;
extern crate sysinfo;
extern crate telegram_bot;
extern crate tokio_core;

use futures::{Future, Stream};
use telegram_bot::types::refs::UserId;
use telegram_bot::*;
use tokio_core::reactor::Core;

mod logger;

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

#[derive(Debug)]
enum ActivityKind {
	Build,
	Deploy,
	UpdateToRevision,
}

impl std::fmt::Display for ActivityKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			ActivityKind::Build => write!(f, "Build"),
			ActivityKind::Deploy => write!(f, "Deploy"),
			ActivityKind::UpdateToRevision => write!(f, "Update to revisions"),
		}
	}
}

fn get_current_activity_kind() -> Option<ActivityKind> {
	use sysinfo::{ProcessExt, RefreshKind, System, SystemExt};

	let sys = System::new_with_specifics(RefreshKind::new().with_processes());

	for (_, proc) in sys.get_processes() {
		match proc.name() {
			"qtcreator_ctrlc_stub.exe" => return Some(ActivityKind::Build),
			"python.exe" => {
				if proc.cmd().contains(&"update_to_revisions.py".to_owned()) {
					return Some(ActivityKind::UpdateToRevision);
				}
			}
			"jinnee-utility.exe" => {
				if proc.cmd().contains(&"--deploy_stand".to_owned()) {
					return Some(ActivityKind::Deploy);
				}
			}
			&_ => continue,
		}
	}

	None
}

fn main() {
	let mut core = Core::new().unwrap();

	let token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap();
	let api = Api::configure(token).build(core.handle()).unwrap();

	let subscribers = std::cell::RefCell::new(std::collections::HashSet::new());

	let mut logger = logger::Logger::new();

	let creator_id = try_get_creator_id();

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
					let s = match Request::new(data) {
						Request::Help => SendMessage::new(message.chat, get_string_help()),

						Request::Check => {
							let s = if let Some(act) = get_current_activity_kind() {
								format!("{}", act)
							} else {
								"There is no current activity".to_owned()
							};
							SendMessage::new(message.chat, s)
						}

						Request::Subscribe => {
							let s = if get_current_activity_kind().is_some() {
								subscribers.borrow_mut().insert(message.chat.clone());
								"You have subscribed to end of building notification"
							} else {
								"There is no building process"
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
			let mut subscribers = subscribers.borrow_mut();
			let tt = get_current_activity_kind();
			if tt.is_some() || subscribers.is_empty() {
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
