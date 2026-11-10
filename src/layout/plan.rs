//! 시작 키와 언락 순서를 레이아웃 위치와 단어 목록으로 자동 계산합니다.

use super::cost::{cost, finger};
use super::{HOME_ROW, Layout, is_letter};
use crate::corpus::{Ngrams, SPACE};
use crate::words::{WordList, key_bit, mask_of};

/// 홈열에서 시작 키를 고르는 순서: 검지 → 중지 → 약지 → 새끼 (안쪽 4, 5열 제외).
const START_COLS: [usize; 8] = [3, 6, 2, 7, 1, 8, 0, 9];
/// 시작 키 개수 범위.
pub const MIN_START: usize = 4;
pub const MAX_START: usize = 8;
/// 시작 키로 칠 수 있는 진짜 단어가 이만큼 있으면 충분하다고 봅니다.
const MIN_START_WORDS: usize = 10;
/// 새 단어가 하나도 안 생기는 단계에서도 순서를 정할 수 있게 글자 빈도를 조금 섞습니다.
const FREQ_SHARE: f64 = 0.1;
/// 롤(같은 손, 다른 손가락, 한 방향)인 트라이그램은 이만큼 더 쳐줍니다.
const ROLL_BONUS: f64 = 1.5;
/// Shift를 같이 눌러야 하는 키는 이만큼 더 어렵다고 봅니다.
const SHIFT_COST: f64 = 1.5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// 전체 키 순서. 앞의 `start_len`개가 시작 키입니다.
    pub order: Vec<char>,
    pub start_len: usize,
}

/// 키의 자리 비용. Shift 키(대문자)는 같은 자리 소문자 비용 + Shift 비용.
pub fn key_cost(layout: &Layout, key: char) -> Option<f64> {
    let (r, c) = layout.position(key.to_ascii_lowercase())?;
    let shift = if key.is_ascii_uppercase() { SHIFT_COST } else { 0.0 };
    Some(cost(r, c) + shift)
}

/// 홈열 키를 검지부터 하나씩 늘려 가며, 진짜 단어가 충분히 나오는 가장 적은 개수(4~8)를 고릅니다.
/// 8개로도 부족하면 8개를 쓰고, 모음이 하나도 없으면 가장 쉬운 자리의 모음을 하나 더합니다.
pub fn start_keys(layout: &Layout, words: &WordList) -> Vec<char> {
    let vowels = words.lang.vowel_keys();
    let home: Vec<char> = START_COLS
        .iter()
        .filter_map(|&c| layout.key_at(HOME_ROW, c))
        .collect();
    let word_count = |keys: &[char]| words.available(mask_of(keys.iter().copied())).len();
    // 홈열에 빈 칸이 있으면 쓸 수 있는 키가 8개보다 적을 수 있습니다.
    let max = MAX_START.min(home.len());
    let n = (MIN_START.min(max)..=max)
        .find(|&n| word_count(&home[..n]) >= MIN_START_WORDS)
        .unwrap_or(max);
    let mut start = home[..n].to_vec();

    if word_count(&start) < MIN_START_WORDS && !start.iter().any(|c| vowels.contains(c)) {
        let vowel = layout
            .iter()
            .filter(|p| vowels.contains(&p.2) && !start.contains(&p.2))
            .min_by(|a, b| cost(a.0, a.1).total_cmp(&cost(b.0, b.1)));
        if let Some(p) = vowel {
            start.push(p.2);
        }
    }
    start
}

/// 단어 목록의 빈도로 트라이그램을 세서 계산합니다 (테스트용 줄임).
#[cfg(test)]
pub fn make_plan(layout: &Layout, words: &WordList) -> Plan {
    make_plan_with(layout, words, &Ngrams::from_words(words))
}

/// 세 글자가 같은 손, 서로 다른 손가락으로 한 방향(안쪽 또는 바깥쪽)으로 이어지는지.
pub fn is_roll(layout: &Layout, tri: [char; 3]) -> bool {
    let mut cols = [0usize; 3];
    for (i, &c) in tri.iter().enumerate() {
        match layout.position(c.to_ascii_lowercase()) {
            Some((_, col)) => cols[i] = col,
            None => return false,
        }
    }
    let [a, b, c] = cols.map(finger);
    let same_hand = a.0 == b.0 && b.0 == c.0;
    let distinct = a.1 != b.1 && b.1 != c.1 && a.1 != c.1;
    let one_way = (cols[0] < cols[1] && cols[1] < cols[2]) || (cols[0] > cols[1] && cols[1] > cols[2]);
    same_hand && distinct && one_way
}

/// 시작 키는 단어 목록으로 정하고, 나머지 순서는 트라이그램 빈도로 정합니다.
///
/// 후보 키마다 "이 키를 열면 이미 열린 키(와 공백)와 함께 새로 칠 수 있게 되는 트라이그램의
/// 빈도 합"을 구하고(롤이면 ROLL_BONUS배), 글자 빈도를 조금 더한 뒤 자리 비용으로 나눕니다.
pub fn make_plan_with(layout: &Layout, words: &WordList, ngrams: &Ngrams) -> Plan {
    let cost_of = |ch: char| key_cost(layout, ch).expect("key must be in the layout");
    // 레이아웃의 글자 키 + 이 언어에서 쓰는 Shift 키.
    let mut keys: Vec<char> = layout.iter().map(|p| p.2).filter(|&c| is_letter(c)).collect();
    let shifted: Vec<char> = words
        .lang
        .shift_keys()
        .iter()
        .copied()
        .filter(|k| keys.contains(&k.to_ascii_lowercase()))
        .collect();
    keys.extend(shifted);
    keys.sort_by(|a, b| cost_of(*a).total_cmp(&cost_of(*b)));

    // 1. 시작 키.
    let start = start_keys(layout, words);
    let start_len = start.len();

    // 2. 트라이그램마다 필요한 키(공백 제외)와 점수(롤이면 가산).
    let tris: Vec<(u64, f64)> = ngrams
        .trigrams
        .iter()
        .map(|&(t, n)| {
            let mask = mask_of(t.iter().copied().filter(|&c| c != SPACE));
            let bonus = if is_roll(layout, t) { ROLL_BONUS } else { 1.0 };
            (mask, n * bonus)
        })
        .filter(|&(mask, _)| mask != 0)
        .collect();
    let total_tri: f64 = tris.iter().map(|t| t.1).sum::<f64>().max(1e-9);
    let total_letters: f64 = ngrams
        .letters
        .iter()
        .filter(|(c, _)| **c != SPACE)
        .map(|(_, n)| n)
        .sum::<f64>()
        .max(1e-9);

    let mut order = start;
    let mut unlocked = mask_of(order.iter().copied());
    let mut remaining: Vec<char> = keys.iter().copied().filter(|c| !order.contains(c)).collect();
    while !remaining.is_empty() {
        let mut best: Option<(usize, f64)> = None;
        for (i, &ch) in remaining.iter().enumerate() {
            let next = unlocked | key_bit(ch).unwrap();
            let gain: f64 = tris
                .iter()
                .filter(|&&(m, _)| m & !next == 0 && m & !unlocked != 0)
                .map(|t| t.1)
                .sum();
            let f = ngrams.letters.get(&ch).copied().unwrap_or(0.0) / total_letters;
            let score = (gain / total_tri + FREQ_SHARE * f) / cost_of(ch);
            // remaining은 비용순이라, 동점이면 쉬운 자리가 먼저 뽑힙니다.
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((i, score));
            }
        }
        let (i, _) = best.unwrap();
        let ch = remaining.remove(i);
        unlocked |= key_bit(ch).unwrap();
        order.push(ch);
    }

    debug_assert_eq!(order.len(), keys.len());
    Plan { order, start_len }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start_of(rows: &str) -> String {
        let l = Layout::parse("t", rows).unwrap();
        start_keys(&l, &WordList::english()).into_iter().collect()
    }

    #[test]
    fn qwerty_plan() {
        let plan = make_plan(&Layout::qwerty(), &WordList::english());
        let start: String = plan.order[..plan.start_len].iter().collect();
        // f j d k s l 에는 모음이 없어서 a가 들어가는 7개에서 멈춥니다.
        assert_eq!(start, "fjdksla");
        assert_eq!(plan.order[plan.start_len], 'e', "{:?}", plan.order);
        // ; , . / 는 빈 칸이라 키는 26개.
        assert_eq!(plan.order.len(), 26);
        // 모든 키가 한 번씩.
        let mut sorted = plan.order.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 26);
    }

    #[test]
    fn layout_with_empty_slots() {
        let l = Layout::parse("mine", "pmal.zbfjy\ninerqxot.s\nkvgd..hcuw").unwrap();
        let plan = make_plan(&l, &WordList::english());
        assert_eq!(plan.order.len(), 26);
        let start = &plan.order[..plan.start_len];
        assert!((MIN_START..=MAX_START).contains(&start.len()), "{start:?}");
        assert!(
            start.iter().all(|&c| l.position(c).unwrap().0 == HOME_ROW),
            "{start:?}"
        );
    }

    #[test]
    fn uses_fewest_keys_that_give_enough_words() {
        // 검지·중지 자리에 t n s e 가 있으면 4개로 충분 (set, ten, nest, seen ...).
        assert_eq!(start_of("qwfpgjluy;\narstdhneio\nzxcvbkm,./"), "tnse");
    }

    #[test]
    fn adds_vowel_when_home_row_has_none() {
        let start = start_of("qwertyuiop\nzsdfghjklx\nacvbnm,./;");
        assert_eq!(start.len(), MAX_START + 1, "{start}");
        assert!(start.chars().any(|c| "aeiou".contains(c)), "{start}");
    }

    #[test]
    fn korean_plan_includes_shift_keys() {
        let plan = make_plan(&Layout::qwerty(), &WordList::korean());
        assert_eq!(plan.order.len(), 26 + 7);
        let start = &plan.order[..plan.start_len];
        assert!((MIN_START..=MAX_START + 1).contains(&start.len()), "{start:?}");
        // ㅆ(T)은 아주 흔해서 Shift 키 중 가장 먼저 열립니다.
        let first_shift = plan.order.iter().find(|c| c.is_ascii_uppercase());
        assert_eq!(first_shift, Some(&'T'), "{:?}", plan.order);
    }

    #[test]
    fn detects_rolls() {
        let q = Layout::qwerty();
        assert!(is_roll(&q, ['a', 's', 'd'])); // 왼손 새끼→약지→중지
        assert!(is_roll(&q, ['l', 'k', 'j'])); // 오른손 바깥→안쪽
        assert!(is_roll(&q, ['w', 'e', 'f'])); // 줄이 달라도 한 방향이면 롤
        assert!(!is_roll(&q, ['a', 'd', 's'])); // 방향이 바뀜
        assert!(!is_roll(&q, ['f', 'g', 'h'])); // 손이 바뀜
        assert!(!is_roll(&q, ['f', 'r', 'd'])); // f, r은 같은 손가락
        assert!(!is_roll(&q, [' ', 'a', 's'])); // 공백 포함
    }

    #[test]
    fn corpus_trigrams_decide_the_order() {
        use crate::corpus::{Ngrams, tests::encode};
        let words = WordList::english();
        let start: Vec<char> = start_keys(&Layout::qwerty(), &words);
        // 시작 키 바로 다음: 'h'가 들어간 트라이그램만 아주 흔한 코퍼스 → h가 먼저 열려야 함.
        let ngrams = Ngrams::parse_corpus(&encode(
            &[('h', 100), ('e', 1)],
            &[],
            &[(['a', 'h', 'a'], 1_000_000), (['a', 'e', 'a'], 1)],
        ))
        .unwrap();
        let plan = make_plan_with(&Layout::qwerty(), &words, &ngrams);
        assert_eq!(&plan.order[..plan.start_len], &start[..]);
        assert_eq!(plan.order[plan.start_len], 'h', "{:?}", plan.order);
        assert_eq!(plan.order.len(), 26);
    }
}
