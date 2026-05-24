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
}

impl Command {
    fn parse(input: &str) -> Option<Self> {
        let mut parts = tokenize(input.trim()).into_iter();
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

fn main() -> anyhow::Result<()> {
    loop {
        print!("$ ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let Some(command) = Command::parse(&input) else {
            continue;
        };

        match command {
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

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut in_quote = false;

    for c in input.chars() {
        match (in_quote, c) {
            (false, '\'') => {
                in_quote = true;
                in_token = true
            }
            (true, '\'') => in_quote = false,
            (false, c) if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut cur));
                    in_token = false
                }
            }
            (_, c) => {
                cur.push(c);
                in_token = true;
            }
        }
    }
    if in_token {
        tokens.push(cur)
    }
    tokens
}


////// ------------------------------------------------ TESTS

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            tokenize("echo 'hello    world'"),
            vec!["echo".to_string(), "hello    world".to_string()]
        );
    }
    #[test]
    fn preserves_spaces() {
        assert_eq!(
            tokenize("'hello    world'"),
            vec!["hello    world".to_string()]
        );
    }
    #[test]
    fn collapses_spaces() {
        assert_eq!(
            tokenize("hello    world"),
            vec!["hello".to_string(), "world".to_string()]
        );
    }
    #[test]
    fn contatenates_spaces() {
        assert_eq!(tokenize("'hello''world'"), vec!["helloworld".to_string()]);
    }
    #[test]
    fn ignores_empty() {
        assert_eq!(tokenize("hello''world"), vec!["helloworld".to_string()]);
    }
}
