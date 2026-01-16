#!/bin/bash
# 停止 auto-scanner 守护进程

echo "正在停止 auto-scanner 守护进程..."
if [ -f "target/debug/auto-scanner" ]; then
    ./target/debug/auto-scanner master --stop
else
    echo "错误: 找不到可执行文件 target/debug/auto-scanner"
    exit 1
fi

echo "停止命令已发送。"
