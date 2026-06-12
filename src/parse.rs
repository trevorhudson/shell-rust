use std::path::PathBuf;

// ----------------------------------------------- lexer -----------------------------------------------

enum QuoteState {
    Double,
    Outside,
    Single,
}

enum Token {
    Background,
    Word(String),
    Redirect { fd: Fd, mode: Mode },
}

fn classify(text: String, quoted: bool) -> Token {
    if !quoted {
        match text.as_str() {
            ">" | "1>" => {
                return Token::Redirect {
                    fd: Fd::Stdout,
                    mode: Mode::Truncate,
                };
            }
            ">>" | "1>>" => {
                return Token::Redirect {
                    fd: Fd::Stdout,
                    mode: Mode::Append,
                };
            }
            "2>" => {
                return Token::Redirect {
                    fd: Fd::Stderr,
                    mode: Mode::Truncate,
                };
            }
            "2>>" => {
                return Token::Redirect {
                    fd: Fd::Stderr,
                    mode: Mode::Append,
                };
            }
            "&" => return Token::Background,
            _ => {}
        }
    }
    Token::Word(text)
}

/// Parse user input into tokens. Respects common bash quotation and character escape rules
fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quoted = false;
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
            (QuoteState::Outside, '\\') => {
                escaped = true;
                quoted = true
            }
            // Enter a single quote
            (QuoteState::Outside, '\'') => {
                state = QuoteState::Single;
                in_token = true;
                quoted = true
            }
            // Enter a double quote
            (QuoteState::Outside, '"') => {
                state = QuoteState::Double;
                in_token = true;
                quoted = true
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
                    tokens.push(classify(
                        std::mem::take(&mut current),
                        std::mem::take(&mut quoted),
                    ));
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
        tokens.push(classify(
            std::mem::take(&mut current),
            std::mem::take(&mut quoted),
        ));
    }
    tokens
}

// ----------------------------------------------- parser ----------------------------------------------

#[derive(Copy, Clone)]
pub enum Fd {
    Stderr,
    Stdout,
}

#[derive(Copy, Clone)]
pub enum Mode {
    Append,
    Truncate,
}

pub struct Redirect {
    pub mode: Mode,
    pub path: PathBuf,
}

pub enum CompleteOp {
    Register { cmd: String, path: PathBuf }, // -C
    Print { cmd: String },                   // -p
    Unregister { cmd: String },
}

pub enum Command {
    Cd { path: String },
    Echo { output: String },
    Exit,
    External { args: Vec<String>, name: String },
    Pwd,
    Type { target: String },
    Jobs,
    Complete(CompleteOp),
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
            "complete" => match parts.next()?.as_str() {
                "-C" => {
                    let path = PathBuf::from(parts.next()?);
                    let cmd = parts.next()?;
                    Command::Complete(CompleteOp::Register { cmd, path })
                }
                "-r" => Command::Complete(CompleteOp::Unregister { cmd: parts.next()? }),
                "-p" => Command::Complete(CompleteOp::Print { cmd: parts.next()? }),
                _ => return None,
            },
            "jobs" => Command::Jobs,
            _ => Command::External {
                name: cmd,
                args: parts.collect(),
            },
        })
    }
}

pub struct ParsedLine {
    pub command: Command,
    pub background: bool,
    pub stderr: Option<Redirect>,
    pub stdout: Option<Redirect>,
}

impl ParsedLine {
    pub fn parse(input: &str) -> Option<Self> {
        let mut stdout: Option<Redirect> = None;
        let mut stderr: Option<Redirect> = None;
        let mut background: bool = false;
        let mut words: Vec<String> = Vec::new();

        let mut tokens = tokenize(input.trim()).into_iter();
        while let Some(token) = tokens.next() {
            match token {
                Token::Background => background = true,
                Token::Word(w) => words.push(w),
                Token::Redirect { fd, mode } => {
                    // target is the next token, and it must be a word
                    let Token::Word(path) = tokens.next()? else {
                        return None; // dangling redirect -> syntax error
                    };
                    let redirect = Some(Redirect {
                        path: path.into(),
                        mode,
                    });
                    match fd {
                        Fd::Stdout => stdout = redirect,
                        Fd::Stderr => stderr = redirect,
                    }
                }
            }
        }

        let command = Command::from_tokens(words)?;
        Some(ParsedLine {
            background,
            command,
            stderr,
            stdout,
        })
    }
}
