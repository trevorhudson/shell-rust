#[allow(unused_imports)]
use std::io::{self, Write};
use std::{env, path::PathBuf};

use is_executable::IsExecutable;

// Now let's refactor to make this actually functional!

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

            match locate_executable(argument) {
                None => println!("{}: not found", argument),
                Some(val) => {
                    println!("{}: found", val.display());
                    if let Some(arguments) = command.strip_suffix(argument) {
                        println!("{}: found", arguments)
                    }
                    // let new_command = Command::new(val).args(args)
                }
            };
        } else if let Some(arguments) = command.strip_prefix("echo ") {
            println!("{}", arguments);
        } else {
            println!("{}: command not found", command);
        }
    }
}

fn locate_executable(argument: &str) -> Option<PathBuf> {
    match env::var_os("PATH") {
        Some(paths) => {
            // find first executable path
            env::split_paths(&paths).find_map(|p| {
                let joined = p.join(argument);
                joined.is_executable().then_some(joined)
            })
        }
        None => None,
    }
}
