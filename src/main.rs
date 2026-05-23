use std::{env, process::Command};

use std::{
    io::{self, Write},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{PathBuf, Path},
};

fn main() -> Result<(), anyhow::Error> {
    loop {
        print!("$ ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let trimmed = input.trim();
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
                if *path == "~" {
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

fn locate_executable(command: &str) -> Option<PathBuf> {
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::split_paths(&path).find_map(|dir| {
        let p = dir.join(command);
        (p.is_file()
            && p.metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false))
        .then_some(p)
    })
}
