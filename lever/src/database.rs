use std::fs::{File};
use std::io::Error;
use LeverFile;
use std::convert::TryInto;
use std::io;
use std::path::{PathBuf,Path};
use std::io::Read;

#[derive(Debug)]
pub struct LeverDB {
	pub installed_packages: Vec<String>, //(name,repo_location)
	pub compiled_packages: Vec<String>, //(name,repo_location)
	pub tracked_packages: Vec<(String,String)>, //(name,repo_location)
	pub db_path: PathBuf,
}
pub struct PackageTreeNode {
	pub dependencies: Vec<PackageTreeNode>,
	pub name: String,
}

impl LeverDB {
	pub fn load<T: AsRef<Path>>(path: T) -> io::Result<Self> {
		//defaults
		let mut installed_packages = vec![];
		let mut compiled_packages = vec![];
		let mut tracked_packages = vec![];
		//create if it doesnt exist
		if !path.as_ref().exists() {File::create(&path)?;}
		//load file
		let mut database = String::new();
		let _ = File::open(&path)?.read_to_string(&mut database);
		//====== read the leverfile line by line ======
		let mut section_name = String::from("");
		for line in database.split('\n') {
			//skip empty lines
			if line.len() == 0 {continue}
			//sections
			if line.chars().next() == Some('[') {
				section_name = line[1..].trim_end_matches(']').into();
				continue;
			}
			match section_name.as_str() {
				"installed" => {
					//simply the name of the packages
					installed_packages.push(line.trim().to_string());
				},
				"compiled" => {
					//also just the name
					compiled_packages.push(line.trim().to_string());
				},
				"tracked" => {
					//split by the deliminator "=>"
					let Ok([name,path]): Result<[&str;2],Vec<&str>> = line
						.split("=>")
						.collect::<Vec<_>>()
						.try_into() 
					else {continue}; //skip in line is invalid
					tracked_packages.push((name.trim().to_string(),path.trim().to_string()));
				},
				_ => (),
			}
		}
		Ok(Self {
			installed_packages,
			tracked_packages,
			compiled_packages,
			db_path: path.as_ref().to_owned(),
		})
	}
	pub fn save(&self) -> io::Result<()>{
		let mut database_content = vec![];
		//====== tracked packages ======
		database_content.push(String::from("[tracked]"));
		for (name,location) in &self.tracked_packages {
			database_content.push(name.to_owned() + "=>" + &location);
		}
		//====== compiled packages ======
		database_content.push(String::from("[compiled]"));
		for name in &self.compiled_packages {
			database_content.push(name.to_owned());
		}
		//====== installed packages ======
		database_content.push(String::from("[installed]"));
		for name in &self.installed_packages {
			database_content.push(name.to_owned());
		}
		std::fs::write(&self.db_path,
			database_content
			.into_iter()
			.fold(String::new(),|string,line| string + &line + "\n") //fold into one long string
		)
	}
	pub fn get_package_location(&self,name_query: &str) -> Option<String> {
		self.tracked_packages.clone()
			.into_iter()
			.find(|(name,_)| name == name_query)
			.map(|(_,location)| location)
	}
	pub fn compiled_packages(&self) -> Vec<String> {
		self.compiled_packages.clone()
	}
	pub fn installed_packages(&self) -> Vec<String> {
		self.installed_packages.clone()
	}
	pub fn add_tracked(&mut self, leverfile: &LeverFile) -> io::Result<()> {
		if self.tracked_packages
			.iter()
			.any(|(name,_)| *name == leverfile.name()){
				return Err(Error::other(format!("package \"{}\" already tracked",leverfile.name())))
		}
		self.tracked_packages.push((leverfile.name().into(),leverfile.absolute_path().display().to_string()));
		Ok(())
	}
	pub fn remove_tracked(&mut self, package_name: &str) -> io::Result<()> {
		let old_package_count = self.tracked_packages.len();
		self.tracked_packages = self.tracked_packages
			.clone()
			.into_iter()
			.filter(|(name,_)| name != package_name)
			.collect();
		if self.tracked_packages.len() != old_package_count {Ok(())}
		else {Err(io::Error::other(format!("Package \"{}\" not found in database",package_name)))}
	}
	pub fn add_installed(&mut self,package_name: &str) -> io::Result<()> {
		if self.installed_packages
			.iter()
			.any(|name| *name == package_name){
				return Err(Error::other("package already installed"))
		}
		self.installed_packages.push(package_name.to_owned());
		Ok(())
	}
	pub fn add_compiled(&mut self,package_name: &str) -> io::Result<()> {
		if self.compiled_packages
			.iter()
			.any(|name| *name == package_name){
				return Err(Error::other("package already compiled"))
		}
		self.compiled_packages.push(package_name.to_owned());
		Ok(())
	}
	pub fn get_package_leverfile(&self, name: impl AsRef<str>) -> io::Result<LeverFile> {
		let name = name.as_ref();
		let Some(leverfile_path) = self.get_package_location(name)
		else {
			return Err(io::Error::other(format!("Package \"{name}\" not known to lever")));
		};
		LeverFile::load(&leverfile_path)
	}
	pub fn generate_package_dependency_tree(&self, name: impl AsRef<str>) -> io::Result<PackageTreeNode>{
		let name = name.as_ref();
		let leverfile = self.get_package_leverfile(name)?;
		let mut dependency_nodes = vec![];
		for package in leverfile.dependencies(){
			//recursion gaming
			dependency_nodes.push(self.generate_package_dependency_tree(&package)?);
		}
		Ok(PackageTreeNode {
			name: name.to_string(),
			dependencies: dependency_nodes,
		})
	}
}

impl PackageTreeNode {
	pub fn flatten(&self) -> Vec<String> {
		//flatten to just pattern names
		if self.dependencies.len() == 0 {
			vec![self.name.clone()]
		}else {
			let mut result = vec![];
			for dependency in &self.dependencies {
				result.append(&mut dependency.flatten())
			}
			result.push(self.name.clone());
			result
		}
	}
	pub fn print(&self, highlight_package: Option<&str>){
		//just a wrapper to make it nicer to call
		self.print_recursive(String::new(),highlight_package);
	}
	pub fn print_recursive(&self, indent: String, highlight_package: Option<&str>){
		//check if node is hilighted
		let highlight = highlight_package.map(|n| n == self.name);
		let start_escape_sequence = if let Some(true) = highlight {
			"\x1b[32m"
		}else {
			""
		};
		//print current node
		print!("{}",indent);
		print!("{}",start_escape_sequence);
		print!("{}",self.name);
		println!("\x1b[39m");
		//recursive step to dependencies
		for (i,dependency) in self.dependencies.iter().enumerate() {
			//this works because... uhhhh... i just had an intuition and it worked?
			let new_indent = indent
				.replace("└"," ")
				.replace("─"," ")
				.replace("├","│")
				+ if i == self.dependencies.len()-1 {"└"} else {"├"}
				+ "─";
			dependency.print_recursive(new_indent,highlight_package);
		}
	}
}
