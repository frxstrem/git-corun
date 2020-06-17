use std::ffi::OsStr;
use std::io;
use std::process::Command;

macro_rules! __gitc_args {
    ($out:ident;) => {};

    ($out:ident; ..$arg:expr $(,)?) => {
        $out.extend(::std::iter::Iterator::map(
            ::std::iter::IntoIterator::into_iter($arg),
            ::std::convert::AsRef::<::std::ffi::OsStr>::as_ref));
    };

    ($out:ident; $arg:expr $(,)?) => {
        $out.push(::std::convert::AsRef::as_ref(&$arg));
    };

    ($out:ident; ..$arg:expr, $($rest:tt)*) => {
        $out.extend(::std::iter::Iterator::map(
            ::std::iter::IntoIterator::into_iter($arg),
            ::std::convert::AsRef::<::std::ffi::OsStr>::as_ref));
        __gitc_args!($out; $($rest)*);
    };

    ($out:ident; $arg:expr, $($rest:tt)*) => {
        $out.push(::std::convert::AsRef::as_ref(&$arg));
        __gitc_args!($out; $($rest)*);
    };
}

macro_rules! gitc_args {
    ($($tt:tt)*) => {
        &{
            let mut args = Vec::new();
            __gitc_args!(args; $($tt)*);
            args
        } as &[&::std::ffi::OsStr]
    };
}

macro_rules! gitc {
    ($($tt:tt)*) => { $crate::git_old::run::run_gitc(gitc_args!($($tt)*)) };
}

pub fn run_gitc<S>(args: &[S]) -> io::Result<String>
where
    S: AsRef<OsStr>,
{
    let output = Command::new("git").args(args).output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let error = error.trim_end_matches('\n');
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("git error: {}", error),
        ));
    }

    let result = String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_string();
    Ok(result)
}
