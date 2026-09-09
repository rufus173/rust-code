mod leverfile;
mod terminal;
mod database;

use std::process::Command;
use terminal::TemporaryOutput;
use leverfile::{LeverFile,LEVERFILE_DEFAULT_NAME};
use std::path::{Path,PathBuf};
use database::{LeverDB,PackageTreeNode};
use std::env;
use std::process::exit;
use std::fs::read_to_string;
use std::fs;
use std::io;

#[derive(Clone)]
pub struct Config {
	pub database_path: PathBuf,
	pub editor: String,
}

const DEFAULT_CONFIG: &str = "\
# This is the default configuration file
# Set options here
# key=value
# database_path=/path/to/config #defaults to ~/.config/lever/packages.ini
editor=vim
";

impl Config {
	pub fn load<P: AsRef<Path> + std::fmt::Debug + Clone>(config_path: Option<P>) -> Self {
		//====== initialise defaults ======
		let mut config = Self {
			database_path: env::home_dir()
				.map(|p| p.join(".config/lever/packages.ini"))
				.unwrap_or(PathBuf::from("/etc/lever/packages.ini")),
			editor: String::new(),
		};
		//====== load the config file if it exists ======
		let config_file_lines = config_path.clone()
			.map(|p| read_to_string(p).ok()) //read the config file and ignore errors
			.flatten()
			.map(|s| s
				.split('\n') //split file into lines
				.map(|x| match x.find('#') {
					Some(index) => x.chars().take(index).collect::<String>(), //remove comments
					None => x.to_string(),
				})
				.collect::<Vec<_>>() //wrap up into a nice Vec<String>
			);
		//====== read the config file if it exists ======
		if let Some(lines) = config_file_lines {
			for (line_number,line) in lines.into_iter().enumerate().filter(|(_,l)| l.len() != 0) {
				let Some((key,value)) = line.split_once('=') else {
					eprintln!("Malformed config in config file {:}:{}",config_path.as_ref().unwrap().as_ref().display(),line_number+1);
					continue
				};
				match key.trim() {
					"database_path" => {config.database_path = value.trim().into()},
					"editor" => {config.editor = value.trim().into()},
					_ => eprintln!("Unknown config option {:?} in {:}:{}",key,config_path.as_ref().unwrap().as_ref().display(),line_number+1)
				}
			}
		}
		//====== return the final config ======
		config
	}
	pub fn write_default(config_path: impl AsRef<Path>) -> io::Result<()> {
		fs::write(config_path,DEFAULT_CONFIG)
	}
}

fn main(){
	//====== load config ======
	let config_path = env::home_dir()
		.map(|p| p.join(".config/lever/lever.conf"))
		.unwrap_or(PathBuf::from("/etc/lever/lever.conf"));
	//create it if it doesnt exist
	if !config_path.exists(){
		if let Err(e) = Config::write_default(&config_path){
			eprintln!("Unable to write default config file to {}: {e}",config_path.display());
			exit(1)
		}
	}
	//load the config
	let config = Config::load(Some(config_path));
	//====== load the database ======
	let mut database = match LeverDB::load(&config.database_path) {
		Ok(db) => db,
		Err(e) => {
			eprintln!("Error loading lever database ({:?}):",&config.database_path);
			eprintln!("{e:?}");
			exit(1)
		}
	};
	//====== handle command line ======
	let command_line = env::args()
		.collect::<Vec<_>>()[1..]
		.to_owned();
	let Ok(_) = (match command_line.iter().map(|s| s.as_str()).next() {
		Some("compile") => compile(command_line[1..].into(),&config,&mut database),
		Some("install") => install(command_line[1..].into(),&config,&mut database),
		Some("update") => update(command_line[1..].into(),&config,&mut database),
		Some("track") => track(command_line[1..].into(),&config, &mut database),
		Some("untrack") => untrack(command_line[1..].into(),&config, &mut database),
		Some("create") => create(command_line[1..].into(),&config,&mut database),
		Some("help") => Ok(help()),
		Some(command) => {
			eprintln!("Unknown command {command:?}");
			Err(())
		},
		None => {
			eprintln!("Expected command as first argument.");
			Err(())
		},
	}) else {exit(1)};
}

fn untrack(targets: Vec<String>, config: &Config, database: &mut LeverDB) -> Result<(),()> {
	//load current directory package if none provided
	let packages = if targets.len() == 0 {
		let leverfile = match LeverFile::load(LEVERFILE_DEFAULT_NAME){
			Ok(leverfile) => leverfile,
			Err(e) => {
				eprintln!("Error loading leverfile in current directory: {e}");
				return Err(());
			}
		};
		vec![leverfile.name().to_string()]
	}else {targets.clone()};
	//untrack each package
	for package in packages {
		if let Err(e) = database.remove_tracked(&package){
			eprintln!("Error removing \"{}\" from tracked list: {e}",package);
		}
	}
	if let Err(e) = database.save(){
		eprintln!("Error saving database: {e}");
		return Err(());
	}
	Ok(())
}
fn track(targets: Vec<String>, config: &Config, database: &mut LeverDB) -> Result<(),()> {
	//no args provided means track leverfile in current dir
	let targets = if targets.len() == 0 {
		vec![String::from(LEVERFILE_DEFAULT_NAME)]
	}else {targets};
	//track all the files
	for file in targets {
		//
		let leverfile = match LeverFile::load(&file){
			Ok(leverfile) => leverfile,
			Err(e) => {
				eprintln!("Error loading leverfile {:?}: {e}",file);
				continue;
			}
		};
		database.add_tracked(&leverfile).map_err(|e| eprintln!("Error tracking {} in database: {e}",file))?;
	}
	database.save().map_err(|e| {
		eprintln!("Error saving database: {e}");
		() //remove the Err content as we have already printed it
	})
}

//TODO: create a template leverfile in current dir, open in editor and track after it is closed
fn create(command_line: Vec<String>, config: &Config, database: &mut LeverDB) -> Result<(),()>{
	let leverfile_path = Path::new("leverfile.ini");
	//write the template if a leverfile doesnt already exist
	if !leverfile_path.exists(){
		//write the template
		if let Err(e) = LeverFile::write_template(leverfile_path){
			eprintln!("Failed to write leverfile template: {e}");
			return Err(())
		}
	}else {
		println!("Leverfile exists, opening...");
	}
	//open it in the editor for ease of user use
	if config.editor.len() != 0 {
		let result = Command::new(&config.editor)
			.args([leverfile_path.to_string_lossy().into_owned()])
			.status();
		match result {
			Err(e) => eprintln!("Failed to open editor: {e}"),
			Ok(s) if !s.success() => eprintln!("Editor exited with error: {s}"),
			Ok(_) => (),
		}
	}
	//validate it
	let leverfile = match LeverFile::load(leverfile_path){
		Ok(leverfile) => leverfile,
		Err(e) => {
			eprintln!("Leverfile invalid so not tracked: {e}");
			Err(())?
		}
	};
	//track if not already tracked
	if database.get_package_location(leverfile.name()).is_none(){
		if let Err(e) = database.add_tracked(&leverfile){
			eprintln!("Error tracking in database: {e}");
			return Err(());
		}
		if let Err(e) = database.save(){
			eprintln!("Error saving database: {e}");
			return Err(());
		}
		println!("{} tracked",leverfile.name());
	}
	Ok(())
}

fn compile(targets: Vec<String>, config: &Config, database: &mut LeverDB) -> Result<(),()> {
	let compile_queue = if targets.len() == 0 {database.installed_packages()}
		else {targets};
	//====== compile all the selected packages ======
	for package in compile_queue {
		//get the path to the leverfile
		let Some(leverfile_path) = database.get_package_location(&package)
		else {
			eprintln!("Couldn't find package {package:?}, skipping");
			continue;
		};
		//load the leverfile
		println!("----> Compiling {:?}",package);
		let leverfile = match database.get_package_leverfile(&package) {
			Ok(lf) => lf,
			Err(error) => {
				eprintln!("Loading leverfile failed: {error}");
				return Err(())
			}
		};
		//determine the compile dir
		let Some(compile_dir) = Path::new(&leverfile_path).parent()
		else {
			eprintln!("Could not determine compile directory.");
			return Err(());
		};
		//actualy compile
		match leverfile.compile(compile_dir) {
			Ok(_) => (),
			Err(e) => {
				eprintln!("Compilation error: {e:?}");
				return Err(());
			}
		};
		print!("\x1bM\x1b[2K");//replace the previous compiling text
		println!("----> Compiled {:?} without errors.",package);
		//log that it has been compiled
		if let Ok(_) = database.add_compiled(&package) {
			let _ = database.save();
		}
	}
	Ok(())
}
fn install(targets: Vec<String>, config: &Config, database: &mut LeverDB) -> Result<(),()> {
	//====== install all if no targets are provided ======
	let mut install_queue = targets.clone();
	if targets.len() == 0 {
		let mut all_installed_packages = database.installed_packages();
		install_queue.append(&mut all_installed_packages);
	}
	//====== generate dependency tree ======
	let mut install_tree = PackageTreeNode {
		dependencies: vec![],
		name: String::from("Selected Packages"),
	};
	for package in install_queue {
		let packge_dependency_tree = match database.generate_package_dependency_tree(&package){
			Ok(tree) => tree,
			Err(e) => {
				eprintln!("Error finding dependencies for \"{package}\": {e}");
				return Err(());
			}
		};
		install_tree.dependencies.push(packge_dependency_tree);
	}
	//TODO: remove duplicates
	let mut install_queue = install_tree.flatten();
	install_queue.pop(); //remove "Selected Packages" as it is only used to make the tree look nice
	//====== install selected packages ======
	for package in install_queue {
		//get the leverfile path
		let Some(leverfile_path) = database.get_package_location(&package)
		else {
			eprintln!("Could not find package {package:?}, skipping");
			continue;
		};
		println!("\n------------ {} ------------",package);
		install_tree.print(Some(&package));
		//compile if not already compiled
		//if let None = database.compiled_packages()
		//	.into_iter()
		//	.find(|name| *name == package){
		//		println!("----> {package:?} Not already compiled, compiling.");
		//		let _ = compile(vec![package.clone()],config,database)?;
		//}
		compile(vec![package.clone()],config,database)?;
		println!("----> Installing {:?}",package);
		//load the leverfile
		let Ok(leverfile) = LeverFile::load(&leverfile_path) else {
			eprintln!("Loading leverfile at {leverfile_path:?} failed.");
			return Err(())
		};
		//determine the compile dir
		let Some(compile_dir) = Path::new(&leverfile_path).parent()
		else {
			eprintln!("Could not determine compile directory.");
			return Err(());
		};
		//actualy install
		match leverfile.install(compile_dir) {
			Ok(_) => (),
			Err(e) => {
				eprintln!("Install error: {e:?}");
				return Err(());
			}
		};
		print!("\x1bM\x1b[2K");//replace the previous compiling text
		println!("----> Installed {:?} without errors.",package);
		//track that the package has now been installed
		if let Ok(_) = database.add_installed(&package) {
			let _ = database.save();
		}
	}
	println!("\n----> All packages installed successfully");
	//TODO: handle git clone if path to leverfile provided
	Ok(())
}
fn update(targets: Vec<String>, config: &Config, database: &mut LeverDB) -> Result<(),()> {
	for target in targets {
		compile(vec![target.clone()],config,database)?;
		install(vec![target.clone()],config,database)?;
	}
	Ok(())
}
fn help(){
	let name = env::args().next().expect("argv[0] nonexistent");
	println!("Lever is a git based package manager designed to help you manage compiling from source.");
	println!("");
	println!("Usage: {name} <command> [options]");
	println!("Commands:");
	println!("--> help");
	println!("Shows this help text. takes no arguments");
	println!("--> pull");
	println!("Pass it the name of packages to update via git pull. Passing nothing will cause it to update all installed packages.");
	println!("--> compile");
	println!("Pass it the name of packages to compile. Passing nothing will cause it to compile all installed packages.");
	println!("--> install");
	println!("Pass it the name of packages to install. Passing nothing will cause it to reinstall all already installed packages.");
	println!("Passing a path to a leverfile will cause it to move it to the default folder, clone the repo, track it and then install it");
	println!("--> update");
	println!("Pass it the name of packages to pull, compile then install. Passing nothing will cause it to act on all packages installed.");
	println!("--> track");
	println!("Pass it the path of a leverfile(s) to track");
	println!("--> untrack");
	println!("removes all references for the provided packages from the database");
	println!("--> create");
	println!("Creates a leverfile in the current directory, and opens it in an editor for you to fill in")
}
