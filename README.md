# git-corun

`git-corun` is a Git command to **c**heck**o**ut a commit (or list of commits) into a temporary directory and **run** a command on each commit.

It is useful for when you want to run a command on a clean checkout for one or more commits, e.g. for ensuring that tests succeed or finding the commit that introduces some bug.

## Installation

Install from Git:

```
cargo install --git https://github.com/frxstrem/git-corun.git git-corun
```

## Usage

```
git corun [OPTIONS] COMMITS... -- COMMAND ARGS...
```

If no commits are given, then the current `HEAD` is implicitly used.

Valid options are:
* `-c`: Run `COMMAND` as a shell command (`/bin/bash -c "COMMAND"`). `ARGS...` are given as arguments to the shell, e.g. `$1`, `$2` etc.
* `-s`: Apply the latest entry on the stash to each commit before running the command.
* `-v`: Show output from commands, not just final result.