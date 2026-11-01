# taja

자판을 처음 배우는 사람을 위한 터미널 타자 연습기입니다 (Rust).

홈열 키 몇 개로 시작해서, 열린 키가 **모두** 목표 속도와 정확도를 넘으면 다음 키가 하나씩 열립니다.
연습 문장은 열린 키로만 만들어지고, 가장 약한 키(집중 키)가 더 자주 나옵니다.

## 실행

```sh
cargo run --release                     # QWERTY
cargo run --release -- --layout mine    # 직접 만든 레이아웃
cargo run --release -- --help           # 옵션 보기
```

| 키 | 동작 |
|---|---|
| 글자 | 입력 (틀리면 맞는 키를 칠 때까지 커서가 멈춤) |
| `Tab` | 새 줄 |
| `Esc` / `Ctrl+C` | 저장하고 종료 |

옵션:

- `--target-wpm 25`: 키별 목표 속도 (기본 25)
- `--target-acc 95`: 키별 목표 정확도 % (기본 95)
- `--reset`: 이 레이아웃의 진도를 지우고 처음부터
- `--paths`: 레이아웃 폴더와 진도 폴더 위치 보기

## 레이아웃 파일

10×3 글자만 적으면 됩니다. 공백은 무시하고, **영어 글자(a~z)가 아닌 칸은 모두 빈 칸**(키 없음)입니다.
빈 칸은 `.`, `;`, `-` 등 아무 기호로 적으면 됩니다. 내장 QWERTY의 `; , . /` 자리도 빈 칸입니다.

```
qwertyuiop
asdfghjkl;
zxcvbnm,./
```

`taja --paths`로 나오는 레이아웃 폴더(Linux: `~/.config/taja/layouts/`)에 `mine.txt`로 저장하고
`--layout mine`으로 실행합니다. 파일 경로를 바로 줘도 됩니다 (`--layout ./mine.txt`).

저장소에 있는 예시: `layouts/mine.txt`

```
pmal.zbfjy
inerqxot.s
kvgd..hcuw
```

```sh
cargo run --release -- --layout layouts/mine.txt
```

이 배열의 계산 결과: 시작 키 `r o e t` (4개) → `s n i a d c l g p m u h f b y v w k x q j z`
레이아웃은 OS나 키보드 펌웨어에 설치된 상태여야 합니다. 프로그램은 들어온 글자를 그대로 비교합니다.

나머지는 모두 위치로 자동 계산합니다:

- **손가락**: 열 번호로 정함 (0 왼손 새끼 … 3·4 왼손 검지 | 5·6 오른손 검지 … 9 오른손 새끼)
- **시작 키 (4~8개 자동)**: 홈열에서 검지 → 중지 → 약지 → 새끼 순으로 하나씩 늘리면서,
  그 키들로 칠 수 있는 진짜 단어가 10개 이상 되는 **가장 적은 개수**에서 멈춤.
  8개로도 부족하고 모음이 없으면 가장 쉬운 자리의 모음을 하나 추가
  - QWERTY: `f j d k s l`에는 모음이 없어서 `a`까지 7개 (`fjdksla`)
  - 홈열이 `arstdhneio`인 레이아웃: `t n s e` 4개로 충분
- **언락 순서**: "이 키를 열면 새로 칠 수 있게 되는 단어의 양 ÷ 그 자리의 치기 어려운 정도"가 가장 큰 키부터
  - QWERTY 결과: `fjdksla` → `e i r t c o n g u m p h v y w b x q z`

진도는 레이아웃마다 따로 저장됩니다 (Linux: `~/.local/share/taja/profiles/<이름>.json`).
레이아웃 파일 내용이 바뀌면 진도를 새로 시작합니다.

## 언락 기준

- 키마다 최근 30번의 입력으로 평가
- **속도**: 바로 앞 키를 친 뒤 이 키를 칠 때까지 걸린 시간의 평균 (2초 넘게 쉰 입력은 제외)
- **정확도**: 한 번에 맞게 친 비율
- 열린 키가 모두 20번 이상 입력되고, 목표 속도와 목표 정확도를 넘으면 다음 키가 열림

## 연습 문장

- 내장 영어 단어 목록(`assets/words/`, 약 1만 1천 개)에서 열린 키로만 된 단어를 고름
  - `common.txt`: 아주 흔한 기본 단어 (가중치 3)
  - `scowl/level10.txt`: SCOWL 10단계, 가장 흔한 단어 약 4천 개 (가중치 2)
  - `early.txt`, `verbs.txt`, `nouns.txt`, `adjectives.txt`, `extra.txt`: 직접 고른 단계별·품사별 단어 (가중치 1)
  - `scowl/level20.txt`: SCOWL 20단계, 흔한 단어 약 7천 개 (가중치 0.7)
  - 공백이나 줄바꿈으로 구분하고 `#` 줄은 주석. 파일에 단어를 추가하면 다음 빌드부터 반영됨
- 진짜 단어가 적은 초반에는 영어 글자 연결 빈도로 만든 가짜 단어를 섞음 (예: `flads`, `kalds`)
- 단어의 절반 정도에 집중 키가 들어감

## 구조

```
src/
├── main.rs              옵션, 터미널 시작/종료, 이벤트 루프
├── app.rs               앱 상태 (지금 줄, 다음 줄, 언락 알림)
├── lesson.rs            한 줄 입력 처리, 오타 멈춤, 키 간격 측정
├── progress.rs          키별 통계, 언락 판정, 집중 키, 진도 데이터
├── storage.rs           파일 위치, 진도 저장/불러오기
├── ui.rs                화면 그리기
├── layout/
│   ├── mod.rs           레이아웃 파일 읽기/검사
│   ├── cost.rs          손가락 배정, 자리별 비용표
│   └── plan.rs          시작 키, 언락 순서 계산
└── words/
    ├── mod.rs           단어 목록, 열린 키로 거르기
    ├── generator.rs     가짜 단어 생성기
    └── line.rs          한 줄 만들기
```

```sh
cargo test
```

## 단어 목록 출처

영어 단어 일부는 [SCOWL](http://wordlist.aspell.net/) (Spell Checker Oriented Word Lists, Kevin Atkinson)
2020.12.07 판의 10·20단계에서 가져왔습니다. 저작권 표시와 사용 조건은 `assets/words/scowl/COPYRIGHT.txt`에 있습니다.
`scripts/import_scowl.py`로 다시 만들 수 있고, 이때 영어 소문자 단어만 남기고 부적절한 단어는 뺍니다.

## 한글 단어 목록 (준비 중)

`assets/words/ko/`에 한글 단어를 미리 만들어 두었습니다. 아직 프로그램에는 연결하지 않았습니다.
완성된 한글로 저장하므로, 나중에 자모로 분해해서 어떤 한글 자판에도 쓸 수 있습니다.

| 파일 | 내용 |
|---|---|
| `basic.txt` | 아주 자주 쓰는 기본 단어 |
| `short.txt`, `short2.txt` | 받침 없는 단어, 받침 하나짜리 짧은 단어 (초반 단계용) |
| `nouns.txt`, `daily.txt`, `themes.txt` | 주제별 명사, 일상 명사, 의성어·의태어 |
| `verbs.txt`, `verbs2.txt`, `adjectives.txt` | 동사, 형용사, 부사와 활용형 |

## 앞으로

- 한글 두벌식: 프로그램 안에 자모 조합기를 만들어 물리 키 단위로 측정
- 대문자(Shift), 숫자
- 기록 그래프
