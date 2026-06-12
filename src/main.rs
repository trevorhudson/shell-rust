use std::ops::ControlFlow;

use rustyline::error::ReadlineError;
use rustyline::{CompletionType, Config, Editor};

mod complete;
mod exec;
mod parse;

use complete::ShellHelper;

const BUILTINS: &[&str] = &["echo", "exit", "type", "pwd", "cd", "complete", "jobs"];

fn main() -> anyhow::Result<()> {
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();
    let mut editor = Editor::<ShellHelper, _>::with_config(config)?;
    let mut jobs: Vec<exec::Job> = Vec::new();

    editor.set_helper(Some(ShellHelper::new(exec::collect_executables())));

    loop {
        // Reap finished background jobs before drawing the prompt, so their
        // `Done` lines land between the last command's output and the prompt.
        exec::report_and_reap(&mut jobs, false);

        let line = match editor.readline("$ ") {
            Ok(line) => line,
            Err(ReadlineError::Eof) => break,
            Err(ReadlineError::Interrupted) => continue,
            Err(e) => return Err(e.into()),
        };

        let action = match editor.helper_mut() {
            Some(helper) => exec::run_line(&line, helper.completions_mut(), &mut jobs),
            None => Ok(ControlFlow::Continue(())),
        };

        match action {
            Ok(ControlFlow::Break(())) => break,
            Ok(ControlFlow::Continue(())) => {}
            Err(e) => eprintln!("shell: {e}"),
        }
    }
    Ok(())
}
