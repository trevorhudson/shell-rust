use std::io::{self, Write};
use std::path::Path;
use std::{env, path::PathBuf, process::Command};

use is_executable::IsExecutable;

fn main() -> Result<(), anyhow::Error> {
    loop {
        let mut command = String::new();

        print!("$ ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut command)?;

        let trimmed = command.trim();
        // Note: this will not perform great under all conditions.
        // will need to be expanded to handle more complex interpolation
        let mut iter = trimmed.split_ascii_whitespace();

        let Some(program) = iter.next() else {
            continue;
        };

        let args: Vec<&str> = iter.collect();

        if is_builtin(program) {
            if program == "exit" {
                break;
            } else if program == "type" {
                let Some(target) = args.first() else {
                    continue;
                };
                if is_builtin(target) {
                    println!("{} is a shell builtin", target);
                } else if let Some(path) = locate_executable(target) {
                    println!("{} is {}", target, path.display());
                } else {
                    println!("{}: not found", target);
                }
            } else if program == "echo" {
                println!("{}", args.join(" "));
            } else if program == "pwd" {
                let path = env::current_dir()?;
                println!("{}", path.display());
            } else if program == "cd" {
                let Some(path) = args.first() else {
                    continue;
                };

                // Handle HOME
                if *path == "~".to_string() {
                    let Some(home_dir) = env::home_dir() else {
                        println!("cd: {}: Home directory not set", path);
                        continue;
                    };

                    let home_str = home_dir.as_path();
                    let directory = Path::new(home_str);
                    env::set_current_dir(directory)?
                } else {
                    let directory = Path::new(path);

                    if directory.exists() {
                        env::set_current_dir(directory)?
                    } else {
                        println!("cd: {}: No such file or directory", directory.display())
                    }
                }
            }
        } else {
            match locate_executable(program) {
                Some(_path) => {
                    let mut c = Command::new(program);
                    c.args(args);
                    c.status()?;
                }
                None => println!("{}: command not found", trimmed),
            }
        }
    }
    Ok(())
}

fn is_builtin(target: &str) -> bool {
    matches!(target, "exit" | "type" | "echo" | "pwd" | "cd")
}

fn locate_executable(argument: &str) -> Option<PathBuf> {
    match env::var_os("PATH") {
        Some(paths) => env::split_paths(&paths).find_map(|p| {
            let joined = p.join(argument);
            joined.is_executable().then_some(joined)
        }),
        None => None,
    }
}
