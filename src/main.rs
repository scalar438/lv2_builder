extern crate futures;
extern crate telegram_bot;
extern crate tokio_core;

use futures::Stream;
use telegram_bot::*;
use tokio_core::reactor::Core;

use futures::Future;

pub enum Request {
	Help,
	Check,
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
		Request::UnknownRequest(command.to_string())
	}
}

fn get_string_help() -> String {
	"This is simple bot for build logviz2 notification. List of supported commands:
	/help: print this message.
	/check: check build status for finished (NOT IMPLEMENTED YET)
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
	let mut core = Core::new().unwrap();

	let token = std::env::var("TELEGRAM_BOT_TOKEN").unwrap();
	let api = Api::configure(token).build(core.handle()).unwrap();

	// Fetch new updates via long poll method
	let future = api
		.stream()
		.for_each(|update| {
			// If the received update contains a new message...
			if let UpdateKind::Message(message) = update.kind {
				if let MessageKind::Text { ref data, .. } = message.kind {
					let s = match Request::new(data) {
						Request::Help => {
							telegram_bot::types::requests::send_message::SendMessage::new(
								message.chat,
								get_string_help(),
							)
						}

						Request::Check => {
							let s = if is_building_just_now() {"Building in progress"} else {"Build completed (maybe with errors, may be not)"};
							telegram_bot::types::requests::send_message::SendMessage::new(
								message.chat,
								s,
							)
						}

						Request::UnknownRequest(_) => {
							telegram_bot::types::requests::send_message::SendMessage::new(
								message.chat,
								format!("Unknown command: {}. Try /help to get list of available commands", data),
							)
						}
					};
					// Print received text message to stdout.
					println!("<{}>: {}", &message.from.first_name, data);

					api.spawn(s);
				}
			}

			Ok(())
		})
		.map_err(|_| ());

	core.run(future).unwrap();
}
