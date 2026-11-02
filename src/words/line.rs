//! 연습할 한 줄 만들기: 진짜 단어와 가짜 단어를 섞고, 집중 키를 자주 넣습니다.

use rand::Rng;

use super::generator::pick_weighted;
use super::{Lang, WordList, display_width, key_bit, korean, mask_of};

/// 진짜 단어가 이만큼 있으면 거의 진짜 단어만 씁니다.
const ENOUGH_REAL_WORDS: f64 = 60.0;
/// 단어 중 집중 키가 들어간 단어의 비율 목표.
const FOCUS_RATE: f64 = 0.5;

/// 한 줄의 단어 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 화면에 보일 글자.
    pub text: String,
    /// 누를 키.
    pub keys: String,
}

/// 열린 키로 가짜 단어 하나를 만듭니다.
fn pseudo(rng: &mut impl Rng, list: &WordList, keys: &[char], focus: Option<char>) -> Option<Item> {
    match list.lang {
        Lang::En => {
            let letters: Vec<char> = keys.iter().copied().filter(char::is_ascii_lowercase).collect();
            let f = focus.filter(char::is_ascii_lowercase);
            let w = list.bigram.generate_with(rng, &letters, f, 3, 6);
            (!w.is_empty()).then(|| Item {
                text: w.clone(),
                keys: w,
            })
        }
        Lang::Ko => korean::generate(rng, keys, focus).map(|(text, keys)| Item { text, keys }),
    }
}

/// `max_width`칸 안에 들어가는 한 줄 (한글은 2칸).
pub fn build_line(
    rng: &mut impl Rng,
    list: &WordList,
    unlocked: &[char],
    focus: Option<char>,
    max_width: usize,
) -> Vec<Item> {
    let keys: Vec<char> = unlocked
        .iter()
        .copied()
        .filter(|&c| key_bit(c).is_some())
        .collect();
    let focus = focus.filter(|f| keys.contains(f));

    let real = list.available(mask_of(keys.iter().copied()));
    let real_ratio = if real.is_empty() {
        0.0
    } else {
        (real.len() as f64 / ENOUGH_REAL_WORDS).clamp(0.15, 0.95)
    };
    let pick_real = |rng: &mut _, want_focus: bool| {
        let options: Vec<(usize, f64)> = real
            .iter()
            .enumerate()
            .filter(|(_, w)| !want_focus || focus.is_none_or(|f| w.uses(f)))
            .map(|(i, w)| (i, w.weight))
            .collect();
        pick_weighted(rng, &options).map(|i| Item {
            text: real[i].text.clone(),
            keys: real[i].keys.clone(),
        })
    };

    let mut items: Vec<Item> = Vec::new();
    let mut width = 0;
    for _ in 0..200 {
        let want_focus = rng.random_bool(FOCUS_RATE);
        let real_word = if rng.random_bool(real_ratio) {
            pick_real(rng, want_focus)
        } else {
            None
        };
        let Some(item) = real_word
            .or_else(|| pseudo(rng, list, &keys, if want_focus { focus } else { None }))
            .or_else(|| pick_real(rng, false))
        else {
            continue;
        };

        if items.last() == Some(&item) {
            continue;
        }
        let w = display_width(&item.text);
        let extra = if items.is_empty() { 0 } else { 1 };
        if !items.is_empty() && width + extra + w > max_width {
            break;
        }
        width += extra + w;
        items.push(item);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn line_text(items: &[Item]) -> String {
        items
            .iter()
            .map(|i| i.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn only_unlocked_keys_and_fits_width() {
        let mut rng = StdRng::seed_from_u64(7);
        for (list, sets) in [
            (
                WordList::english(),
                ["asdfjkl", "asdfjkle", "asdfjkleir", "fjdkslaeitro"],
            ),
            (
                WordList::korean(),
                ["fjdk", "fjdksl", "fjdkslaT", "fjdkslgheitrn"],
            ),
        ] {
            for unlocked in sets {
                let keys = chars(unlocked);
                for _ in 0..100 {
                    let items = build_line(&mut rng, &list, &keys, keys.last().copied(), 50);
                    let text = line_text(&items);
                    assert!(!items.is_empty());
                    assert!(display_width(&text) <= 50, "{text}");
                    for item in &items {
                        assert!(item.keys.chars().all(|c| keys.contains(&c)), "{item:?}");
                    }
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
            for item in build_line(&mut rng, &list, &keys, Some('t'), 50) {
                total += 1;
                with += item.keys.contains('t') as usize;
            }
        }
        assert!(with as f64 / total as f64 > 0.4, "{with}/{total}");
    }
}
