use sysinfo::{ProcessRefreshKind, RefreshKind, System};

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ActivityKind {
	Build,
	Deploy,
	UpdateToRevision,
	UpdateModuleManager,
}

impl std::fmt::Display for ActivityKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match *self {
			ActivityKind::Build => write!(f, "Build"),
			ActivityKind::Deploy => write!(f, "Deploy"),
			ActivityKind::UpdateToRevision => write!(f, "Update to revisions"),
			ActivityKind::UpdateModuleManager => write!(f, "Update with module manager"),
		}
	}
}

pub trait ProcessDescription {
	fn pid(&self) -> &sysinfo::Pid;
	fn activity_kind(&self) -> &ActivityKind;
	fn description(&self) -> Option<&str>;
}

pub struct ProcessDescriptionWithPid {
	pid: sysinfo::Pid,
	description: ProcessDescriptionData,
}

struct ProcessDescriptionData {
	activity: ActivityKind,
	description_text: Option<String>,
}

impl ProcessDescription for ProcessDescriptionWithPid {
	fn pid(&self) -> &sysinfo::Pid {
		&self.pid
	}
	fn activity_kind(&self) -> &ActivityKind {
		&self.description.activity
	}
	fn description(&self) -> Option<&str> {
		if let Some(x) = &self.description.description_text {
			Some(&*x)
		} else {
			None
		}
	}
}

fn get_process_description(proc: &sysinfo::Process) -> Option<ProcessDescriptionData> {
	let name = proc.name();
	let cmd = proc.cmd();
	if name.contains("qtcreator_ctrlc_stub") {
		let build_path;
		if let Some(pos) = cmd.iter().position(|r| r == "--build") {
			if cmd.len() > pos + 1 {
				build_path = Some(cmd[pos + 1].clone());
			} else {
				build_path = None;
			}
			return Some(ProcessDescriptionData {
				activity: ActivityKind::Build,
				description_text: build_path,
			});
		} else {
			return None;
		};
	}

	let arg_item = cmd
		.iter()
		.find(|a| a.contains("update_to_revisions.py"))
		.map(|x| x.strip_suffix("online-inside\\update_to_revisions.py"));
	if name.contains("python") && arg_item.is_some() {
		let opt_path = arg_item.unwrap();
		let path;
		if let Some(x) = opt_path {
			path = Some(x.to_string());
		} else {
			path = None;
		}
		return Some(ProcessDescriptionData {
			activity: ActivityKind::UpdateToRevision,
			description_text: path,
		});
	}

	if name.contains("jinnee-utility") && cmd.contains(&"--deploy_stand".to_owned()) {
		return Some(ProcessDescriptionData {
			activity: ActivityKind::Deploy,
			description_text: get_deploy_path(cmd),
		});
	}

	if name.contains("module-manager") {
		// Find the "store" argument
		let descr = {
			if let Some(store_arg) = cmd.iter().enumerate().find(|(_, arg)| {
				return *arg == "--store";
			}) {
				if store_arg.0 + 1 < cmd.len() {
					Some(cmd[store_arg.0 + 1].clone())
				} else {
					None
				}
			} else {
				None
			}
		};

		return Some(ProcessDescriptionData {
			activity: ActivityKind::UpdateModuleManager,
			description_text: descr,
		});
	}

	None
}

// The longest common prefix of two string
fn lcp<'a>(a: &'a str, b: &'a str) -> &'a str {
	for ((i, c1), c2) in a.char_indices().zip(b.chars()) {
		if c1 != c2 {
			return a.get(..i).unwrap();
		}
	}
	if a.len() < b.len() {
		a
	} else {
		b
	}
}

fn pop_char(a: &str) -> &str {
	let mut c = a.chars();
	c.next_back();
	c.as_str()
}

// There is no deploy path in the arguments list. So I need some actions to get it
// The most precise way to get this is to find the longest common prefix of two strings:
// --deploy_stand C:/Saby/deployed_projects/deploy2\config\test.s3deploy
// --logs_dir C:/Saby/deployed_projects/deploy2\logs
// The common prefix is C:/Saby/deployed_projects/deploy2, so it is the deploy path
// It's also possible to take a prefix to any of these strings, but it may be broken if these paths change in the future
fn get_deploy_path(cmd: &[String]) -> Option<String> {
	let mut deploy_stand_idx = None;
	let mut logs_dir_idx = None;
	for arg in cmd.iter().enumerate() {
		if arg.1 == "--deploy_stand" {
			deploy_stand_idx = Some(arg.0);
		} else if arg.1 == "--logs_dir" {
			logs_dir_idx = Some(arg.0);
		}
	}

	let deploy_stand_path = deploy_stand_idx
		.map(|x| {
			if x + 1 < cmd.len() {
				Some(cmd[x + 1].clone())
			} else {
				None
			}
		})
		.unwrap_or(None);

	let logs_dir_path = logs_dir_idx
		.map(|x| {
			if x + 1 < cmd.len() {
				Some(cmd[x + 1].clone())
			} else {
				None
			}
		})
		.unwrap_or(None);

	let r = match (&deploy_stand_path, &logs_dir_path) {
		(Some(a1), Some(a2)) => Some(lcp(&a1, &a2)),

		(None, Some(logs_dir_path)) => logs_dir_path.strip_suffix("logs"),

		(Some(deploy_stand_path), None) => {
			let mut res = None;
			if let Some(stripped) = deploy_stand_path.strip_suffix("config\\test.s3deploy") {
				res = Some(stripped);
			}

			if res.is_none() {
				if let Some(stripped) = deploy_stand_path.strip_suffix("config/test.s3deploy") {
					res = Some(stripped)
				}
			}

			res
		}

		(None, None) => None,
	};

	if let Some(res) = r {
		Some(
			if res.ends_with("\\") || res.ends_with("/") {
				pop_char(res)
			} else {
				res
			}
			.to_owned(),
		)
	} else {
		None
	}
}

pub fn get_activity_list() -> Vec<impl ProcessDescription> {
	let sys = System::new_with_specifics(
		RefreshKind::new()
			.with_processes(ProcessRefreshKind::new().with_cmd(sysinfo::UpdateKind::OnlyIfNotSet)),
	);

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

#[cfg(test)]
mod test {

	use super::*;

	#[test]
	fn test_lcp() {
		assert_eq!(lcp("abcdef", "abcxyz"), "abc");
		assert_eq!(lcp("abcdef", "abcdef"), "abcdef");
		assert_eq!(lcp("abc", "abcdef"), "abc");
		assert_eq!(lcp("abc", "xyz"), "");
		assert_eq!(lcp("", ""), "");
		assert_eq!(lcp("abc", ""), "");
		assert_eq!(lcp("abc", "ab"), "ab");
	}

	#[test]
	fn test_pop_char() {
		assert_eq!(pop_char("abc"), "ab");
		assert_eq!(pop_char("a"), "");
		assert_eq!(pop_char(""), "");
	}
}
