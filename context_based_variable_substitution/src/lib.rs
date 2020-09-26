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

// same TODO as above.. should we allow custom/dynamic ways
// to detect default variable substitutions?
pub fn capture_default(text: &str) -> regex::CaptureMatches {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(..*?)\x20(\|+)\x20(..*?)$").unwrap();
    }
    RE.captures_iter(text)
}

pub enum DefaultType {
    DefaultNone,
    DefaultString(String, String),
    DefaultKey(String, String),
}
use DefaultType::*;

pub fn try_get_default(key: &str) -> DefaultType {
    let mut default_replace = DefaultNone;
    for default_cap in capture_default(key) {
        // for capturing default, we assume the capture
        // group should only contain:
        // [0]: the_origina_key | default value
        // [1]: the_origina_key
        // [2]: |
        // [3]: default value
        // its important to capture [2] because it tells us
        // if the default should be treated as a key, or as a string
        if default_cap.len() == 4 {
            let original_key = &default_cap[1];
            let default_seperator = &default_cap[2];
            let default_cap_string = &default_cap[3];
            // let out_val = (original_key.into(), default_cap_string.into());
            if default_seperator.len() > 1 {
                // this is a dynamic default.. ie: the
                // default is a key that needs to be resolved
                default_replace = DefaultKey(original_key.into(), default_cap_string.into());
            } else {
                // this is a static default
                default_replace = DefaultString(original_key.into(), default_cap_string.into());
            }
        }
    }

    default_replace
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

        // we need to set key to the output of try_get_default
        // because if the first capture group was formatted
        // with a default, then the 'key' isnt actually the key
        // we want.. it is: "key | default", so we need to parse out the actual "key"
        let default_type = try_get_default(key);
        let key = match default_type {
            DefaultNone => key.into(),
            DefaultString(ref k, _) => k.clone(),
            DefaultKey(ref k, _) => k.clone(),
        };

        if let Some(replace_with) = context.get_value_from_key(key.as_str()) {
            replacements.push((replace_str.to_owned(), replace_with));
            continue;
        }

        match default_type {
            DefaultNone => (),
            DefaultString(_, default_value) => {
                replacements.push((replace_str.to_owned(), default_value));
                continue;
            },
            DefaultKey(_, try_key) => {
                // dynamic key usage:
                if let Some(replace_with) = context.get_value_from_key(try_key.as_str()) {
                    replacements.push((replace_str.to_owned(), replace_with));
                    continue;
                }
            }
        }

        match failure_mode {
            FM_ignore => (),
            FM_panic => panic!("Failed to get context value from key: {}", key),
            FM_default(ref default) => replacements.push((replace_str.to_owned(), default.clone())),
        }
    }

    for (replace_str, replace_with) in replacements {
        s = s.replace(&replace_str[..], &replace_with[..]);
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;
    struct MyContext {}
    impl Context for MyContext {
        fn get_value_from_key(&self, key: &str) -> Option<String> {
            if key == "custom_key" {
                Some("custom_context".into())
            } else {
                None
            }
        }
    }

    #[test]
    fn returns_as_is_if_nothing_to_replace() {
        let context: Vec<String> = vec![];
        let text = r#"
        dsadsa
        dsadsadsa dsadsadsa  {{ something? }}
         dsads dsadsadsa ${{notreplacedbecausenospaces}}
        "#;
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic);
        assert_eq!(text.to_string(), replaced);
    }

    #[test]
    fn replaces_from_simple_vec_contexts() {
        let my_path = "hello";
        let context = vec![my_path];
        let text = "this is my ${{ 0 }} world";
        let expected = "this is my hello world";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn replaces_from_custom_contexts() {
        let context = MyContext {};
        let text = "this is my ${{ custom_key }}";
        let expected = "this is my custom_context";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    #[should_panic(expected = "Failed to get context value from key: nonexistant")]
    fn failure_mode_panic_if_failed_to_find_value_for_key() {
        let context = MyContext {};
        let text = "this is my ${{ nonexistant }}";
        let expected = "this is my custom_context";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic);
    }

    #[test]
    fn should_leave_failed_key_if_mode_is_ignore() {
        let context = MyContext {};
        let text = "this is my ${{ nonexistant }}";
        let expected = "this is my ${{ nonexistant }}";
        let replaced = replace_all_from(text, &context, FailureMode::FM_ignore);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn can_use_default_value_if_mode_is_default() {
        let context = MyContext {};
        let text = "this is my ${{ nonexistant }}";
        let expected = "this is my global_default";
        let replaced = replace_all_from(text, &context, FailureMode::FM_default("global_default".into()));
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn can_use_default_syntax_from_text() {
        let context = MyContext {};
        let text = "this is my ${{ nonexistant | syntax_default }}";
        let expected = "this is my syntax_default";
        // even though we give a global default, we provide a local default
        // so it should use the local default
        let replaced = replace_all_from(text, &context, FailureMode::FM_default("global_default".into()));
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn can_use_default_dynamically() {
        let context = MyContext {};
        // this syntax should allow parsing the default as another key
        // which would allow dynamic defaults
        let text = "this is my ${{ nonexistant || custom_key }}";
        let expected = "this is my custom_context";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn default_dynamic_doesnt_override_original_key() {
        let context = vec!["abc", "xyz"];
        // a dynamic default should not be used if the original
        // key works
        let text = "this is my ${{ 0 || 1 }}";
        let expected = "this is my abc";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic);
        assert_eq!(expected.to_string(), replaced);
    }
}
