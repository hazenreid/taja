//! 연습할 한 줄 만들기: 진짜 단어와 가짜 단어를 섞고, 모든 단어에 연습할 키 순서를 넣습니다.

use std::collections::HashMap;

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use super::generator::pick_weighted;
use super::{Lang, Word, WordList, display_width, key_bit, korean, mask_of};

/// 틀렸던 단어를 다시 넣을 확률 (단어 자리마다).
const REVIEW_RATE: f64 = 0.35;
/// 한 줄에 같은 단어는 이만큼까지만 (진짜 단어가 적을 때 같은 단어만 반복되지 않게).
const MAX_REPEAT: usize = 2;
/// 약한 연결 점수 1점당 단어 가중치를 이만큼 키움 (점수 1이면 1 + 2 = 3배).
const WEAK_BOOST: f64 = 2.0;
/// 연습할 키 순서가 여러 개일 때, 가장 약한 것(첫 번째)이 뽑힐 비중 (나머지는 1).
const WEAKEST_SHARE: f64 = 2.0;
/// 트라이그램 가짜 단어의 최대 길이.
const PATTERN_WORD_MAX: usize = 7;

/// 단어에 든 약한 연결 점수의 합. 앞뒤 공백도 붙여서 단어 시작/끝 연결도 셉니다.
fn weak_score(keys: &str, weak: &[([char; 2], f64)]) -> f64 {
    if weak.is_empty() {
        return 0.0;
    }
    let seq: Vec<char> = std::iter::once(' ').chain(keys.chars()).chain([' ']).collect();
    weak.iter()
        .filter(|(pair, _)| seq.windows(2).any(|w| w == pair))
        .map(|(_, score)| score)
        .sum()
}

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

/// 열린 키로 가짜 단어 하나를 만듭니다. `pattern`(키 순서)이 있으면 그것이 꼭 들어갑니다.
fn pseudo(rng: &mut impl Rng, list: &WordList, keys: &[char], pattern: Option<&str>) -> Option<Item> {
    let one_key = pattern
        .filter(|p| p.chars().count() == 1)
        .and_then(|p| p.chars().next());
    let (text, keys) = match (list.lang, pattern) {
        (Lang::En, _) => {
            let letters: Vec<char> = keys.iter().copied().filter(char::is_ascii_lowercase).collect();
            let w = match pattern {
                Some(p) if one_key.is_none() => {
                    let p: Vec<char> = p.chars().collect();
                    list.bigram
                        .generate_with_pattern(rng, &letters, &p, PATTERN_WORD_MAX)
                }
                _ => {
                    let f = one_key.filter(char::is_ascii_lowercase);
                    list.bigram.generate_with(rng, &letters, f, 3, 6)
                }
            };
            (w.clone(), w)
        }
        (Lang::Ko, Some(p)) if one_key.is_none() => korean::generate_with_pattern(rng, keys, p)?,
        (Lang::Ko, _) => korean::generate(rng, keys, one_key)?,
    };
    (!text.is_empty()).then_some(Item {
        text,
        keys,
        real: false,
    })
}

/// `max_width`칸 안에 들어가는 한 줄 (한글은 2칸).
///
/// `patterns`는 연습할 키 순서들 (집중 키 하나면 `["t"]`, 목표 트라이그램이면 `["her", "the", ...]`).
/// 단어마다 먼저 패턴 하나를 고르고(첫 번째 = 가장 약한 것이 더 자주), 그 패턴이 든 진짜 단어를
/// 고릅니다. 그 패턴의 진짜 단어를 다 썼으면(한 줄에 MAX_REPEAT번) 가짜 단어를 만듭니다.
/// 패턴이 없으면 열린 키로 된 아무 단어나.
/// `review`는 전에 틀린 단어들. 열린 키로만 되어 있고 패턴이 든 것만 섞어 넣습니다.
/// `weak`는 약한 연결([앞 키, 키], 점수). 이 연결이 든 진짜 단어를 더 자주 고릅니다.
pub fn build_line(
    rng: &mut impl Rng,
    list: &WordList,
    unlocked: &[char],
    patterns: &[String],
    review: &[Item],
    weak: &[([char; 2], f64)],
    max_width: usize,
) -> Vec<Item> {
    let keys: Vec<char> = unlocked
        .iter()
        .copied()
        .filter(|&c| key_bit(c).is_some())
        .collect();
    // 열린 키로 칠 수 없는 패턴은 뺍니다.
    let patterns: Vec<&str> = patterns
        .iter()
        .map(String::as_str)
        .filter(|p| !p.is_empty() && p.chars().all(|c| keys.contains(&c)))
        .collect();

    // 패턴마다 그 패턴이 든 진짜 단어 (패턴이 없으면 열린 키로 된 모든 단어 한 묶음).
    let available = list.available(mask_of(keys.iter().copied()));
    let real: Vec<Vec<&Word>> = if patterns.is_empty() {
        vec![available]
    } else {
        patterns
            .iter()
            .map(|p| available.iter().copied().filter(|w| w.keys.contains(p)).collect())
            .collect()
    };
    let shares: Vec<(usize, f64)> = (0..real.len())
        .map(|i| {
            (
                i,
                if i == 0 && real.len() > 1 {
                    WEAKEST_SHARE
                } else {
                    1.0
                },
            )
        })
        .collect();
    let pick_real = |rng: &mut _, words: &[&Word], used: &HashMap<String, usize>, limit: usize| {
        let options: Vec<(usize, f64)> = words
            .iter()
            .enumerate()
            .filter(|(_, w)| used.get(&w.text).copied().unwrap_or(0) < limit)
            .map(|(i, w)| (i, w.weight * (1.0 + WEAK_BOOST * weak_score(&w.keys, weak))))
            .collect();
        pick_weighted(rng, &options).map(|i| Item {
            text: words[i].text.clone(),
            keys: words[i].keys.clone(),
            real: true,
        })
    };

    // 가짜 단어도 한 줄에 MAX_REPEAT번을 넘지 않게 몇 번 다시 만들어 봅니다 (안 되면 처음 것).
    let fresh_pseudo = |rng: &mut _, used: &HashMap<String, usize>, pattern: Option<&str>| {
        let mut first = None;
        for _ in 0..8 {
            let item = pseudo(rng, list, &keys, pattern)?;
            if used.get(&item.text).copied().unwrap_or(0) < MAX_REPEAT {
                return Some(item);
            }
            first.get_or_insert(item);
        }
        first
    };

    let mut review: Vec<&Item> = review
        .iter()
        .filter(|r| r.keys.chars().all(|c| keys.contains(&c)))
        .filter(|r| patterns.is_empty() || patterns.iter().any(|p| r.keys.contains(p)))
        .collect();

    let mut items: Vec<Item> = Vec::new();
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut width = 0;
    for _ in 0..200 {
        let item = if !review.is_empty() && rng.random_bool(REVIEW_RATE) {
            let i = rng.random_range(0..review.len());
            Some(review.swap_remove(i).clone())
        } else {
            pick_weighted(rng, &shares).and_then(|p| {
                pick_real(rng, &real[p], &used, MAX_REPEAT)
                    .or_else(|| fresh_pseudo(rng, &used, patterns.get(p).copied()))
                    // 가짜 단어를 못 만들면 같은 진짜 단어를 더 씀.
                    .or_else(|| pick_real(rng, &real[p], &used, usize::MAX))
            })
        };
        let Some(item) = item else {
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
    // 진짜 단어가 모자라 가짜 단어가 뒤쪽에 몰렸으면 섞습니다 (같은 단어가 붙지 않게).
    if items.iter().any(|i| !i.real) {
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
                    let items = build_line(
                        &mut rng,
                        &list,
                        &keys,
                        &[keys.last().unwrap().to_string()],
                        &[],
                        &[],
                        50,
                    );
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
                    for item in build_line(&mut rng, &list, &keys, &[f.to_string()], &[], &[], 50) {
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
            for item in build_line(&mut rng, &list, &keys, &["t".to_string()], &review, &[], 50) {
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
                for item in build_line(&mut rng, &list, &keys, &[f.to_string()], &[], &[], 50) {
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
            .filter(|w| w.keys.contains('j'))
            .count();
        assert!(real_with_j < 10, "{real_with_j}");
        for _ in 0..100 {
            let items = build_line(&mut rng, &list, &keys, &["j".to_string()], &[], &[], 50);
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
    fn weak_pairs_make_their_words_more_common() {
        let list = WordList::english();
        let keys = chars("fjdkslaeitrhon");
        let share_with_th = |weak: &[([char; 2], f64)]| {
            let mut rng = StdRng::seed_from_u64(13);
            let (mut with, mut total) = (0, 0);
            for _ in 0..300 {
                for item in build_line(&mut rng, &list, &keys, &["e".to_string()], &[], weak, 50) {
                    total += 1;
                    with += item.keys.contains("th") as usize;
                }
            }
            with as f64 / total as f64
        };
        let before = share_with_th(&[]);
        let after = share_with_th(&[(['t', 'h'], 2.0)]);
        assert!(after > before * 1.5, "{before} -> {after}");
    }

    #[test]
    fn weak_score_counts_word_edges() {
        let weak = [([' ', 't'], 1.0), (['e', ' '], 2.0), (['t', 'h'], 0.5)];
        assert_eq!(weak_score("the", &weak), 3.5);
        assert_eq!(weak_score("ate", &weak), 2.0);
        assert_eq!(weak_score("as", &weak), 0.0);
    }
}
