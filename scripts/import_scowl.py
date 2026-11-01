#!/usr/bin/env python3
"""SCOWL 단어 목록에서 연습용 영어 단어를 뽑아 assets/words/scowl/ 에 씁니다.

사용법:
    curl -L -o scowl.tar.gz https://downloads.sourceforge.net/wordlist/scowl-2020.12.07.tar.gz
    mkdir scowl && tar xzf scowl.tar.gz -C scowl
    python3 scripts/import_scowl.py scowl/scowl-2020.12.07

- 10단계(가장 흔한 단어)와 20단계만 씁니다. 35단계부터는 어려운 단어가 많습니다.
- english(공통) + american 목록을 합칩니다.
- 영어 소문자 2글자 이상만 남깁니다 (고유명사, 약어, 아포스트로피 제외).
- 초보 연습에 맞지 않는 단어(욕설, 성적·폭력적 단어 등)는 뺍니다.
"""

import re
import shutil
import sys
from pathlib import Path

LEVELS = [10, 20]
VARIANTS = ["english", "american"]

# 이 패턴으로 시작하는 단어는 뺍니다.
BLOCKED_PREFIXES = [
    "fuck", "shit", "bitch", "cunt", "dick", "cock", "piss", "bastard", "slut",
    "whore", "nigg", "fag", "damn", "sex", "porn", "rape", "rapist", "kill",
    "crap", "arse", "boob", "penis", "vagina", "nazi", "drug", "cocaine",
    "heroin", "suicid", "murder", "retard", "abort", "abus", "erotic", "orgasm",
    "nude", "naked", "genital", "brothel", "prostitut", "masturb",
]
# 이 단어와 정확히 같으면 뺍니다.
BLOCKED_WORDS = {"hell", "ass", "asses", "tit", "tits", "bloody", "gun", "guns"}

WORD = re.compile(r"[a-z]{2,}")


def blocked(word: str) -> bool:
    return word in BLOCKED_WORDS or any(word.startswith(p) for p in BLOCKED_PREFIXES)


def read_level(final: Path, level: int) -> set[str]:
    words: set[str] = set()
    for variant in VARIANTS:
        path = final / f"{variant}-words.{level}"
        if not path.exists():
            continue
        for line in path.read_text(encoding="latin-1").splitlines():
            w = line.strip()
            if WORD.fullmatch(w) and not blocked(w):
                words.add(w)
    return words


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    scowl = Path(sys.argv[1])
    final = scowl / "final"
    out = Path(__file__).resolve().parent.parent / "assets" / "words" / "scowl"
    out.mkdir(parents=True, exist_ok=True)

    seen: set[str] = set()
    for level in LEVELS:
        words = sorted(read_level(final, level) - seen)
        seen.update(words)
        header = (
            f"# SCOWL {level}단계 영어 단어 ({len(words)}개). scripts/import_scowl.py 로 생성.\n"
            "# 저작권과 사용 조건: assets/words/scowl/COPYRIGHT.txt\n"
        )
        lines = [" ".join(words[i : i + 12]) for i in range(0, len(words), 12)]
        (out / f"level{level}.txt").write_text(header + "\n".join(lines) + "\n", encoding="utf-8")
        print(f"level{level}.txt: {len(words)} words")

    shutil.copyfile(scowl / "Copyright", out / "COPYRIGHT.txt")


if __name__ == "__main__":
    main()
