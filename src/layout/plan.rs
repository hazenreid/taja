//! 시작 키와 언락 순서를 레이아웃 위치와 단어 목록으로 자동 계산합니다.

use super::cost::cost;
use super::{COLS, HOME_ROW, Layout, is_letter};
use crate::words::{WordList, letter_bit};

const VOWELS: [char; 5] = ['a', 'e', 'i', 'o', 'u'];
/// 홈열에서 시작 키로 쓰는 칸 (안쪽 4, 5열 제외).
const START_COLS: [usize; 8] = [0, 1, 2, 3, 6, 7, 8, 9];
/// 새 단어가 하나도 안 생기는 단계에서도 순서를 정할 수 있게 글자 빈도를 조금 섞습니다.
const FREQ_SHARE: f64 = 0.1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// 전체 키 순서. 앞의 `start_len`개가 시작 키입니다.
    pub order: Vec<char>,
    pub start_len: usize,
}

pub fn make_plan(layout: &Layout, words: &WordList) -> Plan {
    let cost_of = |ch: char| {
        let (r, c) = layout.position(ch).expect("레이아웃에 있는 글자");
        cost(r, c)
    };
    let mut letters: Vec<char> = layout.iter().map(|p| p.2).filter(|&c| is_letter(c)).collect();
    letters.sort_by(|a, b| cost_of(*a).total_cmp(&cost_of(*b)));

    // 1. 시작 키: 홈열 8칸의 글자.
    let mut start: Vec<char> = START_COLS
        .iter()
        .map(|&c| layout.keys[HOME_ROW][c])
        .filter(|&c| is_letter(c))
        .collect();
    if start.len() < 4 {
        // 홈열이 거의 구두점인 이상한 레이아웃: 가장 쉬운 자리 글자로 채움.
        for &c in &letters {
            if start.len() >= 4 {
                break;
            }
            if !start.contains(&c) {
                start.push(c);
            }
        }
    }
    // 모음이 없으면 진짜 단어가 0개이므로 가장 쉬운 자리의 모음을 추가.
    if !start.iter().any(|c| VOWELS.contains(c))
        && let Some(&v) = letters.iter().find(|c| VOWELS.contains(c))
    {
        start.push(v);
    }
    let start_len = start.len();

    // 2. 나머지 글자: 새로 칠 수 있게 되는 단어가 가장 많은(자리 비용 대비) 글자부터.
    let total_weight: f64 = words.words.iter().map(|w| w.weight).sum::<f64>().max(1e-9);
    let mut freq = [0.0; 26];
    for w in &words.words {
        for ch in w.text.chars() {
            freq[(ch as u8 - b'a') as usize] += w.weight;
        }
    }
    let total_freq: f64 = freq.iter().sum::<f64>().max(1e-9);

    let mut order = start;
    let mut unlocked: u32 = order.iter().filter_map(|&c| letter_bit(c)).fold(0, |m, b| m | b);
    let mut remaining: Vec<char> = letters.iter().copied().filter(|c| !order.contains(c)).collect();
    while !remaining.is_empty() {
        let mut best: Option<(usize, f64)> = None;
        for (i, &ch) in remaining.iter().enumerate() {
            let next = unlocked | letter_bit(ch).unwrap();
            let gain: f64 = words
                .words
                .iter()
                .filter(|w| w.mask & !next == 0 && w.mask & !unlocked != 0)
                .map(|w| w.weight)
                .sum();
            let f = freq[(ch as u8 - b'a') as usize] / total_freq;
            let score = (gain / total_weight + FREQ_SHARE * f) / cost_of(ch);
            // remaining은 비용순이라, 동점이면 쉬운 자리가 먼저 뽑힙니다.
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((i, score));
            }
        }
        let (i, _) = best.unwrap();
        let ch = remaining.remove(i);
        unlocked |= letter_bit(ch).unwrap();
        order.push(ch);
    }

    // 3. 글자가 아닌 키(; , . / 등)는 맨 뒤에, 쉬운 자리부터.
    let mut others: Vec<(usize, usize, char)> = layout.iter().filter(|p| !is_letter(p.2)).collect();
    others.sort_by(|a, b| cost(a.0, a.1).total_cmp(&cost(b.0, b.1)).then(a.1.cmp(&b.1)));
    order.extend(others.into_iter().map(|p| p.2));

    debug_assert_eq!(order.len(), layout.iter().count());
    debug_assert!(order.len() <= 3 * COLS);
    Plan { order, start_len }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qwerty_plan() {
        let plan = make_plan(&Layout::qwerty(), &WordList::english());
        let start: String = plan.order[..plan.start_len].iter().collect();
        assert_eq!(start, "asdfjkl");
        assert_eq!(plan.order[plan.start_len], 'e', "{:?}", plan.order);
        assert_eq!(plan.order.len(), 30);
        // 구두점은 맨 뒤 4개.
        let tail: String = plan.order[26..].iter().collect();
        assert!(tail.chars().all(|c| ";,./".contains(c)), "{tail}");
        // 모든 글자가 한 번씩.
        let mut sorted = plan.order.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 30);
    }

    #[test]
    fn adds_vowel_when_home_row_has_none() {
        // 홈열에 모음이 없는 레이아웃.
        let l = Layout::parse("t", "qwertyuiop\nzsdfghjklx\nacvbnm,./;").unwrap();
        let plan = make_plan(&l, &WordList::english());
        let start = &plan.order[..plan.start_len];
        assert!(start.iter().any(|c| VOWELS.contains(c)), "{start:?}");
    }
}
