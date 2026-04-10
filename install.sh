#!/usr/bin/env bash

if [ -z "${BASH_VERSION:-}" ]; then
    echo "오류: 이 설치 스크립트는 bash가 필요합니다." >&2
    echo "다음처럼 실행하세요: curl -fsSL https://raw.githubusercontent.com/gigagookbob/rwd/main/install.sh | bash" >&2
    exit 1
fi

set -euo pipefail

# rwd 설치 스크립트
# 사용법: curl -fsSL https://raw.githubusercontent.com/gigagookbob/rwd/main/install.sh | bash

REPO="gigagookbob/rwd"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="rwd"

# 최신 릴리즈 태그 가져오기
VERSION=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
    echo "오류: 릴리즈 버전을 가져올 수 없습니다."
    exit 1
fi

echo "rwd ${VERSION} 설치 중..."

# 아키텍처 감지
ARCH=$(uname -m)
OS=$(uname -s)

case "${OS}-${ARCH}" in
    Darwin-arm64)
        ASSET="rwd-aarch64-apple-darwin.tar.gz"
        ;;
    Darwin-x86_64)
        ASSET="rwd-x86_64-apple-darwin.tar.gz"
        ;;
    Linux-x86_64)
        ASSET="rwd-x86_64-unknown-linux-gnu.tar.gz"
        ;;
    *)
        echo "오류: 지원하지 않는 플랫폼입니다: ${OS}-${ARCH}"
        echo "소스에서 직접 빌드하세요: cargo install --git https://github.com/${REPO}.git"
        exit 1
        ;;
esac

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"

# 임시 디렉토리에 다운로드
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "다운로드: ${DOWNLOAD_URL}"
curl -fsSL "$DOWNLOAD_URL" -o "${TMP_DIR}/${ASSET}"

# 압축 해제
tar -xzf "${TMP_DIR}/${ASSET}" -C "$TMP_DIR"

# 바이너리 이름 찾기 (tar에서 추출된 파일)
EXTRACTED=$(find "$TMP_DIR" -type f -name "rwd*" ! -name "*.tar.gz" | head -1)

if [ -z "$EXTRACTED" ]; then
    echo "오류: 바이너리를 찾을 수 없습니다."
    exit 1
fi

# 설치
chmod +x "$EXTRACTED"
if [ -w "$INSTALL_DIR" ]; then
    mv "$EXTRACTED" "${INSTALL_DIR}/${BINARY_NAME}"
else
    echo "관리자 권한이 필요합니다."
    sudo mv "$EXTRACTED" "${INSTALL_DIR}/${BINARY_NAME}"
fi

# 기본 출력 디렉토리 생성 (~/.rwd/output/)
# Obsidian vault를 설정하지 않아도 바로 사용할 수 있도록 미리 만들어 둡니다.
DEFAULT_OUTPUT="${HOME}/.rwd/output"
mkdir -p "$DEFAULT_OUTPUT"

echo ""
echo "rwd ${VERSION} 설치 완료!"
echo "위치: ${INSTALL_DIR}/${BINARY_NAME}"
echo "기본 출력 경로: ${DEFAULT_OUTPUT}"
echo ""
echo "시작하기:"
echo "  rwd init     # 초기 설정 (API 키, 출력 경로)"
echo "  rwd today    # 오늘의 세션 분석"
