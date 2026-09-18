#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
CODEX_ROOT=${CODEX_HOME:-"$HOME/.codex"}
PETS_ROOT="$CODEX_ROOT/pets"
DEST="$PETS_ROOT/march-7th"
STAMP=$(date +%Y%m%d-%H%M%S)
BACKUP="$PETS_ROOT/_backups/march-7th-$STAMP"

cd "$SCRIPT_DIR"

if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 -c SHA256SUMS.txt
else
  echo "警告：未找到 shasum，跳过 SHA-256 校验。"
fi

mkdir -p "$PETS_ROOT"

if [ -d "$DEST" ]; then
  mkdir -p "$(dirname "$BACKUP")"
  cp -R "$DEST" "$BACKUP"
  echo "已备份现有版本到：$BACKUP"
fi

mkdir -p "$DEST"
cp "$SCRIPT_DIR/pet.json" "$DEST/pet.json"
cp "$SCRIPT_DIR/spritesheet.webp" "$DEST/spritesheet.webp"

echo "三月七 v1.0.0 已安装到：$DEST"
echo "请完全退出并重新打开 Codex/ChatGPT 桌面应用。"

