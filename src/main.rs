#[macro_use]
extern crate futures;
extern crate telegram_bot;
extern crate tokio_core;

extern crate tokio;

use futures::Stream;
use telegram_bot::*;
use tokio::timer::Interval;
use tokio_core::reactor::Core;

use std::time::Duration;

#[derive(Debug)]
pub struct Fibonacci {
	interval: Interval,
	curr: u64,
	next: u64,
}

impl Fibonacci {
	fn new(duration: Duration) -> Fibonacci {
		Fibonacci {
			interval: Interval::new_interval(duration),
			curr: 1,
			next: 1,
		}
	}
}

impl Stream for Fibonacci {
	type Item = u64;

	// The stream will never yield an error
	type Error = ();

	fn poll(&mut self) -> futures::Poll<Option<u64>, ()> {
		// Wait until the next interval
		try_ready!(self
			.interval
			.poll()
			// The interval can fail if the Tokio runtime is unavailable.
			// In this example, the error is ignored.
			.map_err(|_| ()));

		let curr = self.curr;
		let next = curr + self.next;

		self.curr = self.next;
		self.next = next;

		Ok(Async::Ready(Some(curr)))
	}
}

use futures::Async;
use futures::Future;

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

	let fib_stream = Fibonacci::new(Duration::from_secs(3)).for_each(|x| {
		println!("{:?}", x);

		Ok(())
	});

	core.run(fib_stream.join(future)).unwrap();
}
