extern crate futures;
extern crate telegram_bot;
extern crate tokio_core;

extern crate tokio;

use futures::Stream;
use telegram_bot::*;
use tokio_core::reactor::Core;

use futures::Future;

mod process_messages;

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
					// Print received text message to stdout.
					println!("<{}>: {}", &message.from.first_name, data);

					// Answer message with "Hi".
					api.spawn(message.text_reply(format!(
						"Hi, {}! You just wrote '{}'",
						&message.from.first_name, data
					)));
				}
			}

			Ok(())
		})
		.map_err(|_| ());

	core.run(future).unwrap();
}
