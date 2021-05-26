use std::io::{Read, Write};
use std::str::FromStr;

pub struct MessageStorage<T: Eq + Ord + serde::Serialize + for<'a> serde::Deserialize<'a>> {
	msg_list: Vec<(T, chrono::DateTime<chrono::Utc>)>,
}

impl<T: Clone + Eq + Ord + serde::Serialize + for<'a> serde::Deserialize<'a>> MessageStorage<T> {
	pub fn new() -> Self {
		match std::fs::File::open(MessageStorage::<T>::get_file_path()) {
			Ok(mut f) => return MessageStorage::new_from_file(&mut f),
			Err(_) => {
				return Self {
					msg_list: Vec::new(),
				}
			}
		}
	}

	fn new_from_file(f: &mut std::fs::File) -> Self {
		let mut data = Vec::new();
		f.read_to_end(&mut data).unwrap();
		if let Ok(msg_list) = serde_json::from_slice::<Vec<(T, String)>>(&data) {
			Self {
				msg_list: msg_list
					.into_iter()
					.filter_map(|(id, date_str)| {
						match chrono::DateTime::<chrono::Utc>::from_str(&date_str) {
							Ok(date) => Some((id, date)),
							Err(_) => None,
						}
					})
					.collect(),
			}
		} else {
			Self {
				msg_list: Vec::new(),
			}
		}
	}

	pub fn get_old_messages(&self, msg_age: &chrono::Duration) -> Vec<T> {
		let too_old = chrono::Utc::now() - *msg_age;
		self.msg_list
			.iter()
			.filter_map(|(id, date)| {
				if *date < too_old {
					Some(id.clone())
				} else {
					None
				}
			})
			.collect()
	}

	pub fn get_old_messages_std(&self, msg_age: &std::time::Duration) -> Vec<T> {
		self.get_old_messages(&chrono::Duration::from_std(*msg_age).unwrap())
	}

	pub fn add_message(&mut self, msg_id: T) {
		for (id, _) in self.msg_list.iter() {
			if *id == msg_id {
				return;
			}
		}
		self.msg_list.push((msg_id, chrono::Utc::now()));
		self.write_messages().ok();
	}

	pub fn remove_messages(&mut self, msg_list: Vec<T>) {
		let mut msg_list = msg_list;
		msg_list.sort();

		self.msg_list
			.retain(|(id, _)| msg_list.binary_search(id).is_err());

		self.write_messages().ok();
	}

	fn write_messages(&self) -> Result<(), std::io::Error> {
		let mut path = std::env::current_exe().unwrap();
		path.pop();
		path.push("message.json");
		let mut f = std::fs::OpenOptions::new()
			.write(true)
			.create(true)
			.open(MessageStorage::<T>::get_file_path())?;

		self.write_messages_to_file(&mut f)
	}

	fn get_file_path() -> std::path::PathBuf {
		let mut path = std::env::current_exe().unwrap();
		path.pop();
		path.push("message.json");

		path
	}

	fn write_messages_to_file(&self, f: &mut std::fs::File) -> Result<(), std::io::Error> {
		let v: Vec<_> = self
			.msg_list
			.iter()
			.map(|(id, date)| (id, date.to_string()))
			.collect();

		let json = serde_json::to_string(&v).unwrap();
		f.write(json.as_bytes()).map(|_| {})
	}
}

#[cfg(test)]
mod test {

	use super::*;
	use std::io::{Seek, SeekFrom};

	#[test]
	fn test_1() {
		let mut msg = MessageStorage::new();
		msg.add_message(1);
		msg.add_message(2);
		std::thread::sleep(std::time::Duration::from_secs(1));
		msg.add_message(3);
		assert_eq!(
			msg.get_old_messages(&chrono::Duration::milliseconds(500)),
			[1, 2]
		);
		msg.remove_messages(vec![1]);
		assert_eq!(
			msg.get_old_messages(&chrono::Duration::milliseconds(500)),
			[2]
		);
	}

	#[test]
	fn test_2() {
		let mut storage_file = tempfile::tempfile().unwrap();
		{
			let cur_dt = chrono::Utc::now();

			let mut new_msg_list = Vec::new();
			new_msg_list.push((1, cur_dt));
			new_msg_list.push((2, cur_dt - chrono::Duration::days(1)));
			new_msg_list.push((3, cur_dt - chrono::Duration::days(2)));
			new_msg_list.push((4, cur_dt - chrono::Duration::days(3)));

			let mut msg = MessageStorage::new_from_file(&mut storage_file);
			msg.msg_list = new_msg_list;
			msg.write_messages_to_file(&mut storage_file).unwrap();
		}
		storage_file.seek(SeekFrom::Start(0)).unwrap();

		{
			let msg = MessageStorage::<i32>::new_from_file(&mut storage_file);
			let msgs = msg.get_old_messages(&chrono::Duration::hours(36));
			assert_eq!(msgs, [3, 4]);
		}
	}
}
