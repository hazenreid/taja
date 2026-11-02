#!/usr/bin/env python3
"""영화 자막 빈도 목록(FrequencyWords, OpenSubtitles 2018)에서 한글 연습 단어를 뽑습니다.

사용법:
    curl -L -o ko_50k.txt \\
      https://raw.githubusercontent.com/hermitdave/FrequencyWords/master/content/2018/ko/ko_50k.txt
    python3 scripts/import_ko_subtitles.py ko_50k.txt

결과: assets/words/ko/subtitles/top.txt (빈도순, CC BY-SA 4.0)

- 빈도 상위 TOP_N개 줄만 봅니다.
- 완성된 한글(가~힣)로만 된 단어만 남깁니다.
- 욕설, 성인·폭력 단어, 흔한 외국 이름(조사가 붙은 형태 포함)은 뺍니다.
  자막 특성상 영화 속 인물 이름이 일부 남을 수 있습니다.
"""

import sys
from pathlib import Path

TOP_N = 10000
MAX_LEN = 6

# 이 글자들이 들어 있으면 뺍니다.
BLOCKED_PARTS = [
    "씨발", "시발", "씨팔", "개새", "새끼", "좆", "병신", "지랄", "염병", "빌어먹", "젠장", "엿먹",
    "닥쳐", "꺼져", "섹스", "창녀", "매춘", "마약", "강간", "자살", "살인", "죽여", "죽일", "죽였",
    "죽인", "죽어", "죽을", "놈", "미친", "담배", "망할", "쌍년", "썅", "제기랄", "니미", "총알",
    "시체", "변태", "성기", "알몸", "나체", "포르노", "술집", "취했", "개자식", "코카인", "헤로인",
]

# 흔한 영어권 이름의 한글 표기. 한국어 일반 단어와 겹치는 것(김, 칼, 짐, 밥, 팀, 조, 진, 영 등)은 넣지 않음.
NAMES = """
마이클 존 데이비드 데이빗 제임스 로버트 윌리엄 윌 리처드 리차드 토마스 토머스 찰스 찰리 조셉 대니얼 다니엘
매튜 매트 맷 앤서니 토니 마크 폴 스티븐 스티브 앤드류 앤디 케빈 브라이언 조지 에드워드 에드 로널드 티모시
제이슨 제프 제프리 라이언 제이콥 제이크 게리 니콜라스 닉 에릭 조나단 조너선 래리 저스틴 스콧 브랜던 벤저민
벤 샘 사무엘 그레고리 그렉 프랭크 알렉산더 알렉스 레이먼드 레이 패트릭 잭 데니스 제리 타일러 아론 호세
애덤 아담 헨리 네이선 더글러스 피터 피트 카일 월터 이선 에단 제레미 해롤드 키스 크리스 크리스찬 로저 테리
숀 션 오스틴 아서 로렌스 조 조이 제시 딜런 빌리 빌 브루스 앨버트 윌리 가브리엘 로건 랄프 로이 유진 루이스
러셀 바비 필립 조니 조쉬 테드 프레드 해리 레오 루크 데이브 마이크 데릭 대니 데니 토비 로비 래리 해리스
잭슨 톰 토미 맥스 댄 앤 앤드리아 메리 패트리샤 제니퍼 젠 린다 엘리자베스 리즈 바바라 수잔 수지 제시카 사라
세라 캐런 낸시 리사 베티 마가렛 샌드라 애슐리 킴벌리 에밀리 도나 미셸 캐롤 아만다 멜리사 데보라 스테파니
레베카 로라 샤론 신시아 캐슬린 에이미 셜리 안젤라 헬렌 안나 애나 브렌다 파멜라 니콜 엠마 사만다 캐서린 케이트
케이티 크리스틴 크리스티 레이첼 캐롤린 재닛 마리아 헤더 다이앤 줄리 조이스 빅토리아 켈리 크리스티나 로렌
올리비아 프랜시스 프란시스 마사 메건 한나 재클린 앨리스 그레이스 소피아 소피 클로이 루시 릴리 엘라 제니
로즈 캐리 캐시 베스 로리 로니 지니 리지 패티 베니 비니 루디 제스 레지 데릴 토드 루시퍼 메러디스 베일리 버크
파블로 그레이 체이스 셜록 왓슨 클라크 스미스 존슨 윌슨 테일러 앤더슨 마틴 톰슨 가르시아 로빈슨 워커 라이트
로페즈 아담스 베이커 넬슨 카터 미첼 로버츠 터너 필립스 캠벨 파커 에반스 콜린스 스튜어트 모리스 머피 로저스
모건 쿠퍼 피터슨 리드 하워드 브룩스 베넷 레예스 크루즈 휴즈 마이어스 포스터 샌더스 로스 설리번 젠킨스 페리
버틀러 피셔 헨더슨 콜먼 패터슨 조던 레이놀즈 해밀턴 그레이엄 트래비스 캐롤 칼리 캐스 로렌스 시드니 피츠
제프리 랄프 해롤드 롤로 토레스 존스 헬렌 데로 프레디 테디 에디 지미 조이 바니 마빈 해럴드 웨인 브래드 칼슨
웨버 미란다 캘리 엘리스 엘리 몰리 리디 버트 애디슨 앨리샤 앨리슨 에린 엘렌 오드리 미스 칼로 디트 트레
""".split()

PARTICLES = sorted(
    ["가", "이", "는", "은", "을", "를", "와", "과", "의", "도", "야", "아", "만", "랑", "이랑", "에게",
     "한테", "에게서", "한테서", "씨", "님", "요", "처럼", "보다", "부터", "까지", "이가", "이는", "이를",
     "이야", "이도", "이의", "이한테", "이에게"],
    key=len,
    reverse=True,
)

NAME_SET = set(NAMES)


def is_hangul(w: str) -> bool:
    return all("가" <= c <= "힣" for c in w)


def is_name(w: str) -> bool:
    if w in NAME_SET:
        return True
    return any(w.endswith(p) and w[: -len(p)] in NAME_SET for p in PARTICLES)


def keep(w: str) -> bool:
    return (
        0 < len(w) <= MAX_LEN
        and is_hangul(w)
        and not any(p in w for p in BLOCKED_PARTS)
        and not is_name(w)
    )


HEADER = """\
# 영화 자막 빈도순 한글 단어 ({n}개). scripts/import_ko_subtitles.py 로 생성.
# 출처: FrequencyWords (Hermit Dave), OpenSubtitles 2018 한국어 자막 빈도 목록 상위 {top}개
#       https://github.com/hermitdave/FrequencyWords
# 라이선스: CC BY-SA 4.0 (https://creativecommons.org/licenses/by-sa/4.0/)
# 변경: 한글만 남기고 욕설·성인 단어·흔한 외국 이름을 뺌. 이 파일도 CC BY-SA 4.0을 따릅니다.
"""


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    lines = Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()[:TOP_N]
    words: list[str] = []
    seen: set[str] = set()
    for line in lines:
        parts = line.split()
        if not parts:
            continue
        w = parts[0]
        if keep(w) and w not in seen:
            seen.add(w)
            words.append(w)

    out = Path(__file__).resolve().parent.parent / "assets" / "words" / "ko" / "subtitles"
    out.mkdir(parents=True, exist_ok=True)
    body = "\n".join(" ".join(words[i : i + 15]) for i in range(0, len(words), 15))
    (out / "top.txt").write_text(HEADER.format(n=len(words), top=TOP_N) + body + "\n", encoding="utf-8")
    print(f"top.txt: {len(words)} words (from top {TOP_N})")


if __name__ == "__main__":
    main()
