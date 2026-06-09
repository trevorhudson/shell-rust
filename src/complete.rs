use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rustyline::Context;
use rustyline::completion::{Completer, FilenameCompleter, Pair, extract_word};
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

use crate::BUILTINS;

#[derive(Helper, Hinter, Highlighter, Validator)]
pub struct ShellHelper {
    executables: Vec<String>,
    completions: HashMap<String, PathBuf>,
    files: FilenameCompleter,
}

impl ShellHelper {
    pub fn new(executables: Vec<String>) -> Self {
        ShellHelper {
            executables,
            completions: HashMap::new(),
            files: FilenameCompleter::new(),
        }
    }

    pub fn completions_mut(&mut self) -> &mut HashMap<String, PathBuf> {
        &mut self.completions
    }

    fn complete_command(&self, prefix: &str) -> Vec<Pair> {
        let mut names: Vec<String> = BUILTINS
            .iter()
            .map(|s| s.to_string())
            .chain(self.executables.iter().cloned())
            .filter(|b| b.starts_with(prefix))
            .collect();
        names.sort();
        names.dedup();
        names
            .into_iter()
            .map(|b| Pair {
                display: b.clone(),
                replacement: b,
            })
            .collect()
    }
}

fn run_completer(script: &Path, args: Vec<&str>) -> Vec<Pair> {
    let Ok(output) = std::process::Command::new(script).args(args).output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| Pair {
            display: l.to_string(),
            replacement: l.to_string(),
        })
        .collect()
}

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let (word_start, word) = extract_word(line, pos, Some('\\'), char::is_whitespace);

        let command = line.split_whitespace().next();

        let (start, mut candidates) = if word_start != 0
            && let Some(script) = command.and_then(|c| self.completions.get(c))
        {
            (
                word_start,
                run_completer(script, vec!["git", "ad", "remote"]),
            )
        } else if word_start == 0 {
            (0, self.complete_command(word))
        } else {
            self.files.complete(line, pos, ctx)?
        };

        // FilenameCompleter puts the trailing `/` only on `replacement`, not `display`.
        // Mirror it onto `display` so directories stand out in the listing.
        for c in candidates.iter_mut() {
            if c.replacement.ends_with('/') && !c.display.ends_with('/') {
                c.display.push('/');
            }
        }

        // Bash convention: unique match gets a trailing space, except directories
        // (which FilenameCompleter already terminates with `/`).
        if let [only] = candidates.as_mut_slice()
            && !only.replacement.ends_with('/')
        {
            only.replacement.push(' ');
        }

        Ok((start, candidates))
    }
}
