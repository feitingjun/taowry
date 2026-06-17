#!/bin/bash

# 使用 napi-rs 构建当前平台的 .node 文件
# 完整的多平台构建请通过 GitHub Actions CI（推送 v* 标签触发）
target=$(rustc -vV | grep host | awk '{print $2}')

mkdir -p binary

echo "Building for target: $target"
npx napi build --release --target "$target"

# 将生成的 .node 文件拷贝到 binary/ 目录
node_file=$(ls *.node 2>/dev/null | head -1)
if [ -n "$node_file" ]; then
  dest="binary/taowry.${target}.node"
  cp "$node_file" "$dest"
  echo "Copied to $dest"
  rm "$node_file"
fi
