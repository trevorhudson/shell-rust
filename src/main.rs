#[allow(unused_imports)]
use std::io::{self, Write};
use std::{env, os::unix::process::CommandExt, path::PathBuf, process::Command};

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

        let mut iter = command.trim().split_ascii_whitespace();
        let Some(program) = iter.next() else {
            continue;
        };

        if program == "exit" {
            break;
        }

        match locate_executable(program) {
            Some(path) => {
                let mut c = Command::new(path);
                println!("{:?}", c.get_program());
                c.args(iter);
                c.status()?;
            }
            None => println!("{}: command not found", command),
        }
    }
    Ok(())
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
