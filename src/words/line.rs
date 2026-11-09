//! 연습할 한 줄 만들기: 진짜 단어와 가짜 단어를 섞고, 모든 단어에 집중 키를 넣습니다.

use std::collections::HashMap;

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::generator::pick_weighted;
use super::{Lang, WordList, display_width, key_bit, korean, mask_of};

/// 틀렸던 단어를 다시 넣을 확률 (단어 자리마다).
const REVIEW_RATE: f64 = 0.35;
/// 집중 키가 든 진짜 단어가 이만큼 있으면 진짜 단어만 씁니다.
const ENOUGH_REAL_WORDS: usize = 15;
/// 한 줄에 같은 단어는 이만큼까지만 (진짜 단어가 적을 때 같은 단어만 반복되지 않게).
const MAX_REPEAT: usize = 2;

/// 한 줄의 단어 하나.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// 화면에 보일 글자.
    pub text: String,
    /// 누를 키.
    pub keys: String,
    /// 단어 목록에 있는 진짜 단어인지 (가짜 단어면 false).
    #[serde(default)]
    pub real: bool,
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
                real: false,
            })
        }
        Lang::Ko => korean::generate(rng, keys, focus).map(|(text, keys)| Item {
            text,
            keys,
            real: false,
        }),
    }
}

/// `max_width`칸 안에 들어가는 한 줄 (한글은 2칸).
/// `review`는 전에 틀린 단어들. 열린 키로만 되어 있고 집중 키가 든 것만 섞어 넣습니다.
pub fn build_line(
    rng: &mut impl Rng,
    list: &WordList,
    unlocked: &[char],
    focus: Option<char>,
    review: &[Item],
    max_width: usize,
) -> Vec<Item> {
    let keys: Vec<char> = unlocked
        .iter()
        .copied()
        .filter(|&c| key_bit(c).is_some())
        .collect();
    let focus = focus.filter(|f| keys.contains(f));

    // 진짜 단어는 집중 키가 들어간 것만.
    let mask = mask_of(keys.iter().copied());
    let mut real: Vec<_> = list
        .available(mask)
        .into_iter()
        .filter(|w| focus.is_none_or(|f| w.uses(f)))
        .collect();
    // 내장 단어가 모자라면 보조 사전(맥의 /usr/share/dict/words 등)에서 채웁니다.
    if real.len() < ENOUGH_REAL_WORDS {
        real.extend(
            list.available_extra(mask)
                .into_iter()
                .filter(|w| focus.is_none_or(|f| w.uses(f))),
        );
    }
    // 진짜 단어를 먼저 쓰고, 다 쓰면(같은 단어 MAX_REPEAT번) 가짜 단어로 채웁니다.
    // 집중 키가 든 진짜 단어가 ENOUGH_REAL_WORDS개 이상이면 사실상 진짜 단어만 나옵니다.
    let enough = real.len() >= ENOUGH_REAL_WORDS;
    let mut used: HashMap<String, usize> = HashMap::new();
    let pick_real = |rng: &mut _, used: &HashMap<String, usize>| {
        let options: Vec<(usize, f64)> = real
            .iter()
            .enumerate()
            .filter(|(_, w)| used.get(&w.text).copied().unwrap_or(0) < MAX_REPEAT)
            .map(|(i, w)| (i, w.weight))
            .collect();
        pick_weighted(rng, &options).map(|i| Item {
            text: real[i].text.clone(),
            keys: real[i].keys.clone(),
            real: true,
        })
    };

    let mut review: Vec<&Item> = review
        .iter()
        .filter(|r| r.keys.chars().all(|c| keys.contains(&c)))
        .filter(|r| focus.is_none_or(|f| r.keys.contains(f)))
        .collect();

    let mut items: Vec<Item> = Vec::new();
    let mut width = 0;
    for _ in 0..200 {
        let again = if !review.is_empty() && rng.random_bool(REVIEW_RATE) {
            let i = rng.random_range(0..review.len());
            Some(review.swap_remove(i).clone())
        } else {
            None
        };
        let Some(item) = again
            .or_else(|| pick_real(rng, &used))
            .or_else(|| pseudo(rng, list, &keys, focus))
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
        *used.entry(item.text.clone()).or_insert(0) += 1;
        items.push(item);
    }
    // 진짜 단어가 적어서 앞쪽에 몰렸으면 가짜 단어와 섞습니다 (같은 단어가 붙지 않게).
    if !enough {
        for _ in 0..10 {
            items.shuffle(rng);
            if items.windows(2).all(|p| p[0] != p[1]) {
                break;
            }
        }
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
                    let items = build_line(&mut rng, &list, &keys, keys.last().copied(), &[], 50);
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
    fn every_word_has_focus_key() {
        let mut rng = StdRng::seed_from_u64(8);
        for (list, unlocked, focuses) in [
            (WordList::english(), "fjdkslaeitr", "tafe"),
            (WordList::english(), "fjdksla", "fja"),
            (WordList::korean(), "fjdkslgheitrn", "trnk"),
            (WordList::korean(), "fjdkT", "Tfk"),
        ] {
            let keys = chars(unlocked);
            for f in focuses.chars() {
                for _ in 0..50 {
                    for item in build_line(&mut rng, &list, &keys, Some(f), &[], 50) {
                        assert!(item.keys.contains(f), "'{f}' not in {item:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn review_words_only_with_focus_key() {
        let list = WordList::english();
        let mut rng = StdRng::seed_from_u64(9);
        let keys = chars("fjdkslaeit");
        let word = |w: &str| Item {
            text: w.into(),
            keys: w.into(),
            real: true,
        };
        // 'tide'에는 t가 있고 'side'에는 없음, 'tick'은 잠긴 키(c)가 있음.
        let review = [word("tide"), word("side"), word("tick")];
        let (mut tide, mut side, mut tick) = (0, 0, 0);
        for _ in 0..200 {
            for item in build_line(&mut rng, &list, &keys, Some('t'), &review, 50) {
                tide += (item.text == "tide") as usize;
                side += (item.text == "side") as usize;
                tick += (item.text == "tick") as usize;
            }
        }
        assert!(tide > 50, "tide {tide}");
        assert_eq!((side, tick), (0, 0));
    }

    #[test]
    fn only_real_words_when_enough() {
        let list = WordList::english();
        let mut rng = StdRng::seed_from_u64(10);
        let keys = chars("fjdkslaeit");
        for f in ['t', 'e', 'a'] {
            for _ in 0..100 {
                for item in build_line(&mut rng, &list, &keys, Some(f), &[], 50) {
                    assert!(item.real, "가짜 단어: {item:?} (focus {f})");
                }
            }
        }
    }

    #[test]
    fn few_real_words_are_used_up_to_twice_then_pseudo() {
        let list = WordList::english();
        let mut rng = StdRng::seed_from_u64(11);
        let keys = chars("fjdksla");
        let real_with_j = list
            .available(mask_of(keys.iter().copied()))
            .into_iter()
            .filter(|w| w.uses('j'))
            .count();
        assert!(real_with_j < ENOUGH_REAL_WORDS, "{real_with_j}");
        for _ in 0..100 {
            let items = build_line(&mut rng, &list, &keys, Some('j'), &[], 50);
            let mut count: HashMap<&str, usize> = HashMap::new();
            for item in items.iter().filter(|i| i.real) {
                *count.entry(item.text.as_str()).or_insert(0) += 1;
            }
            assert!(count.values().all(|&n| n <= MAX_REPEAT), "{count:?}");
            // 진짜 단어를 다 써야 가짜 단어가 나옴.
            if items.iter().any(|i| !i.real) && real_with_j > 0 {
                assert_eq!(count.len(), real_with_j, "{items:?}");
            }
        }
    }

    #[test]
    fn extra_dictionary_fills_in_when_built_in_words_are_few() {
        let mut list = WordList::english();
        list.add_extra("jak\ndaj\nJaffa\nfjord\n");
        let mut rng = StdRng::seed_from_u64(12);
        let keys = chars("fjdksla");
        let mut seen = std::collections::HashSet::new();
        for _ in 0..50 {
            for item in build_line(&mut rng, &list, &keys, Some('j'), &[], 50) {
                if item.real {
                    seen.insert(item.text);
                }
            }
        }
        // fjord는 잠긴 키(o, r)가 있어서 안 나옴.
        let expected: std::collections::HashSet<String> =
            ["jak", "daj", "jaffa"].iter().map(|s| s.to_string()).collect();
        assert_eq!(seen, expected);
    }
}
