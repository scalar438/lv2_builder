use sysinfo::{ProcessRefreshKind, RefreshKind, System};

#[derive(Debug, Eq, PartialEq, Clone)]
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
	pid: sysinfo::Pid,
	description: ProcessDescriptionData,
}

struct ProcessDescriptionData {
	activity: ActivityKind,
	description_text: String,
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
		let build_path;
		if let Some(pos) = cmd.iter().position(|r| r == "--build") {
			if cmd.len() > pos + 1 {
				build_path = cmd[pos + 1].clone();
			} else {
				build_path = "".to_owned();
			}
		} else {
			build_path = "".to_owned()
		}
		return Some(ProcessDescriptionData {
			activity: ActivityKind::Build,
			description_text: build_path,
		});
	}

	let arg_item = cmd
		.iter()
		.find(|a| a.contains("update_to_revisions.py"))
		.map(|x| x.strip_suffix("online-inside\\update_to_revisions.py"));
	if name.contains("python") && arg_item.is_some() {
		let opt_path = arg_item.unwrap();
		let path;
		if let Some(x) = opt_path {
			path = x.to_string();
		} else {
			path = "".to_owned();
		}
		return Some(ProcessDescriptionData {
			activity: ActivityKind::UpdateToRevision,
			description_text: path,
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
	let sys =
		System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));

	sys.processes()
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
