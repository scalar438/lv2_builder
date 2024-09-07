use teloxide::types::UserId;

pub struct Config {
	pub owner_id: UserId,
	pub token: String,
	pub auto_subscribe: bool,
}

pub fn read_config() -> Config {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.push("config.ini");

	read_config_from_file(path)
}

fn read_config_from_file(path: std::path::PathBuf) -> Config {
	let inifile = ini::Ini::load_from_file(path).unwrap();
	let section = inifile.section::<String>(None).unwrap();
	let token = section.get("token").unwrap();
	let owner_id = section
		.get("owner_id")
		.and_then(|s| s.parse().ok())
		.map(UserId)
		.unwrap();

	let auto_subscribe = section
		.get("auto_subscribe")
		.and_then(|s| s.parse().ok())
		.unwrap_or(true);

	println!(
		"Token: {}, owner_id: {:?}, auto_subscribe: {}",
		token, owner_id, auto_subscribe
	);

	Config {
		owner_id: owner_id,
		token: token.to_owned(),
		auto_subscribe: auto_subscribe,
	}
}

#[cfg(test)]
mod test {
	use std::io::Write;

	use super::*;

	#[test]
	fn test_config() {
		let mut ini_file = tempfile::NamedTempFile::new().unwrap();
		ini_file
			.write(
				br#"
token="token"
owner_id = "42"
auto_subscribe="false"
		"#,
			)
			.unwrap();
		ini_file.flush().unwrap();
		let config = read_config_from_file(ini_file.path().to_path_buf());
		assert_eq!(config.auto_subscribe, false);
		assert_eq!(config.token, "token");
		assert_eq!(config.owner_id.0, 42);
	}
}
