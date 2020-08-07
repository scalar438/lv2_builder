use sysinfo::{ProcessExt, RefreshKind, System, SystemExt};

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
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

pub trait ProcessDescription {
	fn pid(&self) -> &sysinfo::Pid;
	fn activity_kind(&self) -> &ActivityKind;
	fn description(&self) -> &str;
}

pub struct ProcessDescriptionWithPid {
	pub pid: sysinfo::Pid,
	pub description: ProcessDescriptionData,
}

pub struct ProcessDescriptionData {
	pub activity: ActivityKind,
	pub description_text: String,
}

impl ProcessDescription for ProcessDescriptionWithPid {
	fn pid(&self) -> &sysinfo::Pid {
		&self.pid
	}
	fn activity_kind(&self) -> &ActivityKind {
		&self.description.activity
	}
	fn description(&self) -> &str {
		&self.description.description_text
	}
}

fn get_process_description(proc: &sysinfo::Process) -> Option<ProcessDescriptionData> {
	let name = proc.name();
	let cmd = proc.cmd();
	if name.contains("qtcreator_ctrlc_stub") {
		return Some(ProcessDescriptionData {
			activity: ActivityKind::Build,
			description_text: "".to_owned(),
		});
	}

	if name.contains("python") && cmd.iter().any(|a| a.contains("update_to_revisions.py")) {
		return Some(ProcessDescriptionData {
			activity: ActivityKind::UpdateToRevision,
			description_text: "".to_owned(),
		});
	}

	if name.contains("jinnee-utility") && cmd.contains(&"--deploy_stand".to_owned()) {
		return Some(ProcessDescriptionData {
			activity: ActivityKind::Deploy,
			description_text: "".to_owned(),
		});
	}
	None
}

pub fn get_activity_list() -> Vec<impl ProcessDescription> {
	let sys = System::new_with_specifics(RefreshKind::new().with_processes());

	sys.get_processes()
		.iter()
		.filter_map(|(_, proc)| {
			if let Some(data) = get_process_description(proc) {
				Some(ProcessDescriptionWithPid {
					pid: proc.pid(),
					description: data,
				})
			} else {
				None
			}
		})
		.collect()
}
