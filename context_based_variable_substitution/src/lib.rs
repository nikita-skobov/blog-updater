use std::string::ToString;
use regex::Regex;
use lazy_static::lazy_static;

// TODO:
// do I want this to be dynamic?
// do I want users of this library to customize the parameter syntax?
// might be useful if this lib is to truly be generic...
// for now it only supports this syntax: ${{ }}
pub fn capture_parameter(text: &str) -> regex::CaptureMatches {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"\$\{\{\x20(..*?)\x20\}\}").unwrap();
    }
    RE.captures_iter(text)
}

pub trait Context {
    fn get_value_from_key(&self, key: &str) -> Option<String>;
}

// some specific implementations we should make so that
// the user doesnt have to make some thin struct to wrap
// some basic types... this is because rust does not allow
// private impl of external traits:
// https://github.com/rust-lang/rfcs/issues/493

// for this one, we assume that the key is something
// that can be parsed into a usize so that we can
// index the Vec<T>. we also ~~assume~~ require that T can be
// converted into a string value
// TODO: figure out how to make this work for anything of [T], or &[..]
// i think its something like this, but I havent gotten it to work yet:
// impl<'a, T> Context for T where T: AsRef<[&'a str]>
impl<T: ToString> Context for Vec<T> {
    fn get_value_from_key(&self, key: &str) -> Option<String> {
        if let Ok(key_usize) = key.parse::<usize>() { match key_usize < self.len() {
            true => Some(self[key_usize].to_string()),
            false => None,
        } } else {
            None
        }
    }
}


// an enum of options of what to do if the replace all
// functions fail to replace a match, ie: if the context does
// not contain the keyword we are replacing
pub enum FailureMode {
    FM_ignore,
    FM_panic,
    FM_default(String),

    // TODO:
    // add a callback variant that has an associated value
    // which is a callback function that can call back to the user
    // and ask: "hey, this key failed. what do you want to do?"
    // im leaving this TODO for now because I don't know if this is possible
    // callbacks are hard :/
    // FM_callback,
}

// text is the text you wish to replace
// context is some data structure that implements the
// Context trait. basically pass in something that this function
// can ask how to get a value from a key.
// parse_by_line is whether or not we should iterate over the lines
// of the text, or just parse all in one go. the advantage to
// parsing
pub fn replace_all_from(
    text: &str,
    context: &impl Context,
    failure_mode: FailureMode,
) -> String {
    use FailureMode::*;

    let mut replacements = vec![];
    let mut s: String = text.clone().to_string();
    for cap in capture_parameter(text) {
        let replace_str = &cap[0];
        let key = &cap[1];
        if let Some(replace_with) = context.get_value_from_key(key) {
            replacements.push((replace_str.to_owned(),replace_with));
            continue;
        }
        match failure_mode {
            FM_ignore => (),
            FM_panic => panic!("Failed to get contex value from key: {}", key),
            FM_default(ref default) => replacements.push((replace_str.to_owned(), default.clone())),
        }
    }

    for (replace_str, replace_with) in replacements {
        s = s.replace(&replace_str[..], &replace_with[..]);
    }

    s
}
