## Tab Autocompletion

- implement for `echo` and `exit`
- when user enters `<TAB>` during a partial command:
  - Check if it matches a known command
  - if it's a match, complete it. add a trailing a space so the user can add additional arguments

 - We need to know all available commmands

Implementation Steps:

1. Refactor current implementation to use rustyline
2.
