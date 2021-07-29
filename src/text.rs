//! Functionality for efficiently generating random text.

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

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
    thread_rng()
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
/// assert!(sentence.len() >= 3*5 && sentence.len() <= 12*5);
/// ```
pub fn random_words(number: usize) -> String {
    let mut words: Vec<String> = Vec::new();
    for _ in 1..number {
        let mut rng = thread_rng();
        words.push(random_word(rng.gen_range(3..12)));
    }
    words.join(" ")
}
