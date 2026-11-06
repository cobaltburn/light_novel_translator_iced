use phf::phf_map;
use std::collections::HashMap;

const FREQUENCY_TOLERANCE: f64 = 15.0;

static LETTER_FREQUENCY: phf::Map<char, f64> = phf_map! {
    'z'=> 0.074,
    'q'=> 0.12,
    'x'=> 0.15,
    'j'=> 0.16,
    'k'=> 0.77,
    'v'=> 0.98,
    'b'=> 1.5,
    'p'=> 1.9,
    'g'=> 2.0,
    'y'=> 2.0,
    'f'=> 2.2,
    'm'=> 2.4,
    'w'=> 2.4,
    'c'=> 2.8,
    'u'=> 2.8,
    'l'=> 4.0,
    'd'=> 4.3,
    'r'=> 6.0,
    'h'=> 6.1,
    's'=> 6.3,
    'n'=> 6.7,
    'i'=> 7.0,
    'o'=> 7.5,
    'a'=> 8.2,
    't'=> 9.1,
    'e'=> 12.7,
};

pub(super) fn check_char_frequency(text: &str) -> bool {
    let char_count: HashMap<char, usize> = text
        .to_lowercase()
        .chars()
        .filter(|e| e.is_alphabetic())
        .fold(HashMap::with_capacity(32), |mut acc, e| {
            *acc.entry(e).or_insert(0) += 1;
            acc
        });

    let total: usize = char_count.values().sum();

    char_count
        .into_iter()
        .map(|(ch, count)| (ch, (count as f64 / total as f64) * 100.0))
        .any(|(ch, percent)| {
            LETTER_FREQUENCY
                .get(&ch)
                .is_some_and(|frequency| percent > frequency + FREQUENCY_TOLERANCE)
        })
}
