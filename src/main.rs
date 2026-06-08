use std::collections::HashMap;
use std::ops::ControlFlow;

use rustyline::error::ReadlineError;
use rustyline::{CompletionType, Config, Editor};

mod complete;
mod exec;
mod parse;

use complete::ShellHelper;

const BUILTINS: &[&str] = &["echo", "exit", "type", "pwd", "cd", "complete"];

fn main() -> anyhow::Result<()> {
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();
    let mut editor = Editor::<ShellHelper, _>::with_config(config)?;

    editor.set_helper(Some(ShellHelper::new(exec::collect_executables())));

    let mut completions: HashMap<String, String> = HashMap::new();

    loop {
        let line = match editor.readline("$ ") {
            Ok(line) => line,
            Err(ReadlineError::Eof) => break,
            Err(ReadlineError::Interrupted) => continue,
            Err(e) => return Err(e.into()),
        };
        match exec::run_line(&line, &mut completions) {
            Ok(ControlFlow::Break(())) => break,
            Ok(ControlFlow::Continue(())) => {}
            Err(e) => eprintln!("shell: {e}"),
        }
    }
    Ok(())
}
