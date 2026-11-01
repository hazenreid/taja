//! 연습할 한 줄 만들기: 진짜 단어와 가짜 단어를 섞고, 집중 키를 자주 넣습니다.

use rand::Rng;

use super::generator::pick_weighted;
use super::{WordList, mask_of};
use crate::layout::is_letter;

/// 진짜 단어가 이만큼 있으면 거의 진짜 단어만 씁니다.
const ENOUGH_REAL_WORDS: f64 = 60.0;
/// 단어 중 집중 키가 들어간 단어의 비율 목표.
const FOCUS_RATE: f64 = 0.5;

pub fn build_line(
    rng: &mut impl Rng,
    list: &WordList,
    unlocked: &[char],
    focus: Option<char>,
    max_chars: usize,
) -> String {
    let letters: Vec<char> = unlocked.iter().copied().filter(|&c| is_letter(c)).collect();
    let letter_focus = focus.filter(|&c| is_letter(c));

    let real = list.available(mask_of(letters.iter().copied()));
    let real_ratio = if real.is_empty() {
        0.0
    } else {
        (real.len() as f64 / ENOUGH_REAL_WORDS).clamp(0.15, 0.95)
    };
    let pick_real = |rng: &mut _, want_focus: bool| {
        let options: Vec<(&str, f64)> = real
            .iter()
            .filter(|w| !want_focus || letter_focus.is_none_or(|f| w.text.contains(f)))
            .map(|w| (w.text.as_str(), w.weight))
            .collect();
        pick_weighted(rng, &options).map(str::to_string)
    };

    let mut words: Vec<String> = Vec::new();
    let mut len = 0;
    for _ in 0..200 {
        let want_focus = rng.random_bool(FOCUS_RATE);
        let word = if rng.random_bool(real_ratio) {
            pick_real(rng, want_focus)
        } else {
            None
        }
        .unwrap_or_else(|| {
            let f = if want_focus { letter_focus } else { None };
            list.bigram.generate_with(rng, &letters, f, 3, 6)
        });

        if words.last() == Some(&word) {
            continue;
        }
        let extra = if words.is_empty() { 0 } else { 1 };
        if !words.is_empty() && len + extra + word.len() > max_chars {
            break;
        }
        len += extra + word.len();
        words.push(word);
    }
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn only_unlocked_keys_and_fits_width() {
        let list = WordList::english();
        let mut rng = StdRng::seed_from_u64(7);
        for unlocked in ["asdfjkl", "asdfjkle", "asdfjkleir", "fjdkslaeitro"] {
            let keys = chars(unlocked);
            for _ in 0..200 {
                let line = build_line(&mut rng, &list, &keys, keys.last().copied(), 50);
                assert!(line.len() <= 50, "{line}");
                assert!(!line.is_empty());
                for c in line.chars() {
                    assert!(c == ' ' || keys.contains(&c), "'{c}' in {line}");
                }
            }
        }
    }

    #[test]
    fn focus_key_appears_often() {
        let list = WordList::english();
        let mut rng = StdRng::seed_from_u64(8);
        let keys = chars("asdfjkleirut");
        let (mut with, mut total) = (0, 0);
        for _ in 0..100 {
            for w in build_line(&mut rng, &list, &keys, Some('t'), 50).split(' ') {
                total += 1;
                with += w.contains('t') as usize;
            }
        }
        assert!(with as f64 / total as f64 > 0.4, "{with}/{total}");
    }
}
