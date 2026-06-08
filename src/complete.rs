use rustyline::Context;
use rustyline::completion::{Completer, FilenameCompleter, Pair, extract_word};
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

use crate::BUILTINS;

#[derive(Helper, Hinter, Highlighter, Validator)]
pub struct ShellHelper {
    executables: Vec<String>,
    files: FilenameCompleter,
}

impl ShellHelper {
    pub fn new(executables: Vec<String>) -> Self {
        ShellHelper {
            executables,
            files: FilenameCompleter::new(),
        }
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

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let (word_start, word) = extract_word(line, pos, Some('\\'), char::is_whitespace);

        let (start, mut candidates) = if word_start == 0 {
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
