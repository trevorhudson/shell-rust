use std::{
    collections::HashMap,
    env, fs,
    io::{self, Write},
    ops::ControlFlow,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
};

use crate::BUILTINS;
use crate::parse::{Command, CompleteOp, Fd, Mode, ParsedLine, Redirect};

#[derive(Debug)]
enum Running {
    True,
    False,
}

#[derive(Debug)]
pub struct Job {
    id: usize,
    command: String,
    child: std::process::Child,
}

/// Write to a file, or print
fn write_to(content: &str, redirect: Option<&Redirect>, default: Fd) -> io::Result<()> {
    match redirect {
        Some(r) => {
            let mut file = open_for(r)?;
            writeln!(file, "{content}")?;
            Ok(())
        }
        None => {
            match default {
                Fd::Stdout => {
                    println!("{content}")
                }
                Fd::Stderr => {
                    eprintln!("{content}")
                }
            }
            Ok(())
        }
    }
}

/// Check if a command is a built in
fn is_builtin(target: &str) -> bool {
    BUILTINS.contains(&target)
}

/// Opens a file to truncate or append
fn open_for(r: &Redirect) -> io::Result<fs::File> {
    match r.mode {
        Mode::Truncate => fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&r.path),
        Mode::Append => fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&r.path),
    }
}

/// Walks PATH and returns sorted executable names
pub fn collect_executables() -> Vec<String> {
    let path = std::env::var("PATH").unwrap_or_default();

    let mut names: Vec<String> = Vec::new();
    for dir in std::env::split_paths(&path) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if !is_executable(&entry.path()) {
                continue;
            }
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }

    names.sort();
    names.dedup();
    names
}

/// Find a path for an executable
fn locate_executable(command: &str) -> Option<PathBuf> {
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::split_paths(&path).find_map(|dir| {
        let p = dir.join(command);
        is_executable(&p).then_some(p)
    })
}

/// Checks whether a path is a file and is executable
fn is_executable(path: &Path) -> bool {
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Replaces a leading ~ with the path of HOME
fn expand_tilde(token: String) -> String {
    let Ok(home) = std::env::var("HOME") else {
        return token;
    };
    if token == "~" {
        return home;
    }
    if let Some(rest) = token.strip_prefix("~/") {
        return format!("{home}/{rest}");
    }
    token
}

pub fn run_line(
    line: &str,
    completions: &mut HashMap<String, PathBuf>,
    jobs: &mut Vec<Job>,
) -> io::Result<ControlFlow<()>> {
    let Some(parsed) = ParsedLine::parse(line) else {
        return Ok(ControlFlow::Continue(()));
    };
    // Pre-open/create the file to match bash behavior. May need to refactor later.
    if let Some(redirect) = &parsed.stdout {
        open_for(redirect)?;
    }
    if let Some(redirect) = &parsed.stderr {
        open_for(redirect)?;
    }

    match parsed.command {
        Command::Exit => return Ok(ControlFlow::Break(())),
        Command::Pwd => {
            let s = format!("{}", env::current_dir()?.display());
            write_to(&s, parsed.stdout.as_ref(), Fd::Stdout)?
        }
        Command::Echo { output } => write_to(&output, parsed.stdout.as_ref(), Fd::Stdout)?,
        Command::Type { target } => {
            if is_builtin(&target) {
                let s = format!("{target} is a shell builtin");
                write_to(&s, parsed.stdout.as_ref(), Fd::Stdout)?;
            } else if let Some(p) = locate_executable(&target) {
                let s = format!("{} is {}", target, p.display());
                write_to(&s, parsed.stdout.as_ref(), Fd::Stdout)?;
            } else {
                let s = format!("{target}: not found");
                write_to(&s, parsed.stderr.as_ref(), Fd::Stderr)?;
            }
        }
        Command::Cd { path } => {
            let target = expand_tilde(path);
            if std::env::set_current_dir(&target).is_err() {
                let s = format!("cd: {target}: No such file or directory");
                write_to(&s, parsed.stderr.as_ref(), Fd::Stderr)?
            }
        }
        Command::Complete(CompleteOp::Register { cmd, path }) => {
            completions.insert(cmd, path);
        }
        Command::Complete(CompleteOp::Unregister { cmd }) => {
            completions.remove(&cmd);
        }
        Command::Complete(CompleteOp::Print { cmd }) => {
            if let Some(c) = completions.get(&cmd) {
                println!("complete -C '{}' {cmd}", c.to_string_lossy())
            } else {
                eprintln!("complete: {cmd}: no completion specification")
            }
        }
        Command::Jobs => {
            for j in jobs {
                println!("[{}]+  {:<24}{}", j.id, "Running", j.command);
            }
        }
        Command::External { name, args } => match locate_executable(&name) {
            Some(path) => {
                let mut cmd = std::process::Command::new(path);
                cmd.arg0(name).args(args);
                if let Some(redirect) = &parsed.stdout {
                    cmd.stdout(open_for(redirect)?);
                }
                if let Some(redirect) = &parsed.stderr {
                    cmd.stderr(open_for(redirect)?);
                }

                if parsed.background {
                    let child = cmd.spawn()?;
                    let id = jobs.len() + 1;
                    println!("[{}] {}", id, child.id());
                    jobs.push(Job {
                        id,
                        command: line.trim().to_string(),
                        child,
                    });
                } else {
                    cmd.status()?;
                }
            }
            None => eprintln!("{}: command not found", name),
        },
    }
    Ok(ControlFlow::Continue(()))
}
