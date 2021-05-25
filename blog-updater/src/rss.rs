use super::BlogFile;
use super::BlogConfig;

pub const RSS_ENDING: &str = "rss.xml";

pub fn rss_err(blog_file: &BlogFile, missing: &str) -> String {
    let err_msg = format!("Blog post {} is missing {}", blog_file.path_from_root, missing);
    err_msg
}

pub fn generate_rss_item(
    blog_config: &BlogConfig,
    blog_file: &BlogFile,
) -> Result<String, String> {
    let title = match &blog_config.title {
        Some(s) => s,
        None => return Err(rss_err(blog_file, "title"))
    };
    let blog_home_url = match &blog_config.blog_home_url {
        Some(s) => s,
        None => return Err(rss_err(blog_file, "blog_home_url"))
    };
    let blog_file_name = match &blog_config.blog_file_name {
        Some(s) => s,
        None => return Err(rss_err(blog_file, "blog_file_name (this is supposed to be generated for you, but somehow we failed to parse the blog file name?)"))
    };
    let description = match &blog_config.description {
        Some(s) => s,
        None => return Err(rss_err(blog_file, "description"))
    };
    let written_timestamp = blog_file.written;
    if written_timestamp == 0 {
        return Err(rss_err(blog_file, "a timestamp of when it was written. Are you sure this file was committed into git?"));
    }

    // has to be formatted like:
    // Mon, 24 May 2021 00:00:00 +0000
    let naive = chrono::NaiveDateTime::from_timestamp(written_timestamp, 0);
    let datetime: chrono::DateTime<chrono::Utc> = chrono::DateTime::from_utc(naive, chrono::Utc);
    let human_date = datetime.format("%a, %d %B %Y %H:%M:%S %z").to_string();

    let link = format!("{}/{}", blog_home_url, blog_file_name);

    let rss_item = format!("
    <item>
    <title>{}</title>
    <link>{}</link>
    <pubDate>{}</pubDate>
    <guid>{}</guid>
    <description>{}</description>
    </item>",
    title,
    link,
    human_date,
    link,
    description,
    );

    Ok(rss_item)
}

pub fn generate_rss(
    blog_config: &BlogConfig,
    blog_file: &BlogFile,
    rss_items_xml: &str
) -> Result<String, String> {
    let title = match &blog_config.blog_name {
        Some(s) => s,
        None => return Err(rss_err(blog_file, "blog_name"))
    };
    let blog_home_url = match &blog_config.blog_home_url {
        Some(s) => s,
        None => return Err(rss_err(blog_file, "blog_home_url"))
    };
    let description = match &blog_config.blog_description {
        Some(s) => s,
        None => return Err(rss_err(blog_file, "description"))
    };
    let right_now = std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Failed to get system time: {}", e))?;
    let right_now = right_now.as_secs() as i64;
    let naive = chrono::NaiveDateTime::from_timestamp(right_now, 0);
    let datetime: chrono::DateTime<chrono::Utc> = chrono::DateTime::from_utc(naive, chrono::Utc);
    let human_date = datetime.format("%a, %d %B %Y %H:%M:%S %z").to_string();

    let rss_location = format!("{}/{}", blog_home_url, RSS_ENDING);

    let rss_xml = format!("
    <rss xmlns:atom=\"http://www.w3.org/2005/Atom\" version=\"2.0\">
    <channel>
    <title>{}</title>
    <link>{}</link>
    <description>{}</description>
    <generator>blog-updater github.com/nikita-skobov/blog-updater</generator>
    <lastBuildDate>{}</lastBuildDate>
    <atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\"></atom:link>
    {}
    </channel>
    </rss>",
    title,
    blog_home_url,
    description,
    human_date,
    rss_location,
    rss_items_xml,
    );

    Ok(rss_xml)
}
