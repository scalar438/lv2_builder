use sysinfo::{ProcessExt, RefreshKind, System, SystemExt};

#[derive(Debug, Eq, PartialEq, Hash)]
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
	pub pid: sysinfo::Pid,
	pub activity: ActivityKind,
}

fn get_process_activity(proc: &sysinfo::Process) -> Option<ActivityKind> {
	let name = proc.name();
	let cmd = proc.cmd();
	if name.contains("qtcreator_ctrlc_stub") {
		return Some(ActivityKind::Build);
	}

	if name.contains("python") && cmd.contains(&"update_to_revisions.py".to_owned()) {
		return Some(ActivityKind::UpdateToRevision);
	}

	if name.contains("jinnee-utility") && cmd.contains(&"--deploy_stand".to_owned()) {
		return Some(ActivityKind::Deploy);
	}
	None
}

pub fn get_activity_list() -> Vec<ProcessActivity> {
	let sys = System::new_with_specifics(RefreshKind::new().with_processes());

	sys.get_processes()
		.iter()
		.filter_map(|(_, proc)| {
			if let Some(activity) = get_process_activity(proc) {
				Some(ProcessActivity {
					pid: proc.pid(),
					activity,
				})
			} else {
				None
			}
		})
		.collect()
}
