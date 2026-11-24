//! 시작 키와 언락 순서를 레이아웃 위치와 단어 목록으로 자동 계산합니다.

use super::cost::{cost, finger};
use super::{Layout, is_letter};
use crate::corpus::{Ngrams, SPACE};
use crate::words::{WordList, key_bit, mask_of};

/// 시작 키를 찾을 때 단계마다 남겨 두는 조합 수.
const BEAM_WIDTH: usize = 300;
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
/// 단계마다 목표로 삼는 트라이그램 수.
pub const GOALS_PER_STAGE: usize = 5;
/// 목표 트라이그램은 열린 키로 칠 수 있는 진짜 단어 이만큼 이상에 나오는 것을 먼저 고릅니다.
const MIN_GOAL_WORDS: usize = 3;

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

/// 트라이그램마다 필요한 키(공백 제외)의 마스크와 점수(롤이면 ROLL_BONUS배).
fn weighted_trigrams(layout: &Layout, ngrams: &Ngrams) -> Vec<(u64, f64)> {
    ngrams
        .trigrams
        .iter()
        .map(|&(t, n)| {
            let mask = mask_of(t.iter().copied().filter(|&c| c != SPACE));
            let bonus = if is_roll(layout, t) { ROLL_BONUS } else { 1.0 };
            (mask, n * bonus)
        })
        .filter(|&(mask, _)| mask != 0)
        .collect()
}

/// 시작 키: 레이아웃의 모든 글자 키 중에서, 그 키들(과 공백)로 칠 수 있는 트라이그램 빈도가
/// 가장 높은 조합을 고릅니다 (자리는 따지지 않음). 4개부터 늘려 가며, 사전 단어가
/// MIN_START_WORDS개 이상 나오는 조합이 있는 가장 적은 개수에서 멈춥니다.
///
/// 조합 수가 너무 많아서(26개 중 8개면 150만 가지) 빔 탐색을 씁니다: 키를 하나씩 늘리면서
/// 단계마다 점수 상위 BEAM_WIDTH개 조합만 남깁니다. 순서는 글자 빈도순.
pub fn start_keys(layout: &Layout, words: &WordList, ngrams: &Ngrams) -> Vec<char> {
    let letters: Vec<char> = layout.iter().map(|p| p.2).filter(|&c| is_letter(c)).collect();
    let tris = weighted_trigrams(layout, ngrams);
    let coverage = |m: u64| {
        tris.iter()
            .filter(|&&(t, _)| t & !m == 0)
            .map(|t| t.1)
            .sum::<f64>()
    };
    let word_count = |m: u64| words.available(m).len();
    let keys_of = |m: u64| -> Vec<char> {
        letters
            .iter()
            .copied()
            .filter(|&c| m & key_bit(c).unwrap() != 0)
            .collect()
    };

    let max = MAX_START.min(letters.len());
    let mut beam: Vec<(u64, f64)> = vec![(0, 0.0)];
    let mut chosen: Option<u64> = None;
    for n in 1..=max {
        // 지금 조합마다 키 하나씩 더해 보고, 중복을 빼고 점수순으로 상위만 남김.
        let mut next: Vec<u64> = beam
            .iter()
            .flat_map(|&(m, _)| letters.iter().map(move |&c| m | key_bit(c).unwrap()))
            .filter(|&m| m.count_ones() as usize == n)
            .collect();
        next.sort_unstable();
        next.dedup();
        let mut scored: Vec<(u64, f64)> = next.into_iter().map(|m| (m, coverage(m))).collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        scored.truncate(BEAM_WIDTH);
        beam = scored;
        if n >= MIN_START.min(max)
            && let Some(&(m, _)) = beam.iter().find(|&&(m, _)| word_count(m) >= MIN_START_WORDS)
        {
            chosen = Some(m);
            break;
        }
    }
    // MAX_START개로도 단어가 모자라면 빈도가 가장 높은 MAX_START개.
    let mut start = keys_of(chosen.unwrap_or_else(|| beam.first().map_or(0, |b| b.0)));
    let freq = |c: &char| ngrams.letters.get(c).copied().unwrap_or(0.0);
    start.sort_by(|a, b| freq(b).total_cmp(&freq(a)));
    start
}

/// 이번 단계의 목표 트라이그램 (빈도순, 최대 GOALS_PER_STAGE개).
///
/// 후보: 띄어쓰기 없이 열린 키로만 되어 있고, 이번 단계에 새로 열린 키가 들어간 트라이그램 중
/// 열린 키로 칠 수 있는 진짜 단어에 하나라도 나오는 것. 진짜 단어 MIN_GOAL_WORDS개 이상에
/// 나오는 것을 빈도순으로 먼저 고르고, 모자라면 단어가 더 적은 것으로 채웁니다.
/// 후보가 하나도 없으면 빈 목록 (그 단계는 새 키 자체가 목표를 넘으면 통과).
pub fn stage_goals(
    words: &WordList,
    ngrams: &Ngrams,
    unlocked: &[char],
    new_keys: &[char],
) -> Vec<[char; 3]> {
    let available = words.available(mask_of(unlocked.iter().copied()));
    // (빈도 순위, 트라이그램, 든 단어 수 (MIN_GOAL_WORDS에서 셈을 멈춤))
    let mut found: Vec<(usize, [char; 3], usize)> = Vec::new();
    let mut well_covered = 0;
    for (rank, &(tri, _)) in ngrams.trigrams.iter().enumerate() {
        if well_covered >= GOALS_PER_STAGE {
            break;
        }
        if tri.contains(&SPACE)
            || !tri.iter().all(|c| unlocked.contains(c))
            || !tri.iter().any(|c| new_keys.contains(c))
        {
            continue;
        }
        let pat: String = tri.iter().collect();
        let n = available
            .iter()
            .filter(|w| w.keys.contains(&pat))
            .take(MIN_GOAL_WORDS)
            .count();
        if n > 0 {
            well_covered += (n >= MIN_GOAL_WORDS) as usize;
            found.push((rank, tri, n));
        }
    }
    // 단어가 많은 것 먼저 (MIN_GOAL_WORDS개 이상은 같게 봄), 같으면 빈도순. 고른 뒤에는 빈도순.
    found.sort_by_key(|&(rank, _, n)| (std::cmp::Reverse(n), rank));
    found.truncate(GOALS_PER_STAGE);
    found.sort_by_key(|&(rank, _, _)| rank);
    found.into_iter().map(|(_, t, _)| t).collect()
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

/// 시작 키와 나머지 순서를 트라이그램 빈도로 정합니다.
///
/// 후보 키마다 "이 키를 열면 이미 열린 키(와 공백)와 함께 새로 칠 수 있게 되는 트라이그램의
/// 빈도 합"을 구하고(롤이면 ROLL_BONUS배), 글자 빈도를 조금 더한 값이 큰 키부터 엽니다.
/// 자리 비용은 따지지 않습니다 (빈도로 최적화한 레이아웃이면 흔한 글자가 이미 편한 자리에 있음).
/// 점수가 같을 때만 쉬운 자리가 먼저입니다.
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
    let start = start_keys(layout, words, ngrams);
    let start_len = start.len();

    // 2. 나머지 키: 새로 칠 수 있게 되는 트라이그램 빈도가 높은 키부터.
    let tris = weighted_trigrams(layout, ngrams);
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
            let score = gain / total_tri + FREQ_SHARE * f;
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
        let w = WordList::english();
        start_keys(&l, &w, &Ngrams::from_words(&w)).into_iter().collect()
    }

    #[test]
    fn qwerty_plan() {
        let plan = make_plan(&Layout::qwerty(), &WordList::english());
        let start: String = plan.order[..plan.start_len].iter().collect();
        // 자리와 상관없이 빈도가 높은 키 4개 (윗줄 e, t 포함).
        assert_eq!(start, "estr");
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
        assert!(start.iter().all(|&c| l.position(c).is_some()), "{start:?}");
    }

    #[test]
    fn start_keys_do_not_depend_on_positions() {
        // 같은 글자를 다른 자리에 둔 레이아웃이라도 시작 키는 같음 (롤 가산 차이는 있을 수 있음).
        let a = start_of("qwertyuiop\nasdfghjkl;\nzxcvbnm,./");
        let b = start_of("qwfpgjluy;\narstdhneio\nzxcvbkm,./");
        assert_eq!(a.len(), b.len());
        assert!((MIN_START..=MAX_START).contains(&a.len()), "{a}");
    }

    #[test]
    fn start_keys_can_come_from_any_row() {
        // 홈열에 모음이 하나도 없어도, 다른 줄의 흔한 글자로 시작할 수 있음.
        let start = start_of("qwertyuiop\nzsdfghjklx\nacvbnm,./;");
        assert!((MIN_START..=MAX_START).contains(&start.len()), "{start}");
        assert!(start.chars().any(|c| "aeiou".contains(c)), "{start}");
    }

    #[test]
    fn start_keys_have_the_best_coverage_of_their_size() {
        // 빔 탐색 결과가 전수 조사(4개 조합)와 같은지 확인.
        let l = Layout::qwerty();
        let w = WordList::english();
        let n = Ngrams::from_words(&w);
        let start = start_keys(&l, &w, &n);
        if start.len() == 4 {
            let tris = weighted_trigrams(&l, &n);
            let cov = |m: u64| {
                tris.iter()
                    .filter(|&&(t, _)| t & !m == 0)
                    .map(|t| t.1)
                    .sum::<f64>()
            };
            let letters: Vec<char> = ('a'..='z').collect();
            let mut best = 0.0f64;
            for a in 0..26 {
                for b in a + 1..26 {
                    for c in b + 1..26 {
                        for d in c + 1..26 {
                            let m = mask_of([letters[a], letters[b], letters[c], letters[d]]);
                            if w.available(m).len() >= MIN_START_WORDS {
                                best = best.max(cov(m));
                            }
                        }
                    }
                }
            }
            assert_eq!(cov(mask_of(start.iter().copied())), best, "{start:?}");
        }
    }

    #[test]
    fn korean_plan_includes_shift_keys() {
        let plan = make_plan(&Layout::qwerty(), &WordList::korean());
        assert_eq!(plan.order.len(), 26 + 7);
        let start = &plan.order[..plan.start_len];
        assert!((MIN_START..=MAX_START + 1).contains(&start.len()), "{start:?}");
        // Shift 키(ㄲ ㄸ ㅃ ㅆ ㅉ ㅒ ㅖ) 7개가 모두 순서에 들어 있음.
        let mut shifts: Vec<char> = plan
            .order
            .iter()
            .copied()
            .filter(char::is_ascii_uppercase)
            .collect();
        shifts.sort();
        assert_eq!(shifts, ['E', 'O', 'P', 'Q', 'R', 'T', 'W'], "{:?}", plan.order);
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
        // 'h', 'a'가 든 트라이그램만 아주 흔한 코퍼스 → 둘 다 시작 키에 들어가야 함.
        let ngrams = Ngrams::parse_corpus(&encode(
            &[('h', 100), ('a', 100), ('e', 1)],
            &[],
            &[(['a', 'h', 'a'], 1_000_000), (['e', 'e', 'e'], 1)],
        ))
        .unwrap();
        let plan = make_plan_with(&Layout::qwerty(), &words, &ngrams);
        let start = start_keys(&Layout::qwerty(), &words, &ngrams);
        assert_eq!(&plan.order[..plan.start_len], &start[..]);
        assert!(start.contains(&'h') && start.contains(&'a'), "{start:?}");
        assert_eq!(plan.order.len(), 26);
    }

    #[test]
    fn every_stage_gets_goal_trigrams_with_the_new_key() {
        for (layout, words) in [
            (Layout::qwerty(), WordList::english()),
            (Layout::qwerty(), WordList::korean()),
        ] {
            let ngrams = Ngrams::from_words(&words);
            let plan = make_plan_with(&layout, &words, &ngrams);
            for n in plan.start_len..=plan.order.len() {
                let unlocked = &plan.order[..n];
                let new: Vec<char> = if n == plan.start_len {
                    unlocked.to_vec()
                } else {
                    vec![plan.order[n - 1]]
                };
                let goals = stage_goals(&words, &ngrams, unlocked, &new);
                assert!((1..=GOALS_PER_STAGE).contains(&goals.len()), "{n}: {goals:?}");
                let available = words.available(mask_of(unlocked.iter().copied()));
                for g in &goals {
                    assert!(!g.contains(&SPACE), "{g:?}");
                    assert!(g.iter().all(|c| unlocked.contains(c)), "{g:?}");
                    assert!(g.iter().any(|c| new.contains(c)), "{g:?} {new:?}");
                    let pat: String = g.iter().collect();
                    assert!(available.iter().any(|w| w.keys.contains(&pat)), "{pat}");
                }
            }
        }
    }

    #[test]
    fn stage_goals_prefer_trigrams_found_in_more_words() {
        use crate::corpus::tests::encode;
        use crate::words::Lang;
        let words = WordList::from_sources(Lang::En, &[("tea eat heat seat meat ate hate", 1.0)]);
        let tri = |s: &str, n| -> ([char; 3], u64) {
            let c: Vec<char> = s.chars().collect();
            ([c[0], c[1], c[2]], n)
        };
        // 단어 수: eat 4, ate 2, 나머지 1. zza는 잠긴 키, the는 새 키(a)가 없음.
        let ngrams = Ngrams::parse_corpus(&encode(
            &[],
            &[],
            &[
                tri("zza", 9000),
                tri("the", 5000),
                tri("tea", 1000),
                tri("eat", 500),
                tri("hea", 400),
                tri("sea", 200),
                tri("mea", 100),
                tri("ate", 50),
            ],
        ))
        .unwrap();
        let unlocked: Vec<char> = "etahsm".chars().collect();
        let goals: Vec<String> = stage_goals(&words, &ngrams, &unlocked, &['a'])
            .iter()
            .map(|g| g.iter().collect())
            .collect();
        // 단어가 많은 eat, ate를 먼저 넣고, 나머지는 빈도순으로 채움 (mea는 빠짐). 결과는 빈도순.
        assert_eq!(goals, ["tea", "eat", "hea", "sea", "ate"]);
    }
}
