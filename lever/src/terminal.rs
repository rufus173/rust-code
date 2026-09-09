use std::ops::Drop;
use std::io;
use std::io::Write;

pub struct TemporaryOutput {
	line_buffer: Vec<String>,
	current_line: String,
	max_lines: usize
}
impl TemporaryOutput {
	pub fn println(&mut self, text: impl AsRef<str>){
		self.print(text.as_ref().to_string()+"\n");
	}
	pub fn print(&mut self, text: impl AsRef<str>){
		//====== count newlines ======
		let text = text.as_ref();
		let mut old_current_line = self.current_line.clone();
		let old_line_count = self.line_buffer.len();
		//add output to the line buffer
		let mut lines: Vec<_> = text
			.split('\n')
			.collect();
		//====== update the line buffer ======
		//extend the current line buffer
		if let Some(incomplete_line) = lines.pop(){
			self.current_line.push_str(incomplete_line);
		}
		//print the first line
		if lines.len() > 0 {
			//new line means flush the current_line buffer
			old_current_line.push_str(lines.remove(0));
			self.line_buffer.push(old_current_line);
			self.current_line.truncate(0);
		}
		//print any other lines
		let mut cloned_lines: Vec<_> = lines
			.into_iter()
			.map(String::from)
			.collect();
		self.line_buffer.append(&mut cloned_lines);
		//remove previous lines if out of space
		while self.line_buffer.len() > self.max_lines {self.line_buffer.remove(0);}
		//====== actually draw to terminal ======
		//make space
		print!("{}","\x1bM\x1b[2K".repeat(old_line_count));
		//print the text again
		self.line_buffer
			.iter()
			.for_each(|line| println!("{}",line));
		//flush stdout
		let _ = io::stdout().flush();
	}
	pub fn new(max_lines: usize) -> TemporaryOutput {
		TemporaryOutput {
			current_line: String::new(),
			line_buffer: vec![],
			max_lines,
		}
	}
	pub fn clear(&self){
		for _ in 0..(self.line_buffer.len()){
			print!("\x1bM");
			print!("\x1b[2K");
		}
		let _ = io::stdout().flush();
	}
}
