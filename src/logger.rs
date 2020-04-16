use chrono::Datelike;
use std::io::Write;

pub struct Logger {
	file: Option<std::fs::File>,
}

impl Logger {
	pub fn new(use_file: bool) -> Self {
		if !use_file {
			return Logger { file: None };
		}
		let file = if let Ok(path) = std::env::current_dir() {
			let now = chrono::Utc::now();
			let filename = format!("{}.{}.{}.log", now.year(), now.month(), now.day());
			let path = path.join(filename);
			let file = std::fs::OpenOptions::new()
				.create_new(true)
				.append(true)
				.open(path.clone())
				.or_else(|x| {
					if x.kind() == std::io::ErrorKind::AlreadyExists {
						std::fs::OpenOptions::new().append(true).open(path.clone())
					} else {
						Err(x)
					}
				});

			match file {
				Ok(f) => {
					println!("Log file opened. Filename: {:?}", path);
					Some(f)
				}
				Err(e) => {
					println!("Cannot open log file, error: {}\n", e);
					None
				}
			}
		} else {
			None
		};

		Logger { file }
	}

	pub fn write(&mut self, s: &str) {
		println!("{}", s);
		if self.write_to_file(s).is_err() {
			println!("Unexpected file error");
		}
	}

	pub fn write_to_file(&mut self, s: &str) -> std::io::Result<()> {
		if let Some(f) = &mut self.file {
			f.write(s.as_bytes())?;
			f.write(b"\n")?;
			f.flush()?;
		}
		Ok(())
	}
}
