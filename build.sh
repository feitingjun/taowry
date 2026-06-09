#!/bin/bash

# 交叉编译所有目标平台的二进制文件
targets=(
  "x86_64-apple-darwin"
  "aarch64-apple-darwin"
  # "x86_64-unknown-linux-gnu"
  # "aarch64-unknown-linux-gnu"
  "x86_64-pc-windows-gnu"
  # "i686-pc-windows-gnu"
)

for target in "${targets[@]}"; do
  echo "Building for target: $target"
  cargo build --target "$target" --release
  
  old_binary="taowry"
  new_binary=$target
  
  # Windows 目标添加 .exe 后缀
  if [[ "$target" == *"windows-gnu" ]]; then
    old_binary="$old_binary.exe"
    new_binary="$new_binary.exe"
  fi

  mv target/$target/release/$old_binary binary/$new_binary

  # 本地开发时复制到 .binary/ 目录（与 postinstall 统一位置）
  current_target=$(rustc -vV | grep host | awk '{print $2}')
  if [[ "$target" == "$current_target" ]]; then
    mkdir -p .binary
    cp binary/$new_binary .binary/$old_binary
    echo "Copied to .binary/$old_binary"
  fi
done
