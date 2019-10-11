use std::io;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
use std::env;

mod chip8;
use chip8::Chip8;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut file_name = String::from("");
    if args.len() == 2 {
        file_name.push_str(&args[1][..]);
    } else {
        file_name.push_str("assets/input.ch8");
    }
    let input_path_buf = env::current_dir().unwrap().join(Path::new(&file_name));
    let input_path = Path::new(&input_path_buf);

    let mut f = File::open(input_path).unwrap();
    let mut rom = Vec::new();
    f.read_to_end(&mut rom)?;

    let mut chip8 = Chip8::new(&rom);

    chip8.run();

    Ok(())
}