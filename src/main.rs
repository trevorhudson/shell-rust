#[allow(unused_imports)]
use std::io::{self, Write};
use std::{env, path::PathBuf};

use is_executable::IsExecutable;

// Now let's refactor to make this actually functional!
// implement better control flow for commands
//
fn main() -> Result<(), anyhow::Error> {
    loop {
        let mut command = String::new();

        print!("$ ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut command)?;

        let trimmed = command.trim();
        let mut iter = trimmed.split_ascii_whitespace();

        let Some(program) = iter.next() else {
            continue;
        };

        let args: Vec<&str> = iter.collect();

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
        } else {
            println!("{}: command not found", program);
        }
    }
    Ok(())
}

fn is_builtin(target: &str) -> bool {
    target == "exit" || target == "type" || target == "echo"
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

// match locate_executable(program) {
//     Some(path) => {
//         let mut c = Command::new(path);
//         c.args(iter);
//         c.status()?;
//     }
//     None => println!("{}: command not found", trimmed),
// }
