---
name: verify
description: bangs 서버를 빌드·실행해 /search, /suggest, /opensearch.xml을 실제 HTTP로 검증하는 레시피
---

# bangs 검증 레시피

표면은 HTTP 소켓. 테스트 재실행이 아니라 서버를 띄우고 curl로 관찰한다.

## 빌드·실행

```bash
go build -o /tmp/bangs-verify .
BANGS_CONFIG_PATH=<설정파일> BANGS_PORT=18081 /tmp/bangs-verify &
curl -s http://localhost:18081/health   # {"status":"ok"}
```

설정 예시는 `bangs.example.yaml` 참고. 제안 프록시 검증에는 로컬 echo 업스트림을
쓰면 외부 의존 없이 인코딩 왕복을 확인할 수 있다 (파이썬 http.server로
`["<q>",["echo:<q>"]]` 반환).

## 핵심 플로우

```bash
curl -s 'http://localhost:18081/search?q=g%20hello' -o /dev/null -w '%{http_code} %{redirect_url}\n'  # 302 + 대상 URL
curl -s 'http://localhost:18081/suggest?q=g%20cla'          # ["g cla",["g ...", ...]] 트리거 접두어 복원
curl -s http://localhost:18081/opensearch.xml | xmllint --noout -   # Firefox는 웰폼드 아니면 거부
curl -s http://localhost:18081/ | grep 'rel="search"'       # discovery link
```

## 찔러볼 곳

- 제안 지연: `-w '%{time_total}'` — Firefox는 500ms 넘으면 표시 안 함
- 느린/거대(>1MB)/비JSON 업스트림 → 빈 목록 `[q,[]]`로 조용히 처리되는지
- 핫 리로드: 서버 실행 중 설정에 뱅 추가 → 재시작 없이 반영 (fsnotify)
- 한글 두벌식 트리거(`호` → `gh`), 탭 구분자, `&` 포함 질의 인코딩 왕복

## 브라우저 렌더링 (선택)

이 머신에는 Chrome/Firefox 정식 설치가 없다. Playwright 캐시의 Firefox를 직접 실행:

```bash
~/Library/Caches/ms-playwright/firefox-*/firefox/Nightly.app/Contents/MacOS/firefox \
  --headless --profile $(mktemp -d) --window-size=1000,700 \
  --screenshot out.png http://localhost:18081/
```

종료 시 juggler JavaScript error 로그는 무해한 노이즈.

## 주의

- pre-commit 훅의 golangci-lint는 PATH 순서 문제로 v1이 잡힘 →
  `PATH=/Users/psi59/.local/share/mise/installs/golangci-lint/latest/golangci-lint-2.12.2-darwin-arm64:$PATH` 로 커밋
- 포트는 18081처럼 기본(8080)과 다르게 격리
