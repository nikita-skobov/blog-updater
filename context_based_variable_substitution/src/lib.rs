use std::string::ToString;
use regex::Regex;
use lazy_static::lazy_static;

// TODO:
// do I want this to be more dynamic?
// right now it allows any sort of syntax character
// like ${{ }} or !{{ }}, @{{ }}, etc..
pub fn capture_parameter_of_type(text: &str) -> regex::CaptureMatches {
    // first matching group: (\S) will capture any single non-whitespace character
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(\S)\{\{\x20(..*?)\x20\}\}").unwrap();
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

/// a context can be defined on any data structure. The context trait allows the user
/// to define what keys, and what syntax chars are relevant, and then how to
/// get a value from a replacement key. Consider the example:
/// ${{ my_env_variable }}
/// when your context is called, you will get key = "my_env_variable", and
/// syntax_char = '$', and from there you can figure out the appropriate way to
/// return a string to replace
pub trait Context {
    fn get_value_from_key(&self, key: &str, syntax_char: char) -> Option<String>;
}

// some specific implementations we should make so that
// the user doesnt have to make some thin struct to wrap
// some basic types... this is because rust does not allow
// private impl of external traits:
// https://github.com/rust-lang/rfcs/issues/493

/// for this one, we assume that the key is something
/// that can be parsed into a usize so that we can
/// index the Vec<T>. we also ~~assume~~ require that T can be
/// converted into a string value
/// TODO: figure out how to make this work for anything of [T], or &[..]
/// i think its something like this, but I havent gotten it to work yet:
/// impl<'a, T> Context for T where T: AsRef<[&'a str]>
impl<T: ToString> Context for Vec<T> {
    fn get_value_from_key(&self, key: &str, syntax_char: char) -> Option<String> {
        if let Ok(key_usize) = key.parse::<usize>() { match key_usize < self.len() {
            true => Some(self[key_usize].to_string()),
            false => None,
        } } else {
            None
        }
    }
}

/// an enum of options of what to do if the replace all
/// functions fail to replace a match, ie: if the context does
/// not contain the keyword we are replacing
pub enum FailureModeEx<F: FnMut(&String) -> String> {
    FM_ignore,
    FM_panic,
    FM_default(String),
    FM_callback(F),
}

/// an enum of options of what to do if the replace all
/// functions fail to replace a match, ie: if the context does
/// not contain the keyword we are replacing
pub enum FailureMode {
    FM_ignore,
    FM_panic,
    FM_default(String),
}

impl<F: FnMut(&String) -> String> From<FailureMode> for FailureModeEx<F> {
    fn from(orig: FailureMode) -> Self {
        match orig {
            FailureMode::FM_ignore => FailureModeEx::FM_ignore,
            FailureMode::FM_panic => FailureModeEx::FM_panic,
            FailureMode::FM_default(s) => FailureModeEx::FM_default(s),
        }
    }
}

/// Text is the text you wish to replace.
/// context is some data structure that implements the
/// Context trait. Basically pass in something that this function
/// can ask how to get a value from a key.
/// Failure mode determines what to do if it finds a match of
/// the parameter substitution syntax, but couldn't find a key to substitute.
/// Lastly, valid_syntax_chars is an array of chars that should be allowed
/// allowed to try to get keys from, this can be useful if your application
/// only wants to look at ${{ }} syntax at one stage, and at another
/// stage, only evaluate !{{ }}
/// # Example 1:
/// ```
/// // where SomeContextImpl would need to
/// // implement the Context trait
/// let context = SomeContextImpl { my_var: "world" };
/// s = replace_all_from("hello ${{ my_var }}", context, FailureMode::FM_panic, None)
/// assert_eq!(s, "hello world")
/// ```
/// # Example 2:
/// by default, any vec of str, or String has context implemented for it.
/// This example also shows you can use defaults with a single pipe character
/// or dynamic defaults with a double pipe. the dynamic defaults will interpret
/// the thing after the double pipe as a key to try to get a value for
/// ```
/// let context = vec!["zero", "one", "two", "three"];
/// s = replace_all_from(
///    "${{ 0 }} default:${{ 4 | constant here }} dynamic_default:${{ 4 || 3 }}",
///    context, FailureMode::FM_panic, None);
/// assert_eq!(s, "zero  default:constant here dynamic_default:three")
/// ```
pub fn replace_all_from(
    text: &str,
    context: &impl Context,
    failure_mode: FailureMode,
    valid_syntax_chars: Option<&str>,
) -> String {
    let mut failure_mode_ex = failure_mode.into();
    // TODO: how to properly specify: I dont care about the
    // type of the callback for failure mode ex?
    // in this case, we are tricking the compiler which is quite odd
    // and ugly. ideally I can specify something like:
    // failure_mode_ex: FailureModeEx<_>
    // but idk how to do that correctly
    if false {
        failure_mode_ex = FailureModeEx::FM_callback(|s| s.clone());
    }
    replace_all_from_ex(text, context, failure_mode_ex, valid_syntax_chars)
}

/// Like replace_all_from, but you can use an additional failure mode which is
/// `FailureModeEx` which allows specifying a failure mode of a callback
/// to replace a key if not found.
pub fn replace_all_from_ex<F: FnMut(&String) -> String>(
    text: &str,
    context: &impl Context,
    failure_mode: FailureModeEx<F>,
    valid_syntax_chars: Option<&str>,
) -> String {
    let mut failure_mode = failure_mode;
    let mut replacements = vec![];
    let mut s: String = text.clone().to_string();
    // by default only examine '$'
    let valid_chars = valid_syntax_chars.unwrap_or("$");
    let valid_chars: Vec<char> = valid_chars.chars().collect();
    for cap in capture_parameter_of_type(text) {
        let replace_str = &cap[0];
        let syntax_char = &cap[1];
        let key = &cap[2];
        // I don't think its possible for chars.nth(0) to be None
        // because if we are here that means we DID find a match...
        let syntax_char = syntax_char.chars().nth(0).unwrap();

        // if the current matches syntax char
        // is not one of the valid ones provided, then
        // skip this capture
        if ! valid_chars.contains(&syntax_char) {
            continue;
        }

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

        // ask the provided context if
        // the current key has a value to be replaced
        if let Some(replace_with) = context.get_value_from_key(key.as_str(), syntax_char) {
            replacements.push((replace_str.to_owned(), replace_with));
            continue;
        }

        // if not, then try the default
        match default_type {
            DefaultNone => (),
            DefaultString(_, default_value) => {
                replacements.push((replace_str.to_owned(), default_value));
                continue;
            },
            DefaultKey(_, try_key) => {
                // dynamic key usage:
                if let Some(replace_with) = context.get_value_from_key(try_key.as_str(), syntax_char) {
                    replacements.push((replace_str.to_owned(), replace_with));
                    continue;
                }
            }
        }

        // if that failed, then use the provided failure mode
        match failure_mode {
            FailureModeEx::FM_ignore => (),
            FailureModeEx::FM_panic => panic!("Failed to get context value from key: {}", key),
            FailureModeEx::FM_default(ref default) => replacements.push((replace_str.to_owned(), default.clone())),
            FailureModeEx::FM_callback(ref mut cb) => {
                let replace_with = cb(&key);
                replacements.push((replace_str.to_owned(), replace_with));
            }
        }
    }

    // perform all the replacements at the end
    // the above loop just populates a replacement vec
    // and its easier to iterate over this at the end
    for (replace_str, replace_with) in replacements {
        s = s.replace(&replace_str[..], &replace_with[..]);
    }

    // TODO:
    // would be nice to change API to allow return option
    // either it did replace, or it did not
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    struct MyContext {}
    impl Context for MyContext {
        fn get_value_from_key(&self, key: &str, syntax_char: char) -> Option<String> {
            if key == "custom_key" {
                Some("custom_context".into())
            } else {
                None
            }
        }
    }

    #[test]
    fn replace_from_callback_works() {
        let mut failed_to_replace_something = false;
        let failuremode = FailureModeEx::FM_callback(|key| {
            if key == "abc" {
                "xyz".into()
            } else {
                failed_to_replace_something = true;
                "".into()
            }
        });
        let text = "${{ abc }} ${{ hello }}";
        let context: Vec<String> = vec![];
        let replaced = replace_all_from_ex(text, &context, failuremode, None);
        assert!(failed_to_replace_something);
        assert!(replaced.contains("xyz"));
    }

    #[test]
    fn returns_as_is_if_nothing_to_replace() {
        let context: Vec<String> = vec![];
        let text = r#"
        dsadsa
        dsadsadsa dsadsadsa  {{ something? }}
         dsads dsadsadsa ${{notreplacedbecausenospaces}}
        "#;
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, None);
        assert_eq!(text.to_string(), replaced);
    }

    #[test]
    fn replaces_from_simple_vec_contexts() {
        let my_path = "hello";
        let context = vec![my_path];
        let text = "this is my ${{ 0 }} world";
        let expected = "this is my hello world";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, None);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn replaces_from_custom_contexts() {
        let context = MyContext {};
        let text = "this is my ${{ custom_key }}";
        let expected = "this is my custom_context";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, None);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    #[should_panic(expected = "Failed to get context value from key: nonexistant")]
    fn failure_mode_panic_if_failed_to_find_value_for_key() {
        let context = MyContext {};
        let text = "this is my ${{ nonexistant }}";
        let expected = "this is my custom_context";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, None);
    }

    #[test]
    fn should_leave_failed_key_if_mode_is_ignore() {
        let context = MyContext {};
        let text = "this is my ${{ nonexistant }}";
        let expected = "this is my ${{ nonexistant }}";
        let replaced = replace_all_from(text, &context, FailureMode::FM_ignore, None);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn can_use_default_value_if_mode_is_default() {
        let context = MyContext {};
        let text = "this is my ${{ nonexistant }}";
        let expected = "this is my global_default";
        let replaced = replace_all_from(text, &context, FailureMode::FM_default("global_default".into()), None);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn can_use_default_syntax_from_text() {
        let context = MyContext {};
        let text = "this is my ${{ nonexistant | syntax_default }}";
        let expected = "this is my syntax_default";
        // even though we give a global default, we provide a local default
        // so it should use the local default
        let replaced = replace_all_from(text, &context, FailureMode::FM_default("global_default".into()), None);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn can_use_default_dynamically() {
        let context = MyContext {};
        // this syntax should allow parsing the default as another key
        // which would allow dynamic defaults
        let text = "this is my ${{ nonexistant || custom_key }}";
        let expected = "this is my custom_context";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, None);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn default_dynamic_doesnt_override_original_key() {
        let context = vec!["abc", "xyz"];
        // a dynamic default should not be used if the original
        // key works
        let text = "this is my ${{ 0 || 1 }}";
        let expected = "this is my abc";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, None);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn uses_dollar_sign_as_default_syntax_char() {
        let context = vec!["abc", "xyz"];
        let text = "this is my ${{ 0 }}";
        let expected = "this is my abc";
        // because we pass None for syntax chars, it should default to only use
        // dollar sign
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, None);
        assert_eq!(expected.to_string(), replaced);
        let text = "this is my Q{{ 0 }}";
        let expected = "this is my Q{{ 0 }}";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, None);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn calls_context_with_syntax_character() {
        struct MyContext2 {}
        impl Context for MyContext2 {
            fn get_value_from_key(&self, key: &str, syntax_char: char) -> Option<String> {
                if syntax_char == '@' {
                    Some("char_was_@".into())
                } else if syntax_char == '!' {
                    Some("char_was_!".into())
                } else {
                    None
                }
            }
        }

        let valid_chars = Some("@!a");
        let context = MyContext2 {};
        let text = "this is my @{{ custom_key }}";
        let expected = "this is my char_was_@";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, valid_chars);
        assert_eq!(expected.to_string(), replaced);
        let context = MyContext2 {};
        let text = "this is my !{{ custom_key }}";
        let expected = "this is my char_was_!";
        let replaced = replace_all_from(text, &context, FailureMode::FM_panic, valid_chars);
        assert_eq!(expected.to_string(), replaced);
        let context = MyContext2 {};
        let text = "this is my a{{ custom_key }}";
        let expected = "this is my a{{ custom_key }}";
        let replaced = replace_all_from(text, &context, FailureMode::FM_ignore, valid_chars);
        assert_eq!(expected.to_string(), replaced);
    }

    #[test]
    fn whitespace_doesnt_count_as_syntax_char() {
        let context = MyContext {};
        let text = "this is my {{ custom_key }}";
        let expected = "this is my {{ custom_key }}";
        // should be as is because there was no char in front of {{ to match with
        let replaced = replace_all_from(text, &context, FailureMode::FM_ignore, None);
        assert_eq!(expected.to_string(), replaced);
    }

    // TODO:
    // #[test]
    // fn context_works_for_any_vec_slice_or_array() {
    //     let context = ["abc", "xyz"];
    //     let context_vec = vec!["abc", "xyz"];
    //     let context_string: [String; 2] = ["abc".into(), "xyz".into()];
    //     let context_vec_string: Vec<String> = vec!["abc".into(), "xyz".into()];
    //     context.something();
    //     context_vec.something();
    //     context_string.something();
    //     context_vec_string.something();

    //     takes_context3(&context);
    //     takes_context3(&context_vec);
    //     takes_context3(&context_string);
    //     takes_context3(&context_vec[..]);
    //     takes_context3(&context_vec_string[..]);
    // }
}
