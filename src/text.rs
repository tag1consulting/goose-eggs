//! Functionality for efficiently generating random text.

use rand::distr::Alphanumeric;
use rand::{rng, Rng};

/// Generate a random "word" of the specified length of random
/// alphanumeric characters.
///
/// # Example
/// ```rust
/// use goose_eggs::text::random_word;
///
/// // Generate a random "word" comprised of 10 alphanumeric characters.
/// let word = random_word(10);
/// assert_eq!(word.len(), 10);
/// ```
pub fn random_word(length: usize) -> String {
    rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

/// Generate a random "sentence" comprised of random alphanumeric "words"
/// each ranging between 3 and 12 characters.
///
/// # Example
/// ```rust
/// use goose_eggs::text::random_words;
///
/// // Generate a random "sentence" comprised of 5 alphanumeric "words".
/// let sentence = random_words(5);
/// assert_eq!(sentence.split(' ').count(), 5);
/// ```
pub fn random_words(number: usize) -> String {
    let mut words: Vec<String> = Vec::new();
    for _ in 0..number {
        let mut rng = rng();
        words.push(random_word(rng.random_range(3..=12)));
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_word_cases() {
        // (case, requested length)
        let cases = [("empty", 0), ("one", 1), ("ten", 10), ("long", 500)];
        for (case, length) in cases {
            let word = random_word(length);
            assert_eq!(
                word.chars().count(),
                length,
                "random_word case {case}: wrong length in {word:?}"
            );
            assert!(
                word.chars().all(|c| c.is_ascii_alphanumeric()),
                "random_word case {}: non alphanumeric character in {:?}",
                case,
                word
            );
        }
        // 32 random alphanumeric characters repeating is a broken generator.
        assert_ne!(
            random_word(32),
            random_word(32),
            "random_word case repeat: two calls returned the same word"
        );
    }

    #[test]
    fn random_words_cases() {
        // (case, requested number of words)
        let cases = [("zero", 0), ("one", 1), ("five", 5), ("fifty", 50)];
        for (case, number) in cases {
            for _ in 0..20 {
                let sentence = random_words(number);
                let words: Vec<&str> = sentence.split(' ').filter(|w| !w.is_empty()).collect();
                assert_eq!(
                    words.len(),
                    number,
                    "random_words case {case}: wrong number of words in {sentence:?}"
                );
                for word in words {
                    assert!(
                        (3..=12).contains(&word.len()),
                        "random_words case {}: word {:?} not 3 to 12 characters",
                        case,
                        word
                    );
                    assert!(
                        word.chars().all(|c| c.is_ascii_alphanumeric()),
                        "random_words case {}: non alphanumeric word {:?}",
                        case,
                        word
                    );
                }
            }
        }
    }
}
