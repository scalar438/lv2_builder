use teloxide::types::UserId;

pub struct Config {
	pub owner_id: Option<UserId>,
	pub token: String,
	auto_subscribe: bool,
}

pub fn read_config() -> Config {
	let mut path = std::env::current_exe().unwrap();
	path.pop();
	path.push("config.ini");

	let inifile = ini::Ini::load_from_file(path).unwrap();
	let section = inifile.section::<String>(None).unwrap();
	let token = section.get("token").unwrap();
	let owner_id = section
		.get("owner_id")
		.and_then(|s| s.parse().ok())
		.map(UserId);

	println!("{} {:?}", token, owner_id);

	Config {
		owner_id: owner_id,
		token: token.to_owned(),
		auto_subscribe: false,
	}
}
