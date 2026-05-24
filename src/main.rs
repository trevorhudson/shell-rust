use std::{
    env, fs, io::{self, Write}, os::unix::{fs::PermissionsExt, process::CommandExt}, path::{self, PathBuf}
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

struct ParsedLine {
    command: Command,
    redirect: Option<PathBuf>,
}

impl ParsedLine {
    fn parse(input: &str) -> Option<Self> {
        let mut tokens = tokenize(input.trim());
        let mut redirect: Option<PathBuf> = None;
        let position = tokens.iter().position(|t| t == ">" || t == "1>");

        if let Some(i) = position {
            redirect = Some(PathBuf::from(tokens.remove(i + 1)));
            tokens.remove(i); // remove redirect symbol
        }

        let command = Command::from_tokens(tokens)?;
        Some(ParsedLine { command, redirect })
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


        match parsed.command {
            Command::Exit => break,
            Command::Pwd => {
                let output = format!("{}", env::current_dir()?.display());
                match &parsed.redirect {
                    Some(path) => fs::write(path, format!("{output}\n"))?,
                    None => println!("{output}"),
                }
            }
            Command::Echo { output } => {
                match &parsed.redirect {
                    Some(path) => fs::write(path, format!("{output}\n"))?,
                    None => println!("{output}"),
                }            }
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
