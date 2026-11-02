//! 자리별 손가락 배정과 치기 어려운 정도.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finger {
    Pinky,
    Ring,
    Middle,
    Index,
}

/// 열 번호로 손가락을 정합니다. 4, 5열은 검지가 안쪽으로 뻗는 자리입니다.
pub fn finger(col: usize) -> (Hand, Finger) {
    use Finger::*;
    use Hand::*;
    match col {
        0 => (Left, Pinky),
        1 => (Left, Ring),
        2 => (Left, Middle),
        3 | 4 => (Left, Index),
        5 | 6 => (Right, Index),
        7 => (Right, Middle),
        8 => (Right, Ring),
        _ => (Right, Pinky),
    }
}

pub fn finger_name(col: usize) -> String {
    let (hand, finger) = finger(col);
    let hand = match hand {
        Hand::Left => "left",
        Hand::Right => "right",
    };
    let finger = match finger {
        Finger::Pinky => "pinky",
        Finger::Ring => "ring",
        Finger::Middle => "middle",
        Finger::Index => "index",
    };
    format!("{hand} {finger}")
}

pub fn row_name(row: usize) -> &'static str {
    match row {
        0 => "top row",
        1 => "home row",
        _ => "bottom row",
    }
}

/// 자리별 비용. 홈열 검지가 가장 쉽고(1.0), 안쪽 칸과 아랫줄 새끼가 가장 어렵습니다.
const COST: [[f64; 10]; 3] = [
    [3.0, 2.4, 2.0, 2.2, 3.2, 3.2, 2.2, 2.0, 2.4, 3.0],
    [1.6, 1.3, 1.1, 1.0, 2.0, 2.0, 1.0, 1.1, 1.3, 1.6],
    [3.4, 2.8, 2.4, 2.0, 3.6, 3.6, 2.0, 2.4, 2.8, 3.4],
];

pub fn cost(row: usize, col: usize) -> f64 {
    COST[row][col]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_symmetric() {
        for row in COST {
            for c in 0..5 {
                assert_eq!(row[c], row[9 - c]);
            }
        }
    }

    #[test]
    fn fingers_by_column() {
        assert_eq!(finger(3), (Hand::Left, Finger::Index));
        assert_eq!(finger(4), (Hand::Left, Finger::Index));
        assert_eq!(finger(9), (Hand::Right, Finger::Pinky));
        assert_eq!(finger_name(7), "right middle");
    }
}
