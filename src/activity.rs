use sysinfo::{ProcessExt, RefreshKind, System, SystemExt};

#[derive(Debug)]
pub enum ActivityKind {
	Build,
	Deploy,
	UpdateToRevision,
}

impl std::fmt::Display for ActivityKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			ActivityKind::Build => write!(f, "Build"),
			ActivityKind::Deploy => write!(f, "Deploy"),
			ActivityKind::UpdateToRevision => write!(f, "Update to revisions"),
		}
	}
}

pub struct ProcessActivity {
	pid: sysinfo::Pid,
	pub activity: ActivityKind,
}

pub fn get_activity_list() -> Vec<ProcessActivity> {
	let sys = System::new_with_specifics(RefreshKind::new().with_processes());

	sys.get_processes()
		.iter()
		.filter_map(|(_, proc)| {
			let activity;
			match proc.name() {
				"qtcreator_ctrlc_stub.exe" => activity = ActivityKind::Build,
				"python.exe" => {
					if proc.cmd().contains(&"update_to_revisions.py".to_owned()) {
						activity = ActivityKind::UpdateToRevision;
					} else {
						return None;
					}
				}

				"jinnee-utility.exe" => {
					if proc.cmd().contains(&"--deploy_stand".to_owned()) {
						activity = ActivityKind::Deploy;
					} else {
						return None;
					}
				}
				&_ => return None,
			}

			Some(ProcessActivity {
				pid: proc.pid(),
				activity,
			})
		})
		.collect()
}
