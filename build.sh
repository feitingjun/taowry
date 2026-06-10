#!/bin/bash

# 使用 napi-rs 构建所有目标平台的 .node 文件
targets=(
  "aarch64-apple-darwin"
  "x86_64-apple-darwin"
  "x86_64-pc-windows-gnu"
)

mkdir -p binary

for target in "${targets[@]}"; do
  echo "Building for target: $target"
  npx napi build --release --target "$target"
  
  # 将生成的 .node 文件拷贝到 binary/ 目录
  node_file=$(ls *.node 2>/dev/null | head -1)
  if [ -n "$node_file" ]; then
    # 重命名为 taowry.{target}.node 格式
    dest="binary/taowry.${target}.node"
    cp "$node_file" "$dest"
    echo "Copied to $dest"
    rm "$node_file"
  fi
done
