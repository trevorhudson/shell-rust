use std::{
    env, fs,
    io::{self, Write},
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::PathBuf,
};

enum Command {
    Exit,
    Pwd,
    Echo { output: String },
    Type { target: String },
    Cd { path: String },
    External { name: String, args: Vec<String> },
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

struct Redirect {
    path: PathBuf,
    mode: Mode,
}
#[derive(Copy, Clone)]
enum Fd {
    Stdout,
    Stderr,
}
#[derive(Copy, Clone)]
enum Mode {
    Truncate,
    Append,
}

struct ParsedLine {
    command: Command,
    stdout: Option<Redirect>,
    stderr: Option<Redirect>,
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
            stdout,
            stderr,
        })
    }
}

fn main() -> anyhow::Result<()> {
    loop {
        print!("$ ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let Some(parsed) = ParsedLine::parse(&input) else {
            continue;
        };

        // Pre-open/create the file to match bash behavior. May need to refactor later.
        if let Some(redirect) = &parsed.stdout {
            fs::File::create(&redirect.path)?;
        }
        if let Some(redirect) = &parsed.stderr {
            fs::File::create(&redirect.path)?;
        }

        match parsed.command {
            Command::Exit => break,
            Command::Pwd => {
                let output = format!("{}", env::current_dir()?.display());
                match &parsed.stdout {
                    Some(redirect) => fs::write(&redirect.path, format!("{output}\n"))?,
                    None => println!("{output}"),
                }
            }
            Command::Echo { output } => match &parsed.stdout {
                Some(redirect) => fs::write(&redirect.path, format!("{output}\n"))?,
                None => println!("{output}"),
            },
            Command::Type { target } => {
                let output = if is_builtin(&target) {
                    format!("{target} is a shell builtin")
                } else if let Some(path) = locate_executable(&target) {
                    format!("{} is {}", target, path.display())
                } else {
                    let output = format!("{target}: not found");
                    match &parsed.stderr {
                        Some(redirect) => fs::write(&redirect.path, format!("{output}\n"))?,
                        None => eprintln!("{output}"),
                    }
                    continue;
                };
                match &parsed.stdout {
                    Some(redirect) => fs::write(&redirect.path, format!("{output}\n"))?,
                    None => println!("{output}"),
                }
            }
            Command::Cd { path } => {
                let path = path.replace("~", &std::env::var("HOME").unwrap_or_default());
                if std::env::set_current_dir(&path).is_err() {
                    let output = format!("cd: {path}: No such file or directory");
                    match &parsed.stderr {
                        Some(redirect) => fs::write(&redirect.path, format!("{output}\n"))?,
                        None => eprintln!("{output}"),
                    }
                }
            }
            Command::External { name, args } => match locate_executable(&name) {
                Some(path) => {
                    let mut cmd = std::process::Command::new(path);
                    cmd.arg0(name).args(args);
                    if let Some(redirect) = &parsed.stdout {
                        cmd.stdout(std::fs::File::create(&redirect.path)?);
                    }
                    if let Some(redirect) = &parsed.stderr {
                        cmd.stderr(std::fs::File::create(&redirect.path)?);
                    }
                    cmd.status()?;
                }
                None => eprintln!("{}: command not found", name),
            },
        }
    }
    Ok(())
}

fn write_to(content: &str, redirect: Option<&Redirect>, default: Fd) -> io::Result<()> {
    match redirect {
        Some(r) => {
            let mut file = match r.mode {
                Mode::Truncate => fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&r.path)?,
                Mode::Append => fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&r.path)?,
            };
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

enum QuoteState {
    Outside,
    Single,
    Double,
}

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
