mod git;

use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command, ExitStatus, Stdio};

use chrono::{prelude::*, Duration, Local};
use structopt::{clap, StructOpt};

#[derive(Clone, Debug, StructOpt)]
struct Options {
    #[structopt(
        help = "Directory to check out and run code in",
        short = "d",
        long = "dir"
    )]
    dir: Option<PathBuf>,

    #[structopt(
        help = "Apply latest stash before running",
        short = "s",
        long = "stash"
    )]
    apply_stash: bool,

    #[structopt(skip)]
    apply_index: bool,

    #[structopt(help = "Run as shell command", short = "c")]
    shell_command: bool,

    #[structopt(help = "Show output from commands", short = "v", long = "verbose")]
    verbose: bool,

    #[structopt(help = "List of commits to run on", default_value = "HEAD")]
    commits: Vec<String>,

    #[structopt(help = "Command to execute", required = true, last = true)]
    command: Vec<String>,
}

impl Options {
    fn from_args_safe() -> clap::Result<Self> {
        Ok(Self::from_clap(&Self::clap().get_matches_safe()?))
    }
}

fn main() {
    let opts = Options::from_args_safe().unwrap_or_else(|err| {
        if err.use_stderr() {
            eprintln!("{}", err.message);
            process::exit(128);
        }
        let out = io::stdout();
        writeln!(&mut out.lock(), "{}", err.message).expect("Error writing error to stdout");
        process::exit(128);
    });

    match app(opts) {
        Err(err) => {
            eprintln!("Error: {}", err);
            process::exit(128);
        }
        Ok(exit_code) => process::exit(exit_code),
    }
}

fn app(opts: Options) -> Result<i32, Box<dyn Error>> {
    // get git directory
    let git_dir = git::get_git_dir()?;

    // get latest stash commit
    let stash_commit = if opts.apply_stash {
        Some(git::get_commit_hash(&git_dir, "refs/stash")?)
    } else if opts.apply_index {
        unimplemented!()
    } else {
        None
    };

    // expand list of commits
    let commits = opts
        .commits
        .iter()
        .map(|commit| git::get_commit_hashes(&git_dir, commit))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    // create temporary directory (and possibly clean up old ones)
    let tmpdir = create_directory(&opts)?;
    eprintln!("Running in directory: {}", tmpdir.to_string_lossy());

    // git clone into temporary directory
    git::clone_local(&git_dir, &tmpdir)?;

    let mut last_exit_code = 0;
    for commit in commits {
        last_exit_code = run_app_for(
            &opts,
            &git_dir,
            tmpdir.as_ref(),
            &commit,
            stash_commit.as_ref().map(String::as_str),
        )?;
    }

    Ok(last_exit_code)
}

fn run_app_for(
    opts: &Options,
    git_dir: &Path,
    work_tree: &Path,
    commit: &str,
    stash_commit: Option<&str>,
) -> Result<i32, Box<dyn Error>> {
    // get commit hash
    let commit = git::get_commit_hash(&git_dir, commit)?;

    // check out directory
    git::checkout_detached(&work_tree, &commit)?;

    // clean directory
    git::clean_work_dir(&work_tree)?;

    if let Some(stash_commit) = stash_commit {
        // apply stash
        git::apply_stash(&work_tree, stash_commit)?;
    }

    // print commit
    print_commit(&git_dir, &commit, Status::Pending)?;

    // run command in repo
    let exit_status = run_in(&opts, opts.command.iter().map(String::as_str), &work_tree)?;
    let status = exit_status.into();

    // print status
    if !opts.verbose {
        let stdout = io::stdout();
        let mut stdout = stdout.lock();
        write!(stdout, "\x1b[1F\x1b[K")?;
        stdout.flush()?;
    }
    print_commit(&git_dir, &commit, status)?;

    Ok(exit_status.code().unwrap_or(255))
}

fn print_commit(
    git_dir: impl AsRef<Path>,
    commit: impl AsRef<str>,
    status: Status,
) -> io::Result<()> {
    let base_format = "%C(yellow)%h %C(bold)%G? %Creset%C(cyan)[%Cgreen%ad%C(cyan) by %Cred%an%C(cyan)]%Creset %s";
    let format = format!("{} {}", status.get_format(), base_format);

    git::show_commit(git_dir, commit, &format)
}

fn run_in<'a, I>(opts: &Options, command: I, dir: impl AsRef<Path>) -> io::Result<ExitStatus>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut command = command.into_iter();
    let dir = dir.as_ref();

    let cmd_first = command.next().unwrap();
    let cmd_rest = command.collect::<Vec<_>>();

    let (exec_name, cmd_args) = if opts.shell_command {
        let exec_name = "/bin/bash";
        let mut args = vec!["-c", cmd_first, "--"];
        args.extend(cmd_rest);
        (exec_name, args)
    } else {
        (cmd_first, cmd_rest)
    };

    if opts.verbose {
        Command::new(&exec_name)
            .args(&cmd_args)
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
    } else {
        Command::new(&exec_name)
            .args(&cmd_args)
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    }
}

fn default_base_dir() -> PathBuf {
    dirs::home_dir().expect("no home dir").join(".git-corun")
}

fn create_directory(opts: &Options) -> io::Result<PathBuf> {
    const DATE_FORMAT_STR: &str = "%Y%m%d-%H%M%S-%f";

    let path = match &opts.dir {
        Some(dir) => dir.clone(),
        None => {
            let base_dir = default_base_dir();
            // clean up existing build directories
            if base_dir.exists() {
                let now = Local::now();

                fs::read_dir(&base_dir)?
                    .flat_map(Result::ok)
                    .map(|entry| entry.path())
                    .flat_map(|path| {
                        let name = path.file_name()?.to_string_lossy().to_string();
                        let date = Local.datetime_from_str(&name, DATE_FORMAT_STR).ok()?;
                        Some((path, date))
                    })
                    .filter(|(_, date)| now.signed_duration_since(*date) > Duration::weeks(7))
                    .for_each(|(path, _)| {
                        eprintln!("Removing old directory: {:?}", path);
                        if let Err(err) = fs::remove_dir_all(path) {
                            eprintln!("  Failed to remove directory: {}", err);
                        }
                    });
            }

            let name = Local::now().format(DATE_FORMAT_STR).to_string();
            base_dir.join(name)
        }
    };

    // create new directory
    fs::create_dir_all(&path)?;
    Ok(path)
}

#[derive(Copy, Clone, Debug)]
enum Status {
    /// Process is still running.
    Pending,
    /// Process exited with exit code 0.
    Success(i32),
    /// Process exited with exit code 1-124, 126 or 127.
    Failure(i32),
    /// Process exited with exit code 125.
    Inconclusive(i32),
    /// Process exited with any other exit code.
    Abort(Option<i32>),
}

impl Status {
    fn get_format(self) -> impl Display {
        let prefix = match self {
            Status::Pending => "%C(bold)%C(yellow)●",
            Status::Success(_) => "%C(bold)%C(green)✔",
            Status::Failure(_) => "%C(bold)%C(red)✘",
            Status::Inconclusive(_) => "%C(bold)%C(blue)?",
            Status::Abort(_) => "%C(bold)%C(red)!",
        };

        if let Some(code) = self.code() {
            format!("{}{:>3}%Creset", prefix, (code & 0xff) as u8)
        } else {
            format!("{}   %Creset", prefix)
        }
    }

    fn code(&self) -> Option<i32> {
        match *self {
            Status::Pending => None,
            Status::Success(code) => Some(code),
            Status::Failure(code) => Some(code),
            Status::Inconclusive(code) => Some(code),
            Status::Abort(code) => code,
        }
    }
}

impl From<ExitStatus> for Status {
    fn from(exit_status: ExitStatus) -> Status {
        match exit_status.code() {
            Some(code @ 0) => Status::Success(code),
            Some(code @ 1..=124) | Some(code @ 126) | Some(code @ 127) => Status::Failure(code),
            Some(code @ 125) => Status::Inconclusive(code),
            code => Status::Abort(code),
        }
    }
}
