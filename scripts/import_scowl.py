#!/usr/bin/env python3
"""SCOWL 단어 목록에서 영어 연습 단어를 뽑아 assets/words/scowl/ 에 씁니다.

사용법:
    curl -L -o scowl.tar.gz https://downloads.sourceforge.net/wordlist/scowl-2020.12.07.tar.gz
    mkdir scowl && tar xzf scowl.tar.gz -C scowl
    python3 scripts/import_scowl.py scowl/scowl-2020.12.07

- 10, 20단계만 씁니다 (숫자가 작을수록 흔한 단어). 각 파일에는 그 단계에서 새로 나오는 단어만 들어갑니다.
- english(공통) + american 목록을 합칩니다.
- 영어 소문자 2글자 이상만 남깁니다 (고유명사, 약어, 아포스트로피 제외). 그 밖에 거르는 단어는 없습니다.
"""

import re
import shutil
import sys
from pathlib import Path

LEVELS = [10, 20]
VARIANTS = ["english", "american"]
WORD = re.compile(r"[a-z]{2,}")


def read_level(final: Path, level: int) -> set[str]:
    words: set[str] = set()
    for variant in VARIANTS:
        path = final / f"{variant}-words.{level}"
        if path.exists():
            for line in path.read_text(encoding="latin-1").splitlines():
                w = line.strip()
                if WORD.fullmatch(w):
                    words.add(w)
    return words


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    scowl = Path(sys.argv[1])
    out = Path(__file__).resolve().parent.parent / "assets" / "words" / "scowl"
    out.mkdir(parents=True, exist_ok=True)

    seen: set[str] = set()
    for level in LEVELS:
        words = sorted(read_level(scowl / "final", level) - seen)
        seen.update(words)
        header = (
            f"# SCOWL {level}단계에서 새로 나오는 영어 단어 ({len(words)}개). scripts/import_scowl.py 로 생성.\n"
            "# 저작권과 사용 조건: assets/words/scowl/COPYRIGHT.txt\n"
        )
        body = "\n".join(" ".join(words[i : i + 12]) for i in range(0, len(words), 12))
        (out / f"level{level}.txt").write_text(header + body + "\n", encoding="utf-8")
        print(f"level{level}.txt: {len(words)} words")

    shutil.copyfile(scowl / "Copyright", out / "COPYRIGHT.txt")


if __name__ == "__main__":
    main()
