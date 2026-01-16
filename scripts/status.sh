#!/bin/bash
# 检查 auto-scanner 守护进程状态

if [ -f "target/debug/auto-scanner" ]; then
    ./target/debug/auto-scanner master --status
else
    echo "错误: 找不到可执行文件 target/debug/auto-scanner"
    exit 1
fi
