# Bangs

> **Note**: This code was 100% written by LLM (Claude)

DuckDuckGo Bangs 스타일의 검색 리다이렉트 서비스입니다. Chrome 검색 엔진으로 등록하여 사용할 수 있습니다.

## 기능

- **Bang 검색**: `g hello` → Google에서 "hello" 검색 (trigger가 설정에서 `g`로 지정된 경우)
- **기본 검색 엔진**: bang 없이 검색하면 설정된 기본 엔진 사용
- **홈페이지 이동**: `yt` (검색어 없이) → YouTube 홈으로 이동
- **커스텀 트리거**: `!g`, `@g`, `#g` 등 원하는 형식으로 트리거 설정 가능
- **Auto-reload**: 설정 파일 변경 시 서버 재시작 없이 자동 반영
- **Panic Recovery**: 예상치 못한 오류 발생 시 서버 유지

## 빠른 시작

### 1. 설치

```bash
go install github.com/psi59/bangs@latest
```

또는 소스에서 빌드:

```bash
git clone https://github.com/psi59/bangs.git
cd bangs
go build .
```

### 2. 설정 파일 생성

`bangs.yaml` 파일을 생성합니다:

```yaml
default_bang: g

bangs:
  - trigger: g
    name: Google
    url_template: "https://www.google.com/search?q={{{s}}}"

  - trigger: w
    name: Wikipedia
    url_template: "https://en.wikipedia.org/wiki/Special:Search?search={{{s}}}"

  - trigger: yt
    name: YouTube
    url_template: "https://www.youtube.com/results?search_query={{{s}}}"
```

샘플 파일: [bangs.example.yaml](./bangs.example.yaml)

### 3. 서버 실행

```bash
# 기본 설정 (./bangs.yaml, 포트 8080)
./bangs

# 환경변수로 설정
BANGS_CONFIG_PATH=/path/to/bangs.yaml BANGS_PORT=3000 ./bangs
```

### 4. Chrome 검색 엔진 등록

1. Chrome 설정 → 검색 엔진 → 검색 엔진 관리
2. "추가" 클릭
3. 설정:
   - **검색 엔진**: `Bangs`
   - **키워드**: `b`
   - **URL**: `http://localhost:8080/search?q=%s`

이제 주소창에서 `b g hello`를 입력하면 Google에서 "hello"를 검색합니다.

## 설정 파일 형식

```yaml
default_bang: g  # 기본 검색 엔진 (bang 없이 검색할 때 사용)

bangs:
  - trigger: g                                            # g로 트리거
    name: Google                                          # 표시 이름
    url_template: "https://www.google.com/search?q={{{s}}}"  # 검색 URL
    # home_url: "https://www.google.com"                  # 생략 시 자동 추출

  - trigger: "!yt"                                        # !yt로 트리거 (DuckDuckGo 스타일)
    name: YouTube
    url_template: "https://www.youtube.com/results?search_query={{{s}}}"

  - trigger: custom
    name: Custom Site
    url_template: "https://example.com/search?q={{{s}}}"
    home_url: "https://example.com/home"  # 명시적 지정 가능
```

### 필드 설명

| 필드 | 설명 |
|------|------|
| `trigger` | Bang 트리거. 원하는 형식으로 설정 가능 (예: `g`, `!g`, `@yt`, `#gh`) |
| `name` | 검색 엔진 이름 |
| `url_template` | 검색 URL 템플릿. `{{{s}}}`가 검색어로 대체됨 |
| `home_url` | (선택) 검색어 없이 bang만 입력 시 이동할 URL. 생략 시 `url_template`에서 자동 추출 |

## 환경변수

| 변수 | 설명 | 기본값 |
|------|------|--------|
| `BANGS_CONFIG_PATH` | 설정 파일 경로 | `./bangs.yaml` |
| `BANGS_PORT` | 서버 포트 | `8080` |

## 사용 예시

설정에 따라 트리거가 다르게 동작합니다.

**설정 예시:**
```yaml
bangs:
  - trigger: g      # "g"로 시작하는 검색
  - trigger: "!yt"  # "!yt"로 시작하는 검색
```

| 입력 | 결과 |
|------|------|
| `g hello world` | Google에서 "hello world" 검색 (trigger: `g`) |
| `!yt golang tutorial` | YouTube에서 "golang tutorial" 검색 (trigger: `!yt`) |
| `!yt` | YouTube 홈페이지로 이동 (trigger: `!yt`) |
| `hello world` | 기본 검색 엔진에서 "hello world" 검색 |

## API

### GET /search

검색 쿼리를 처리하고 적절한 URL로 리다이렉트합니다.

**Query Parameters:**
- `q`: 검색 쿼리 (필수)

**Response:**
- `302 Found`: 검색 URL로 리다이렉트
- `400 Bad Request`: 쿼리 파라미터 누락
- `503 Service Unavailable`: 설정 오류

## 개발

```bash
# Git hooks 설정 (pre-commit 린트 체크)
git config core.hooksPath .githooks

# 테스트 실행
go test ./...

# 린트 실행
golangci-lint run

# 빌드
go build .

# 실행
./bangs
```

## 라이선스

MIT
