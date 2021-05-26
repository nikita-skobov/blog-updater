use super::CommandOutput;
use super::new_err;
use std::io::prelude::*;
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use io::BufReader;

pub fn get_git_command<T>(
    cmd: &[&str],
    filter: impl Fn(&CommandOutput) -> Result<T, String>,
) -> io::Result<T> {
    let cmd_out = exechelper::execute(cmd)?;
    let filtered = filter(&cmd_out)
        .map_err(|s| io::Error::new(io::ErrorKind::Other, s))?;
    Ok(filtered)
}

pub fn make_git_branch(branch_name: &str, from_ref: &str) -> io::Result<()> {
    let exec_args = [
        "git", "branch", branch_name, from_ref,
    ];
    get_git_command(&exec_args, |cmdout| {
        if cmdout.status != 0 {
            let err_msg = format!("Failed to make branch {}", branch_name);
            Err(err_msg)
        } else {
            Ok(())
        }
    })
}

pub fn get_all_git_branches() -> io::Result<Vec<String>> {
    let exec_args = [
        "git", "for-each-ref", "--format=%(refname:short)", "refs/heads/"
    ];
    get_git_command(&exec_args, |cmdout| {
        let branches = cmdout.stdout.trim_end().split('\n')
            .map(|n| n.to_string()).collect::<Vec<String>>();
        if branches.is_empty() {
            return Err("Failed to find any git branches. Are you sure you're in a git repository?".into())
        }
        Ok(branches)
    })
}

pub fn get_first_commit_of_branch(branch_name: &str) -> io::Result<String> {
    let exec_args = [
        "git", "rev-list", "--max-parents=0", branch_name
    ];
    get_git_command(&exec_args, |cmdout| {
        if cmdout.status != 0 {
            let err_msg = format!("Failed to get first commit of branch {}", branch_name);
            Err(err_msg)
        } else {
            let commit_id = cmdout.stdout.trim_end();
            Ok(commit_id.to_owned())
        }
    })
}
// TODO: I thought this was a good command at first
// but then decided not to use it because it would require too much parsing
// going to leave it as a comment here in case I want to use it again sometime:
// git log A..B --date=unix --name-only --pretty=format:%h%n%cd
// the above log format will be:
//      [hash]
//      [timestamp]
//      [files...]
//      <newline>

pub fn get_all_files_changed_since_last_blog_update(
    blog_branch_name: &str, main_ref_branch_name: &str,
) -> io::Result<Vec<String>> {
    let exec_args = [
        "git", "diff", main_ref_branch_name, blog_branch_name, "--name-only",
    ];
    let list = get_git_command(&exec_args, |cmdout| {
        if cmdout.status != 0 {
            let err_msg = format!("Failed to get files changed for revision between {} and {}", main_ref_branch_name, blog_branch_name);
            Err(err_msg)
        } else {
            let list = cmdout.stdout.trim_end().split('\n')
                .map(|n| n.to_string()).collect::<Vec<String>>();
            Ok(list)
        }
    })?;
    Ok(list)
}

pub fn get_all_timestamps_of_file_commits(
    blog_file_path: &str, main_ref_branch_name: &str,
) -> io::Result<Vec<String>> {
    let exec_args = [
        "git", "log", main_ref_branch_name, "--date=unix", "--pretty=format:%cd,%an", "--", blog_file_path,
    ];
    let list = get_git_command(&exec_args, |cmdout| {
        if cmdout.status != 0 {
            let err_msg = format!("Failed to get timestamps of changes to {}", blog_file_path);
            Err(err_msg)
        } else {
            let list = cmdout.stdout.trim_end().split('\n')
                .map(|n| n.to_string()).collect::<Vec<String>>();
            Ok(list)
        }
    })?;
    Ok(list)
}

pub fn get_git_toplevel_absolute_path() -> io::Result<PathBuf> {
    let exec_args = [
        "git", "rev-parse", "--show-toplevel"
    ];
    get_git_command(&exec_args, |cmdout| {
        if cmdout.status != 0 {
            Err("Failed to find git repo root".into())
        } else {
            Ok(PathBuf::from(cmdout.stdout.trim_end().clone()))
        }
    })
}

pub fn get_blog_file_from_branch(blog_file_path: &str, branch_name: &str) -> io::Result<String> {
    let refpath = format!("{}:{}", branch_name, blog_file_path);
    let exec_args = [
        "git", "show", &refpath
    ];
    get_git_command(&exec_args, |cmdout| {
        if cmdout.status != 0 {
            Err(format!("Failed to get blog file {}", refpath))
        } else {
            Ok(cmdout.stdout.clone())
        }
    })
}

pub fn can_blog_branch_be_fast_forwarded(blog_branch_name: &str, main_ref_branch_name: &str) -> io::Result<bool> {
    let exec_args = [
        "git", "merge-base", "--is-ancestor", blog_branch_name, main_ref_branch_name
    ];
    get_git_command(&exec_args, |cmdout| Ok(cmdout.status == 0))
}

pub fn delete_branch(branch_name: &str) -> io::Result<bool> {
    let exec_args = [
        "git", "branch", "-D", branch_name
    ];
    get_git_command(&exec_args, |cmdout| Ok(cmdout.status == 0))
}

pub fn find_all_blog_files_from_git_tracked_files(
    blog_name: &str, main_ref_branch_name: &str,
) -> io::Result<Vec<String>> {
    // for this one we will use a manual spawn and then loop through the output
    // to avoid allocating a massive string, because some repositories can have
    // tons of files.
    let mut out_vec = vec![];
    let mut cmd = Command::new("git");
    cmd.arg("ls-tree").arg("-r").arg(main_ref_branch_name).arg("--name-only");
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    let mut child = cmd.spawn()?;
    let stdout = child.stdout.as_mut()
        .map_or(Err(new_err("Failed to get git process standard output")), |s| Ok(s))?;
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let line = line?;
        if line.ends_with(blog_name) {
            out_vec.push(line);
        }
    }

    Ok(out_vec)
}
