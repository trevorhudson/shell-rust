#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    loop {
        let mut command = String::new();
        print!("$ ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut command).unwrap();
        command = command.trim().to_string();

        if command == "exit" {
            break;
        } else if command.starts_with("echo ") {
            let arguments = &command[5..];
            println!("{}", arguments);
        } else {
        println!("{}: command not found", command);
        }
    }
}
