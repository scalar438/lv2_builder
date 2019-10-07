pub enum Request {
	Help,

	UnknownRequest(String),
}

impl Request {
	pub fn new(command: &str) -> Request {
		let vs: Vec<_> = command
			.split_ascii_whitespace()
			.filter_map(|s| {
				if s.len() == 0 {
					None
				} else {
					Some(s.to_ascii_lowercase())
				}
			})
			.collect();

		if vs.is_empty() || vs[0] != "help" {
			return Request::UnknownRequest(command.to_string());
		}

		Request::Help
	}
}
