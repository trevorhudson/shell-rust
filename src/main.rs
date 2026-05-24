use std::{
    env,
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
    // Redirect { output: String, path: String}
}

impl Command {
    fn parse(input: &str) -> Option<Self> {
        let mut parts = tokenize(input.trim()).into_iter();
        let cmd = parts.next()?;

        println!("{:?}", parts);

        // process each of them to return all args instead of just the next arg they are expecting?
        // then
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
    redirect: Option<PathBuf>,
}

impl ParsedLine {
    fn parse(input: &str) -> Option<Self> {
        let command = Command::parse(input)?;
        let redirect = None;
        Some(ParsedLine { command, redirect })
    }
}

fn main() -> anyhow::Result<()> {
    loop {
        print!("$ ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let Some(parsed) = ParsedLine::parse(&input) else { continue };

        // Each of these need to be refactored to return an actual result.
        // That result then needs to be parsed
        // PROCESS the command. STORE it. CHECK if there's a pipe, and if so, try to pipe it.
        match parsed.command {
            Command::Exit => break,
            Command::Pwd => {
                println!("{}", env::current_dir()?.display());
            }
            Command::Echo { output } => {
                println!("{}", output)
            }
            Command::Type { target } => {
                if is_builtin(&target) {
                    println!("{} is a shell builtin", target)
                } else if let Some(path) = locate_executable(&target) {
                    println!("{} is {}", target, path.display());
                } else {
                    eprintln!("{}: not found", target);
                }
            }
            Command::Cd { path } => {
                let path = path.replace("~", &std::env::var("HOME").unwrap_or_default());
                if std::env::set_current_dir(&path).is_err() {
                    eprintln!("cd: {path}: No such file or directory");
                }
            }
            Command::External { name, args } => match locate_executable(&name) {
                Some(path) => {
                    std::process::Command::new(path)
                        .arg0(name)
                        .args(args)
                        .status()?;
                }
                None => eprintln!("{}: command not found", name),
            },
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
