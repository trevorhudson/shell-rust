use std::{
    env, fs,
    io::{self, Write},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::PathBuf,
};

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::{Context, Editor};
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

const BUILTINS: &[&str] = &["echo", "exit", "type", "pwd", "cd"];

enum QuoteState {
    Double,
    Outside,
    Single,
}

#[derive(Copy, Clone)]
enum Fd {
    Stderr,
    Stdout,
}

#[derive(Copy, Clone)]
enum Mode {
    Append,
    Truncate,
}

struct Redirect {
    mode: Mode,
    path: PathBuf,
}

enum Command {
    Cd { path: String },
    Echo { output: String },
    Exit,
    External { args: Vec<String>, name: String },
    Pwd,
    Type { target: String },
}

impl Command {
    fn from_tokens(input: Vec<String>) -> Option<Self> {
        let mut parts = input.into_iter();
        let cmd = parts.next()?;

        Some(match cmd.as_str() {
            "exit" => Command::Exit,
            "pwd" => Command::Pwd,
            "echo" => Command::Echo {
                output: parts.collect::<Vec<_>>().join(" "),
            },
            "cd" => Command::Cd {
                path: parts.next()?,
            },
            "type" => Command::Type {
                target: parts.next()?,
            },
            _ => Command::External {
                name: cmd,
                args: parts.collect(),
            },
        })
    }
}

struct ParsedLine {
    command: Command,
    stderr: Option<Redirect>,
    stdout: Option<Redirect>,
}

impl ParsedLine {
    fn parse(input: &str) -> Option<Self> {
        let mut tokens = tokenize(input.trim());
        let mut stdout: Option<Redirect> = None;
        let mut stderr: Option<Redirect> = None;

        let mut found: Vec<(usize, Fd, Mode, String)> = Vec::new();

        for (i, t) in tokens.iter().enumerate() {
            let Some((fd, mode)) = (match t.as_str() {
                ">" | "1>" => Some((Fd::Stdout, Mode::Truncate)),
                ">>" | "1>>" => Some((Fd::Stdout, Mode::Append)),
                "2>" => Some((Fd::Stderr, Mode::Truncate)),
                "2>>" => Some((Fd::Stderr, Mode::Append)),
                _ => None,
            }) else {
                continue;
            };

            let path = tokens.get(i + 1)?;

            found.push((i, fd, mode, path.clone()))
        }

        for f in found.iter().rev() {
            tokens.remove(f.0 + 1);
            tokens.remove(f.0);
        }

        for (_, fd, mode, path) in found.iter() {
            let redirect = Some(Redirect {
                path: path.into(),
                mode: *mode,
            });
            match fd {
                Fd::Stdout => stdout = redirect,
                Fd::Stderr => stderr = redirect,
            }
        }

        let command = Command::from_tokens(tokens)?;
        Some(ParsedLine {
            command,
            stderr,
            stdout,
        })
    }
}

#[derive(Helper, Hinter, Highlighter, Validator)]
struct ShellHelper {
    executables: Vec<String>,
}

impl Completer for ShellHelper {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let prefix = &line[..pos];

        let candidates: Vec<Pair> = BUILTINS
            .iter()
            .map(|s| s.to_string())
            .chain(self.executables.iter().cloned())
            .filter(|b| b.starts_with(prefix))
            .map(|b| Pair {
                display: b.to_string(),
                replacement: format!("{b} "),
            })
            .collect();
        Ok((0, candidates))
    }
}

// ------------------------------------------ functions ------------------------------------------------

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
fn collect_executables() -> Vec<String> {
    let path = std::env::var("PATH").unwrap_or_default();

    let mut names: Vec<String> = Vec::new();
    for dir in std::env::split_paths(&path) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let exec = path
                .metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false);
            if !exec {
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
        (p.is_file()
            && p.metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false))
        .then_some(p)
    })
}

/// Parse user input into tokens. Respects common bash quotation and character escape rules
fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut escaped = false;

    let mut state = QuoteState::Outside;

    for c in input.chars() {
        match (&state, c) {
            // Push next char if escaped
            (_, c) if escaped => {
                current.push(c);
                escaped = false;
                in_token = true;
            }
            // // Outside + escaped char
            (QuoteState::Outside, '\\') => escaped = true,
            // Enter a single quote
            (QuoteState::Outside, '\'') => {
                state = QuoteState::Single;
                in_token = true
            }
            // Enter a double quote
            (QuoteState::Outside, '"') => {
                state = QuoteState::Double;
                in_token = true
            }
            // // Double + escaped char
            (QuoteState::Double, '\\') => escaped = true,
            // Exit single
            (QuoteState::Single, '\'') => state = QuoteState::Outside,
            // Exit double
            (QuoteState::Double, '"') => state = QuoteState::Outside,
            // Outside + Whitespace, ends token
            (QuoteState::Outside, c) if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false
                }
            } // All other state/char combinations
            (_, c) => {
                current.push(c);
                in_token = true;
            }
        }
    }
    // Push final token
    if in_token {
        tokens.push(current)
    }
    tokens
}

// ---------------------------------------------------------- main ----------------------------------------------------

fn main() -> anyhow::Result<()> {
    let mut editor = Editor::<ShellHelper, _>::new()?;
    editor.set_helper(Some(ShellHelper {
        executables: collect_executables(),
    }));

    // DELETE let _path = std::env::var("PATH").unwrap_or_default();

    loop {
        let line = match editor.readline("$ ") {
            Ok(line) => line,
            Err(ReadlineError::Eof) => break,
            Err(ReadlineError::Interrupted) => continue,
            Err(e) => return Err(e.into()),
        };

        let Some(parsed) = ParsedLine::parse(&line) else {
            continue;
        };

        // Pre-open/create the file to match bash behavior. May need to refactor later.
        if let Some(redirect) = &parsed.stdout {
            open_for(redirect)?;
        }
        if let Some(redirect) = &parsed.stderr {
            open_for(redirect)?;
        }

        match parsed.command {
            Command::Exit => break,
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
                let path = path.replace("~", &std::env::var("HOME").unwrap_or_default());
                if std::env::set_current_dir(&path).is_err() {
                    let s = format!("cd: {path}: No such file or directory");
                    write_to(&s, parsed.stderr.as_ref(), Fd::Stderr)?
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
                    cmd.status()?;
                }
                None => eprintln!("{}: command not found", name),
            },
        }
    }
    Ok(())
}
