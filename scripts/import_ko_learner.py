#!/usr/bin/env python3
"""국립국어원 「한국어 학습용 어휘 목록」(2003)에서 한글 연습 단어를 뽑아 assets/words/ko/learner/ 에 씁니다.

사용법:
    # 국립국어원 자료실(https://www.korean.go.kr/front_eng/down/down_02V.do?etc_seq=70)에서
    # 텍스트 파일 「한국어 학습용 어휘 목록.txt」를 받은 뒤
    python3 scripts/import_ko_learner.py "한국어 학습용 어휘 목록.txt"

원본 형식: CP949 인코딩, 탭으로 구분된 표 (순위, 단어, 품사, 풀이, 등급). 등급은 A(가장 기초), B, C.

- 동음이의어 번호(`가다01` → `가다`)를 떼고, 완성된 한글로만 된 단어만 씁니다.
- 등급별 파일(a.txt, b.txt, c.txt)에 빈도 순위순으로 씁니다. 여러 등급에 나오면 더 쉬운 등급에만 넣습니다.
"""

import re
import sys
from pathlib import Path

GRADES = ["A", "B", "C"]
WORD = re.compile(r"([가-힣]+)\d*")

HEADER = """\
# 한국어 학습용 어휘 목록 {grade}등급 ({n}개, 빈도 순위순). scripts/import_ko_learner.py 로 생성.
# 출처: 국립국어원(국립국어연구원), 「한국어 학습용 어휘 목록」, 2003.
#       https://www.korean.go.kr/front_eng/down/down_02V.do?etc_seq=70
# 이용 조건: 공공누리 제1유형 (출처 표시). 변경: 동음이의어 번호를 떼고 등급별로 나눔.
"""


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    rows = Path(sys.argv[1]).read_text(encoding="cp949").splitlines()[1:]
    entries = []
    for line in rows:
        cols = line.split("\t")
        if len(cols) < 5 or not cols[0].strip().isdigit():
            continue
        m = WORD.fullmatch(cols[1].strip())
        grade = cols[4].strip()
        if m and grade in GRADES:
            entries.append((GRADES.index(grade), int(cols[0]), m.group(1)))

    out = Path(__file__).resolve().parent.parent / "assets" / "words" / "ko" / "learner"
    out.mkdir(parents=True, exist_ok=True)
    seen: set[str] = set()
    for gi, grade in enumerate(GRADES):
        words = []
        for _, _, w in sorted(e for e in entries if e[0] == gi):
            if w not in seen:
                seen.add(w)
                words.append(w)
        body = "\n".join(" ".join(words[i : i + 15]) for i in range(0, len(words), 15))
        (out / f"{grade.lower()}.txt").write_text(HEADER.format(grade=grade, n=len(words)) + body + "\n", encoding="utf-8")
        print(f"{grade.lower()}.txt: {len(words)} words")


if __name__ == "__main__":
    main()
