use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct SubscriberName(String);

impl SubscriberName {
    pub fn parse(s: String) -> Result<SubscriberName, String> {
        let empty_or_whitespace = s.trim().is_empty();

        let too_long = s.graphemes(true).count() > 256;

        let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];

        let contain_forbidden_chars = s.chars().any(|g| forbidden_characters.contains(&g));

        if empty_or_whitespace || too_long || contain_forbidden_chars {
            Err(format!("{} is not a valid subscriber name.", s))
        } else {
            Ok(Self(s))
        }
    }

    pub fn inner(self) -> String {
        self.0
    }

    pub fn inner_mut(&mut self) -> &mut str {
        &mut self.0
    }

    pub fn inner_ref(&self) -> &str {
        &self.0
    }
}


impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}




#[cfg(test)]
mod tests {
    use claim::{assert_ok, assert_err};
    use super::SubscriberName;
    

    #[test]
    fn a_256_grapheme_log_name_valid() {
        let name = "a̐".repeat(256);
        assert_ok!(SubscriberName::parse(name));   
    }

    #[test]
    fn a_name_longer_than_256_grapheme_is_rejected() {
        let name = "a".repeat(257);
        assert_err!(SubscriberName::parse(name));
    }
}