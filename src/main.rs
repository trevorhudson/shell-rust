#[allow(unused_imports)]
use std::io::{self, Write};
use std::{env};

use is_executable::IsExecutable;

fn main() {
    loop {
        let mut command = String::new();
        print!("$ ");
        io::stdout().flush().unwrap();
        io::stdin().read_line(&mut command).unwrap();
        command = command.trim().to_string();

        if command == "exit" {
            break;
        } else if let Some(argument) = command.strip_prefix("type ") {
            if argument == "echo" || argument == "exit" || argument == "type" {
                println!("{} is a shell builtin", argument);
                continue;
            }

            for path in env::split_paths(&env!("PATH")) {
                if path.is_executable() {
                    println!("{} is {:?}", argument, path);
                    continue;
                }
            }

            println!("{}: not found", argument);

        } else if let Some(arguments) = command.strip_prefix("echo ") {
            println!("{}", arguments);
        } else {
            println!("{}: command not found", command);
        }
    }
}
