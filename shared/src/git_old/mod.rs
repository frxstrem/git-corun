use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[macro_use]
pub mod run;

pub fn get_git_dir() -> io::Result<PathBuf> {
    let git_dir = gitc!("rev-parse", "--git-dir")?;
    Ok(PathBuf::from(git_dir))
}

pub fn get_commit_hash(
    git_dir: impl AsRef<Path>,
    commit_ref: impl AsRef<str>,
) -> io::Result<String> {
    let git_dir = git_dir.as_ref();
    let commit_ref = commit_ref.as_ref();

    let git_hash = gitc!("--git-dir", git_dir, "rev-parse", commit_ref)?;
    Ok(git_hash)
}

pub fn get_commit_hashes(
    git_dir: impl AsRef<Path>,
    ref_or_range: impl AsRef<str>,
) -> io::Result<Vec<String>> {
    let git_dir = git_dir.as_ref();
    let ref_or_range = ref_or_range.as_ref();

    if !ref_or_range.contains("..") {
        let git_hash = get_commit_hash(git_dir, ref_or_range)?;
        return Ok(vec![git_hash]);
    }

    let git_hashes = gitc!("--git-dir", git_dir, "rev-list", "--reverse", ref_or_range)?;
    Ok(git_hashes
        .split('\n')
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

pub fn clone_local(src_dir: impl AsRef<Path>, dst_dir: impl AsRef<Path>) -> io::Result<()> {
    let src_dir = src_dir.as_ref();
    let dst_dir = dst_dir.as_ref();

    gitc!("clone", "--local", "--recurse-submodules", src_dir, dst_dir)?;
    Ok(())
}

pub fn clean_work_dir(work_dir: impl AsRef<Path>) -> io::Result<()> {
    let work_dir = work_dir.as_ref();

    gitc!("-C", work_dir, "clean", "-fxd")?;
    Ok(())
}

pub fn checkout_detached(work_dir: impl AsRef<Path>, commit: impl AsRef<str>) -> io::Result<()> {
    let work_dir = work_dir.as_ref();
    let commit = commit.as_ref();

    gitc!("-C", work_dir, "checkout", "--force", "--detach", commit)?;
    Ok(())
}

pub fn apply_stash(work_dir: impl AsRef<Path>, commit: impl AsRef<str>) -> io::Result<()> {
    let work_dir = work_dir.as_ref();
    let commit = commit.as_ref();

    gitc!("-C", work_dir, "stash", "apply", "--index", commit)?;
    Ok(())
}

pub fn show_commit(
    git_dir: impl AsRef<Path>,
    commit: impl AsRef<str>,
    format: impl AsRef<str>,
) -> io::Result<()> {
    let git_dir = git_dir.as_ref();
    let commit = commit.as_ref();
    let format = format.as_ref();

    let pretty_format = format!("--pretty=format:{}", format);

    let output = Command::new("git")
        .args(gitc_args!(
            "--git-dir",
            git_dir,
            "show",
            "--quiet",
            "--no-patch",
            pretty_format,
            "--date=format:%e %b %Y %H:%M",
            commit
        ))
        .stdout(Stdio::inherit())
        .output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let error = error.trim_end_matches('\n');
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("git error: {}", error),
        ));
    }

    Ok(())
}
