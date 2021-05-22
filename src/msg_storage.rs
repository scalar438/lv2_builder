struct MessageStorage {}

impl MessageStorage {
	fn new() -> Self {
		Self {}
	}

	fn get_old_messages(&self) -> Vec<i32> {
		unimplemented!()
	}

	fn remove_messages(&mut self, msg_list: &[i32]) {
		unimplemented!()
	}
}
