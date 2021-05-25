use gumdrop::Options;
use std::path::PathBuf;
use std::io::prelude::*;
use std::{collections::HashMap, io};
use exechelper::CommandOutput;
use serde::Deserialize;
use pulldown_cmark::{Parser, html};
use context_based_variable_substitution::{ replace_all_from_ex, FailureModeEx };
use simple_interaction as interact;
use chrono;

#[derive(Debug, Options)]
pub struct Cli {
    #[options(default = "blogs")]
    pub blogs_branch_name: String,
    #[options(default = "BLOG.md")]
    pub blog_file_name: String,
    /// by default we will look for either a 'master' or a 'main' branch. otherwise, if you want to use some other specific branch as the main branch then you can specify this with --main-branch-name <name>.
    pub main_branch_name: Option<String>,

    /// specify the path to your blog config that should contain things like your blog's name, URL, author's name, etc... if a blog config is not provided, this information can also be provided in the header of the blog file itself
    pub blog_config_path: Option<PathBuf>,

    /// specify the html template to use to transpile the rendered markdown into. if not specified, the default template will be used.
    pub template_path: Option<PathBuf>,

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

/// not all of these properties should be in your blog_config
/// for example, it doesnt make sense for title to be there.
/// however, for convenience Im using the same struct to represent the
/// blog config as well as the data we get from the blog file itself,
/// because there is potentially some overlap there.
#[derive(Deserialize, Debug, Default, Clone)]
pub struct BlogConfig {
    // these probably should only come from the blog file:
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub date_written: Option<String>,
    pub date_updated: Option<String>,
    
    // generated:
    pub blog_file_name: Option<String>,
    pub published_time_iso: Option<String>,
    pub modified_time_iso: Option<String>,


    // these probably only should come from the blog config:
    pub author_name: Option<String>,
    pub author_url: Option<String>,
    pub author_email: Option<String>,
    pub author_projects_url: Option<String>,
    pub blog_name: Option<String>,
    pub blog_home_url: Option<String>,
}

impl BlogConfig {
    pub fn apply(&mut self, other: BlogConfig) {
        if let Some(s) = other.title {
            self.title = Some(s);
        }
        if let Some(s) = other.description {
            self.description = Some(s);
        }
        if !other.tags.is_empty() {
            self.tags = other.tags;
        }
        if let Some(s) = other.date_written {
            self.date_written = Some(s);
        }
        if let Some(s) = other.date_updated {
            self.date_updated = Some(s);
        }
        if let Some(s) = other.author_name {
            self.author_name = Some(s);
        }
        if let Some(s) = other.author_url {
            self.author_url = Some(s);
        }
        if let Some(s) = other.author_email {
            self.author_email = Some(s);
        }
        if let Some(s) = other.author_projects_url {
            self.author_projects_url = Some(s);
        }
        if let Some(s) = other.blog_name {
            self.blog_name = Some(s);
        }
        if let Some(s) = other.blog_home_url {
            self.blog_home_url = Some(s);
        }
        if let Some(s) = other.blog_file_name {
            self.blog_file_name = Some(s);
        }
    }

    pub fn to_hashmap_context<'a>(&'a self, markdown: &'a Option<String>) -> HashMap<&'a str, String> {
        let mut context = HashMap::new();
        if let Some(ref m) = markdown {
            context.insert("rendered_markdown", m.clone());
        }
        if let Some(s) = &self.title {
            context.insert("title", s.clone());
        }
        if let Some(s) = &self.description {
            context.insert("description", s.clone());
        }
        if !self.tags.is_empty() {
            let mut meta_tag_str = "".into();
            for tag in &self.tags {
                let this_tag = format!("<meta property=\"article:tag\" content=\"{}\">", tag);
                meta_tag_str = format!("{}{}\n", meta_tag_str, this_tag);
            }
            context.insert("meta_tags", meta_tag_str);
        }
        if let Some(s) = &self.date_written {
            context.insert("date_written", s.clone());
        }
        if let Some(s) = &self.date_updated {
            context.insert("date_updated", s.clone());
        }
        if let Some(s) = &self.author_name {
            context.insert("author_name", s.clone());
            let publisher_tag = format!("<meta property=\"article:publisher\" content=\"{}\">", s.clone());
            context.insert("publisher_tag", publisher_tag);
        }
        if let Some(s) = &self.author_url {
            context.insert("author_url", s.clone());
        }
        if let Some(s) = &self.author_email {
            context.insert("author_email", s.clone());
        }
        if let Some(s) = &self.author_projects_url {
            context.insert("author_projects_url", s.clone());
        }
        if let Some(s) = &self.blog_name {
            context.insert("blog_name", s.clone());
        }
        if let Some(s) = &self.blog_home_url {
            context.insert("blog_home_url", s.clone());
        }
        if let Some(s) = &self.blog_file_name {
            context.insert("blog_file_name", s.clone());
        }
        if let Some(s) = &self.modified_time_iso {
            context.insert("modified_time_iso", s.clone());
        }
        if let Some(s) = &self.published_time_iso {
            context.insert("published_time_iso", s.clone());
        }
        context
    }
}

#[derive(Debug)]
pub struct BlogFile {
    pub path_from_root: String,
    /// the first element should be the most recent update,
    /// ie: use this as the blog post's updated time,
    /// and the last element should be the created at time
    pub update_timestamps: Vec<i64>,
    pub git_author_name: String,
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

pub fn get_all_blog_files_changed_since_last_blog_update(
    blog_branch_name: &str, main_ref_branch_name: &str,
    blog_file_name: &str,
) -> io::Result<Vec<BlogFile>> {
    let files_changed = get_all_files_changed_since_last_blog_update(blog_branch_name, main_ref_branch_name)?;
    let mut out_vec = vec![];
    for file in &files_changed {
        if file.ends_with(blog_file_name) {
            let updates = get_all_timestamps_of_file_commits(file, main_ref_branch_name)?;
            let mut blog_file = BlogFile {
                path_from_root: file.to_owned(),
                update_timestamps: vec![],
                git_author_name: "".into(),
            };
            for update in &updates {
                let comma_index = update.find(",")
                    .map_or(Err(new_err("Failed to parse output of git log")), |i| Ok(i))?;
                let timestamp = &update[0..comma_index];
                let name = &update[(comma_index + 1)..];
                let timestamp_parsed = timestamp.parse::<i64>()
                    .map_err(|_| new_err("Failed to parse timestamp from git log"))?;
                blog_file.update_timestamps.push(timestamp_parsed);
                if blog_file.git_author_name.is_empty() {
                    blog_file.git_author_name = name.trim_start().trim_end().to_string();
                }
            }
            out_vec.push(blog_file);
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
        "Looks like you have both master and main branches\nThis program does not know which one to use".into()
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
        let potential_err = format!("{}\nWhich one would you like to use as the main reference branch?", potential_err);
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

pub fn get_template(template: &Option<PathBuf>) -> io::Result<String> {
    let default_template = include_str!("../templates/default.html");
    match template {
        Some(path) => std::fs::read_to_string(path),
        None => Ok(default_template.to_string())
    }
}

pub fn get_blog_config(path: &Option<PathBuf>) -> io::Result<BlogConfig> {
    match path {
        Some(p) => {
            let file = std::fs::read_to_string(p)?;
            let obj: BlogConfig = serde_json::from_str(&file)?;
            Ok(obj)
        }
        None => Ok(BlogConfig::default())
    }
}

pub fn parse_blog_header_line(line: &str, blog_config: &mut BlogConfig) {
    let first_colon_index = if let Some(ind) = line.find(':') {
        ind
    } else { return; };

    let key = &line[0..first_colon_index];
    let key = key.trim_start().trim_end();
    let value = &line[(first_colon_index + 1)..];
    let value = value.trim_start().trim_end();

    match key {
        "title" => {
            blog_config.title = Some(value.to_owned());
        }
        "tags" => {
            let mut tags = vec![];
            for tag in value.split(',') {
                tags.push(tag.to_string());
            }
            blog_config.tags = tags;
        }
        "author" => {
            blog_config.author_name = Some(value.to_owned());
        }
        "author_email" => {
            blog_config.author_email = Some(value.to_owned());
        }
        "author_projects_url" => {
            blog_config.author_projects_url = Some(value.to_owned());
        }
        "blog_home_url" => {
            blog_config.blog_home_url = Some(value.to_owned());
        }
        "blog_name" => {
            blog_config.blog_name = Some(value.to_owned());
        }
        "date" => {
            blog_config.date_written = Some(value.to_owned());
        }
        "last_updated" => {
            blog_config.date_updated = Some(value.to_owned());
        }
        "blog_file_name" => {
            blog_config.blog_file_name = Some(value.to_owned());
        }
        _ => {},
    }
}

/// blog_file is the raw text of the markdown file that was committed
/// on the blog branch. we now want to extract some information from it
/// and modify it slightly so that we only pass the relevant markdown
/// to our markdown renderer.
/// things we want to extract from blog_file: (in [] means its optional)
/// note that some of these can come from the blog config file. the
/// actual file takes precedence over whats in the blog config.
/// - title
/// - sanitized file name (for the URL)
/// - description of this blog post
/// - author name
/// - [author URL]
/// - [author email]
/// - [author projects URL]
/// - [list of tags]
/// - blog home URL
/// - name of blog home
/// - date string written
/// - [date string updated]
pub fn parse_blog_file_info(blog_file: &str) -> io::Result<(BlogConfig, &str)> {
    let mut config = BlogConfig::default();
    // first line of blog file has to be the
    // header info, otherwise, we assume there is no header
    // and return a default blog config
    if !blog_file.starts_with("---") {
        return Ok((config, blog_file));
    }

    let mut split_index = 0;
    let mut first_line = true;
    for line in blog_file.lines() {
        // plus 1 for the newline
        split_index += line.len() + 1;
        if first_line {
            first_line = false;
            continue;
        }

        if line.starts_with("---") {
            break;
        }

        parse_blog_header_line(line, &mut config);
    }

    Ok((config, &blog_file[split_index..]))
}

/// the blog_text should not include the blog header.
/// it is assumed you already stripped that out before calling this
pub fn get_description(blog_text: &str) -> Option<String> {
    let mut split = blog_text.trim_start().split("\n\n");
    match split.next() {
        Some(s) => Some(s.to_string()),
        None => None,
    }
}

/// used to create a valid url from a title.
/// I think you can have some other characters in the url path
/// but for now, this is the more sensible approach
pub fn replace_with_valid_word(word: &str) -> String {
    let valid_chars = "abcdefghijklmnopqrstuvwxyz0123456789";
    let mut out_word: String = "".into();
    for c in word.chars() {
        if valid_chars.contains(c) {
            out_word.push(c);
        }
    }

    out_word
}

pub fn get_blog_file_name(title: &Option<String>) -> Option<String> {
    if let Some(title) = title {
        let mut out_str: String = "".into();
        for word in title.split_whitespace() {
            let valid_word = replace_with_valid_word(&word.to_lowercase());
            if valid_word.is_empty() {
                continue;
            }
            if !out_str.is_empty() {
                out_str.push('-');
            }
            out_str.push_str(&valid_word);
        }
        Some(out_str)
    } else {
        None
    }
}

pub fn get_date_string_from_timestamp(timestamp: i64) -> (String, String) {
    let naive = chrono::NaiveDateTime::from_timestamp(timestamp, 0);
    let datetime: chrono::DateTime<chrono::Utc> = chrono::DateTime::from_utc(naive, chrono::Utc);
    let human_date = datetime.format("%B %d, %Y").to_string();
    let iso = datetime.to_rfc3339().to_string();
    (human_date, iso)
}

pub fn get_name_and_date_html(blog_info: &BlogConfig) -> String {
    let name_url = match &blog_info.author_url {
        Some(s) => s,
        None => "#",
    };
    let name = match &blog_info.author_name {
        Some(s) => s,
        None => "AUTHORNAMENOTFOUND",
    };
    let human_date = match &blog_info.date_written {
        Some(s) => s,
        None => "DATESTRINGNOTFOUND",
    };
    format!("<span style=\"color: #92979b; font-size: 16px\"><a style=\"font-weight: bold; color: #92979b\" href=\"{}\">{}</a> - {}</span>", name_url, name, human_date)
}

pub fn get_about_me_markdown(blog_info: &BlogConfig) -> String {
    let mut use_about_me = false;
    let mut about_me = "About me:\n\n".into();

    if let Some(s) = &blog_info.author_name {
        about_me = format!("{}> I am {}.<br>\n", about_me, s);
        use_about_me = true;
    }
    if let Some(s) = &blog_info.author_email {
        about_me = format!("{}> Contact me via email: {}.<br>\n", about_me, s);
        use_about_me = true;
    }
    if let Some(s) = &blog_info.author_projects_url {
        about_me = format!("{}> Check out my projects: {}.<br>\n", about_me, s);
        use_about_me = true;
    }
    if let Some(s) = &blog_info.blog_home_url {
        about_me = format!("{}> Check out my other blog posts: {}.<br>\n", about_me, s);
        use_about_me = true;
    }

    if use_about_me {
        about_me
    } else {
        "".into()
    }
}

pub fn render_blog_actual(
    blog_file: &str,
    updated_blog: &BlogFile,
    template: &str,
    blog_config: &mut BlogConfig,
) -> io::Result<String> {
    let (blog_info, rest_of_blog_file) = parse_blog_file_info(&blog_file)?;
    // make a clone of the global blog config
    let mut this_blog_info = blog_config.clone();
    // and then apply the blog information of this blog file
    // onto the cloned config
    this_blog_info.apply(blog_info);

    // some validation: title is required, and
    // we should have at least one entry in the updated blogs timestamps:
    if this_blog_info.title.is_none() {
        return Err(new_err(format!("Missing title for blog {}", updated_blog.path_from_root)));
    }
    if updated_blog.update_timestamps.is_empty() {
        return Err(new_err(format!("Failed to find a single commit for blog {}", updated_blog.path_from_root)));
    }

    // now our 'this_blog_info' contains everything
    // from the global blog config, and everything in the blog header
    // but what we probably cant/didnt get from the blog header is:
    // - description
    // - sanitized file name
    // also if we failed to get author's name, or the date strings,
    // we will get them here via the information we have from git:

    if this_blog_info.description.is_none() {
        this_blog_info.description = get_description(rest_of_blog_file);
    }
    if this_blog_info.blog_file_name.is_none() {
        this_blog_info.blog_file_name = get_blog_file_name(&this_blog_info.title);
    }
    if this_blog_info.author_name.is_none() {
        this_blog_info.author_name = Some(updated_blog.git_author_name.clone());
    }
    if this_blog_info.date_written.is_none() {
        // the last entry in the array is the first commit
        let first_update_index = updated_blog.update_timestamps.len() - 1;
        let first_update = &updated_blog.update_timestamps[first_update_index];
        let (human_date, iso_date) = get_date_string_from_timestamp(*first_update);
        this_blog_info.date_written = Some(human_date);
        this_blog_info.published_time_iso = Some(iso_date.clone());
        this_blog_info.modified_time_iso = Some(iso_date);
    }
    if this_blog_info.date_updated.is_none() && updated_blog.update_timestamps.len() > 1 {
        // the first entry is the most recent commit, ie: latest update
        let last_update = &updated_blog.update_timestamps[0];
        let (human_date, iso_date) = get_date_string_from_timestamp(*last_update);
        this_blog_info.date_updated = Some(human_date);
        this_blog_info.modified_time_iso = Some(iso_date);
    }

    // we append some text to the markdown string before we render it
    // to html. this includes the blog name and date, the title,
    // and an about me section (depending if user supplied necessary information for about me)
    let blog_name_and_date = get_name_and_date_html(&this_blog_info);
    let about_me = get_about_me_markdown(&this_blog_info);
    let use_title = if let Some(t) = &this_blog_info.title { format!("# {}", t) } else { "# title".into() };
    let render_this = format!("{}\n{}\n{}\n\n\n{}", use_title, blog_name_and_date, rest_of_blog_file, about_me);

    // now we should have all the information we need
    // we first create an html string from the rest of the markdown text
    // after we removed the blog header:
    let parser = Parser::new(&render_this);
    let mut html_out = String::from("");
    html::push_html(&mut html_out, parser);

    // then we transclude the blog information and the rendered markdown
    // into the template:
    let markdown_rendered = Some(html_out);
    let replace_context = this_blog_info.to_hashmap_context(&markdown_rendered);
    let transcluded = replace_all_from_ex(
        &template, &replace_context, FailureModeEx::FM_callback(|_key| {
            // TODO: handle cases where we do not have that key
            Some("".into())
        }), None);
    Ok(transcluded)
}


pub fn render_blog_to_string(
    updated_blog: &BlogFile,
    template: &str,
    blogs_branch_name: &str,
    blog_config: &mut BlogConfig,
) -> io::Result<String> {
    let blog_file = get_blog_file_from_branch(&updated_blog.path_from_root, &blogs_branch_name)?;
    let rendered = render_blog_actual(&blog_file, updated_blog, template, blog_config)?;
    Ok(rendered)
}

pub fn run_cli(cli: Cli) -> io::Result<()> {
    let git_root = get_git_toplevel_absolute_path()?;
    std::env::set_current_dir(git_root)?;
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
    let template = get_template(&cli.template_path)?;
    let mut blog_config = get_blog_config(&cli.blog_config_path)?;

    for updated_blog in &updated_blogs {
        let rendered = render_blog_to_string(updated_blog, &template, &blogs_branch_name, &mut blog_config)?;
    }

    Ok(())
}

pub fn real_main() -> io::Result<()> {
    let current_dir = std::env::current_dir()?;
    let opts = <Cli as Options>::parse_args_default_or_exit();
    if let Err(e) = run_cli(opts) {
        eprintln!("{}", e);
        // no matter what, go back to the directory we were at the start
        let _ = std::env::set_current_dir(current_dir);
        std::process::exit(1);
    }
    std::env::set_current_dir(current_dir)?;

    Ok(())
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("Some catostrophic error occurred: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Parser, html};
    use context_based_variable_substitution::{ Context, replace_all_from, FailureMode };
    use std::collections::HashMap;

    #[derive(Debug, Default)]
    struct MyContext {
        pub keys: HashMap<String, String>,
    }
    impl Context for MyContext {
        fn get_value_from_key(&self, key: &str, _syntax_char: char) -> Option<String> {
            match self.keys.get(key) {
                Some(s) => Some(s.clone()),
                None => None,
            }
        }
    }

    fn markdowntest1_actual() -> io::Result<()> {
        let data = std::fs::read_to_string("test/markdown.md")?;
        let template = std::fs::read_to_string("test/template.html")?;
        let parser = Parser::new(&data);
        let mut html_out = String::from("");
        html::push_html(&mut html_out, parser);
        let mut replace_context = MyContext::default();
        replace_context.keys.insert("markdownRendered".into(), html_out);

        let replaced = replace_all_from(&template, &replace_context, FailureMode::FM_panic, None);
        println!("{}\n", replaced);
        // the markdown file contains the helloworld string, after variable
        // substitution we want to ensure that the string is included
        // in the final output
        assert!(replaced.contains("<h1>helloworld</h1>"));
        Ok(())
    }

    #[test]
    fn markdowntest1() {
        markdowntest1_actual().unwrap();
    }

    #[test]
    fn parse_blog_header_works() {
        let blog_file = "---\ntitle: hello\n---\nrest of blog file here";
        let (blog_config, rest_of_blog_file) = parse_blog_file_info(blog_file).unwrap();
        assert_eq!(blog_config.title, Some("hello".into()));
        assert_eq!(rest_of_blog_file, "rest of blog file here");
    }

    fn markdowntest2_actual() -> io::Result<()> {
        let data = std::fs::read_to_string("test/m2.md")?;
        let template = std::fs::read_to_string("templates/default.html")?;
        let blog_file_info = BlogFile {
            path_from_root: "doesntmatter".into(),
            update_timestamps: vec![1621897682],
            git_author_name: "me".into(),
        };
        let mut blog_config = BlogConfig::default();
        blog_config.tags = vec!["abcxyz".into()];
        let rendered = render_blog_actual(
            &data, &blog_file_info, &template, &mut blog_config)?;
        println!("\n{}\n", rendered);

        let expected_tag = "<meta property=\"article:tag\" content=\"abcxyz\">";
        assert!(rendered.contains("<title>m2title</title>"));
        assert!(rendered.contains("description\" content=\"m2description"));
        assert!(rendered.contains("2021-05-24")); // this is the timestamp above
        assert!(rendered.contains(expected_tag));
        assert!(rendered.contains("May 24, 2021")); // this is the human readable one
        Ok(())
    }

    #[test]
    fn markdowntest2() {
        markdowntest2_actual().unwrap();
    }
}
