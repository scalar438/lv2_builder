pub struct MessageStorage<T: Eq + Ord> {
	msg_list: Vec<(T, chrono::DateTime<chrono::Utc>)>,
}

impl<T: Eq + Ord + Clone> MessageStorage<T> {
	pub fn new() -> Self {
		Self {
			msg_list: Vec::new(),
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
	}

	pub fn remove_messages(&mut self, msg_list: Vec<T>) {
		let mut msg_list = msg_list;
		msg_list.sort();

		self.msg_list
			.retain(|(id, _)| msg_list.binary_search(id).is_err());
	}
}

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
