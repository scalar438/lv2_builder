extern crate futures;
extern crate telegram_bot;
extern crate tokio_core;

extern crate tokio;

use futures::Stream;
use telegram_bot::*;
use tokio_core::reactor::Core;

use futures::Future;

mod process_messages;

fn get_string_help() -> String
{
	"This is simple bot for build logviz2 notification. List of supported commands:
	/help: print this message.
	/check: check build status for finished (NOT IMPLEMENTED YET)
	".to_string()
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
					use process_messages::Request;
					let s = match Request::new(data) {
						Request::Help => {
							telegram_bot::types::requests::send_message::SendMessage::new(
								message.chat,
								get_string_help(),
							)
						}

						Request::Check => {
							telegram_bot::types::requests::send_message::SendMessage::new(
								message.chat,
								"Checking is not implemented yet",
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
