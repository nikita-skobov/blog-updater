use gumdrop::Options;
use std::path::PathBuf;
use std::io::prelude::*;
use std::io;
use exechelper::CommandOutput;

use simple_interaction as interact;

#[derive(Debug, Options)]
pub struct Cli {
    #[options(default = "blogs")]
    pub blogs_branch_name: String,
    #[options(default = "BLOG.md")]
    pub blog_file_name: String,
    /// by default we will look for either a 'master' or a 'main' branch. otherwise, if you want to use some other specific branch as the main branch then you can specify this with --main-branch-name <name>.
    pub main_branch_name: Option<String>,

    pub blog_renderer_executable: String,
    pub homepage_renderer_executable: String,
    // TODO:
    // might need to add formatting string for how
    // options need to be passed to blog_renderer_executable
    #[options(default = "tmp_blog")]
    pub rendered_directory: PathBuf,

    /// by default, this program will prompt the user with some questions. disable interactive mode if you want to go with the default choices
    pub no_interactive: bool,
}

pub struct BlogFile {
    pub path_from_root: String,
    /// the first element should be the most recent update,
    /// ie: use this as the blog post's updated time,
    /// and the last element should be the created at time
    pub update_timestamps: Vec<String>,
}

/// steps for updating blogs:
/// 1. find the <blogs_branch> and check every commit
///    that has been made since that involes a file called <blog_file>
/// 2. for every <blog_file> that has been updated since <blogs_branch>
///    current HEAD, send that file to <blog_renderer> and output to
///    <rendered_directory>
/// 3. After iterating over all <blog_file>s, also render the blog homepage
/// 4. And also update the <blogs_branch> to point to current <main> HEAD
/// 5. update RSS by [fetching existing RSS and updating it, re-creating RSS from scratch]
///    and also place that in <rendered_directory>
/// 6. optionally push <rendered_directory> up to wherever its being hosted
/// 7. optionally delete <rendered_directory>

pub fn get_git_command<T>(
    cmd: &[&str],
    filter: impl Fn(&CommandOutput) -> Result<T, String>,
) -> io::Result<T> {
    let cmd_out = exechelper::execute(cmd)?;
    let filtered = filter(&cmd_out)
        .map_err(|s| io::Error::new(io::ErrorKind::Other, s))?;
    Ok(filtered)
}

pub fn get_all_git_branches() -> io::Result<Vec<String>> {
    let exec_args = [
        "git", "for-each-ref", "--format='%(refname:short)'", "refs/heads/"
    ];
    get_git_command(&exec_args, |cmdout| {
        let branches = cmdout.stdout.split('\n')
            .map(|n| n.to_string()).collect::<Vec<String>>();
        if branches.is_empty() {
            return Err("Failed to find any git branches. Are you sure you're in a git repository?".into())
        }
        Ok(branches)
    })
}

pub fn new_err<M: AsRef<str>>(message: M) -> io::Error {
    io::Error::new(io::ErrorKind::Other, message.as_ref())
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
    // TODO: this needs to be ran from the root of the repo
    let exec_args = [
        "git", "log", main_ref_branch_name, "--date=unix", "--pretty-format:%cd", "--", blog_file_path,
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

pub fn get_all_blog_files_changed_since_last_blog_update(
    blog_branch_name: &str, main_ref_branch_name: &str,
    blog_file_name: &str,
) -> io::Result<Vec<BlogFile>> {
    let files_changed = get_all_files_changed_since_last_blog_update(blog_branch_name, main_ref_branch_name)?;
    let mut out_vec = vec![];
    for file in &files_changed {
        if file.ends_with(blog_file_name) {
            let update_timestamps = get_all_timestamps_of_file_commits(file, main_ref_branch_name)?;
            out_vec.push(BlogFile {
                path_from_root: file.to_owned(),
                update_timestamps,
            });
        }
    }

    Ok(out_vec)
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

pub fn handle_branch_missing(
    cli: &Cli,
    branch_name: &str,
    main_ref_branch_name: &str,
    branch_list: &Vec<String>,
) -> io::Result<String> {
    let mut choices = interact::InteractChoices::from(
        &[format!("Create branch '{}' for me", branch_name),
        "use a different branch name".into(),
        "exit".into()][..]
    );
    let description = format!(
        "We failed to find the blog branch '{}'\nWould you like to:",
        cli.blogs_branch_name
    );
    choices.description = Some(description);
    let selected = interact::interact_number(choices)?;

    // 1. create it for me
    // 2. use a different blog branch name
    // 3. exit
    // TODO: handle 1 and 2
    let use_branch = match selected {
        1 => {
            let first_commit_of_ref_branch = get_first_commit_of_branch(main_ref_branch_name)?;
            make_git_branch(branch_name, &first_commit_of_ref_branch)?;
            branch_name.to_owned()
        },
        2 => {
            let word_choice = interact::InteractChoices::choose_word("Enter a name of a branch to use as the blog branch.\nIf this branch does not exist, it will be created");
            let branch = interact::interact_word(word_choice)?;
            if !branch_list.contains(&branch) {
                // make it if it doesnt exist
                let first_commit_of_ref_branch = get_first_commit_of_branch(main_ref_branch_name)?;
                make_git_branch(&branch, &first_commit_of_ref_branch)?;
            }

            branch
        }
        _ => return Err(new_err("Exiting...")),
    };

    Ok(use_branch)
}

pub fn handle_multiple_main_branches(description: String) -> io::Result<&'static str> {
    let mut choices = interact::InteractChoices::from(&["main", "master"][..]);
    choices.description = Some(description);
    let selected = interact::interact_number(choices)?;
    // 1. main
    // 2. master
    if selected == 1 {
        Ok("main")
    } else {
        Ok("master")
    }
}

pub fn get_main_reference_branch(cli: &Cli, branch_list: &Vec<String>) -> io::Result<String> {
    let search_for_main_branch: Vec<String> = match &cli.main_branch_name {
        Some(b) => vec![b.to_owned()],
        None => vec!["main".into(), "master".into()],
    };

    // if we are checking for both master and main, we have to make sure
    // that only ONE of those is present, otherwise ask user which one they
    // want to use
    let has_all = search_for_main_branch.iter().all(|b| branch_list.contains(&b));
    let has_any = search_for_main_branch.iter().any(|b| branch_list.contains(&b));
    let potential_err = if search_for_main_branch.len() == 2 && has_all {
        "Looks like you have both master and main branches\nThis program does not know which one you wish to use as the main reference branch".into()
    } else if search_for_main_branch.len() == 2 && !has_any {
        "Failed to find either master or main branch".into()
    } else {
        format!("Failed to find the reference branch: {}", search_for_main_branch[0])
    };

    // if user has both master/main branches, and running without interactivity
    // then exit.
    // also if user does not have the branch that they specified with --main-branch-name
    // then also exit.
    // also if the user does not have either of master/main then also exit.
    if (has_all && search_for_main_branch.len() == 2 && cli.no_interactive) || !has_all && search_for_main_branch.len() == 1 || !has_any && search_for_main_branch.len() == 2 {
        eprintln!("{}", potential_err);
        eprintln!("Please run this program again with the --main-branch-name <name> argument");
        eprintln!("where <name> should be the name of an existing branch");
        eprintln!("to explicitly specify the branch you wish to use as the reference branch");
        return Err(new_err("Failed to find reference branch"));
    }

    // otherwise lets try to interactively help the user out
    let use_branch = if has_all && search_for_main_branch.len() == 2 {
        // first case is if theres both main/master, so lets select the desired one
        handle_multiple_main_branches(potential_err)?
    } else if search_for_main_branch.len() == 2 {
        // otherwise, if we are looking for either master/main
        // and we know we have one of them, then lets try to find it now:
        if branch_list.contains(&search_for_main_branch[0]) {
            &search_for_main_branch[0]
        } else if branch_list.contains(&search_for_main_branch[1]) {
            &search_for_main_branch[1]
        } else {
            return Err(new_err("Failed to find either master/main branch. try again with a --main-branch-name argument"));
        }
    } else {
        // otherwise, by this point, it should be assumed
        // that we have the main branch that the user specified:
        if branch_list.contains(&search_for_main_branch[0]) {
            &search_for_main_branch[0]
        } else {
            let err_str = format!("Failed to find branch '{}' try again with a --main-branch-name that exists", &search_for_main_branch[0]);
            return Err(new_err(err_str));
        }
    };

    Ok(use_branch.to_owned())
}

pub fn run_cli(cli: Cli) -> io::Result<()> {
    let branch_list = get_all_git_branches()?;
    let main_ref_branch = get_main_reference_branch(&cli, &branch_list)?;

    let blogs_branch_name = if !branch_list.contains(&cli.blogs_branch_name) {
        if cli.no_interactive {
            eprintln!("Blog branch '{}' does not exist.", cli.blogs_branch_name);
            eprintln!("Verify that '{}' is what you wish to use as your blog branch name.", cli.blogs_branch_name);
            eprintln!("If you do not want to use that as your blog branch name, then run this");
            eprintln!("command again with a different --blogs-branch-name <name> argument");
            return Err(new_err("Failed to find blog branch"));
        }

        // if user specified a blogs branch that doesnt exist (or the default doesnt exist)
        // offer them to either create it for them, or to make a different one
        handle_branch_missing(&cli, &cli.blogs_branch_name, &main_ref_branch, &branch_list)?
    } else {
        cli.blogs_branch_name
    };

    let updated_blogs = get_all_blog_files_changed_since_last_blog_update(&blogs_branch_name, &main_ref_branch, &cli.blog_file_name)?;

    Ok(())
}

fn main() {
    let opts = <Cli as Options>::parse_args_default_or_exit();
    if let Err(e) = run_cli(opts) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}
