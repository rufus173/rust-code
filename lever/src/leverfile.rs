use std::path::{PathBuf,Path};
use terminal::TemporaryOutput;
use std::path;
use std::process::{Stdio,Command};
use std::io::Read;
use std::fs::File;
use std::fs;
use std::io;

#[derive(Debug)]
pub struct LeverFile {
	url: Option<String>,
	compile_commands: Vec<String>,
	install_commands: Vec<String>,
	dependencies: Vec<String>,
	absolute_path: PathBuf,
	name: String,
}

pub const LEVERFILE_DEFAULT_NAME: &str = "leverfile.ini";
const LEVERFILE_TEMPLATE: &str = "\
# Template leverfile

[name]
#example package name

#uncomment to set url
#[url]
#this can be left blank

[compile]
#add compile commands here

[dependencies]
#add leverfile package names here as dependencies
#example-package
";

impl LeverFile {
	pub fn load<T: AsRef<Path>>(path: T) -> io::Result<Self> {
		//if directory is given use leverfile.ini as a default leverfile
		let leverfile_path = if path.as_ref().is_dir() {
			path.as_ref().join("leverfile.ini")
		}else {
			path.as_ref().to_path_buf()
		};
		//defaults
		let mut url = None;
		let mut name = String::new();
		let mut compile_commands = vec![];
		let mut install_commands = vec![];
		let mut dependencies = vec![];
		//load file
		let leverfile = fs::read_to_string(&leverfile_path)?;
		//====== read the leverfile line by line ======
		let mut section_name = String::from("global");
		for (_,raw_line) in leverfile.split('\n').into_iter().enumerate() {
			//remove comments
			let line = if let Some(index) = raw_line.find('#') {
				raw_line[..index].to_string()
			}else {raw_line.to_string()};
			//skip empty lines
			if line.len() == 0 {continue}
			//sections
			if line.chars().next() == Some('[') {
				section_name = line[1..].trim_end_matches(']').into();
				continue;
			}
			match section_name.as_str() {
				"url" => {
					url = Some(line.into())
				},
				"install" => {
					install_commands.push(line.into())
				},
				"compile" => {
					compile_commands.push(line.into())
				},
				"name" => {
					name = line.into();
				},
				"dependencies" => {
					dependencies.push(line.into())
				}
				_ => {
					return Err(io::Error::other(format!("Unknown leverfile section {section_name:?} at line {line}")));
				},
			}
		}
		//check for required fields
		if name.len() == 0 {
			return Err(io::Error::other("Leverfile missing required section: \"name\""));
		}
		//pack struct
		Ok(Self {
			url,
			compile_commands,
			install_commands,
			dependencies,
			name,
			absolute_path: path::absolute(leverfile_path)?,
		})
	}
	//TODO we could definitely save path so we dont have to pass git_repo_path
	pub fn compile<T: AsRef<Path>>(&self, git_repo_path: T) -> io::Result<()>{
		run_commands(self.compile_commands.clone(),git_repo_path)
	}
	pub fn install<T: AsRef<Path>>(&self, git_repo_path: T) -> io::Result<()>{
		run_commands(self.install_commands.clone(),git_repo_path)
	}
	pub fn dependencies(&self) -> Vec<String> {
		self.dependencies.clone()
	}
	pub fn write_template(path: impl AsRef<Path>) -> io::Result<()> {
		fs::write(path,LEVERFILE_TEMPLATE)
	}
	pub fn absolute_path(&self) -> &Path {
		&self.absolute_path
	}
	pub fn name(&self) -> &str {
		&self.name
	}
}

fn run_commands<T: AsRef<Path>>(commands: Vec<String>, execution_dir: T) -> io::Result<()>{
	let mut temp_output = TemporaryOutput::new(8);
	for command in commands {
		temp_output.println(format!("> {command}"));
		//start a shell to execute each line
		let mut child = Command::new("sh")
			.arg("-c")
			.arg(&command)
			.current_dir(&execution_dir)
			.stderr(Stdio::piped())
			.stdout(Stdio::piped())
			.stdin(Stdio::null())
			.spawn()?;
		//read and print stdout from shell
		if let Some(ref mut stdout) = child.stdout {
			let mut line_buffer = String::new();
			loop {
				//read some data
				let mut stdout_buffer = [0; 64];
				let count = stdout.read(&mut stdout_buffer)?;
				if count == 0 {break}
				//output in a nice format
				for byte in &stdout_buffer[..count] {
					let ch = char::from(*byte);
					if ch == '\n' {
						temp_output.println(format!("==> {}",line_buffer));
						line_buffer.truncate(0);
					}else {
						line_buffer.push(ch);
					}
				}
			}
		}
		//once stdout closes print anything on stderr
		if let Some(ref mut stderr) = child.stderr {
			let mut line_buffer = String::new();
			loop {
				//read some data
				let mut stderr_buffer = [0; 64];
				let count = stderr.read(&mut stderr_buffer)?;
				if count == 0 {break}
				//output in a nice format
				for byte in &stderr_buffer[..count] {
					let ch = char::from(*byte);
					if ch == '\n' {
						temp_output.println(format!("-e-> {}",line_buffer));
						line_buffer.truncate(0);
					}else {
						line_buffer.push(ch);
					}
				}
			}
		}
		let exit_status = child.wait()?;
		if !exit_status.success() {
			eprintln!("Process exited with an error");
			match exit_status.code() {
				Some(code) => println!("{command:?} exited with code {code}",),
				None => println!("{command:?} was terminated by signal",),
			}
			return Err(io::Error::other("Command Failed"))
		}
	}
	temp_output.clear();
	Ok(())
}
