#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    let mut command = String::new();
    io::stdin().read_line(&mut command).unwrap();
    println!("{}: command not found", command.trim());
}
