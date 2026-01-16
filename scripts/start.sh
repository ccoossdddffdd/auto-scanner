#!/bin/bash
# 启动 auto-scanner 守护进程

# 确保 cargo 已安装
if ! command -v cargo &> /dev/null; then
    echo "错误: 未安装 cargo。"
    exit 1
fi

# 构建项目
echo "正在构建项目..."
cargo build

if [ $? -ne 0 ]; then
    echo "构建失败。"
    exit 1
fi

# 检查是否设置了 INPUT_DIR，如果没有则使用默认值
if [ -z "$INPUT_DIR" ]; then
    export INPUT_DIR="input"
    mkdir -p "$INPUT_DIR"
fi

# 启动守护进程
echo "正在启动 auto-scanner 守护进程..."
# 注意：我们这里不直接使用 nohup，因为 --daemon 参数已经在代码中处理了后台运行
# 但为了确保脚本退出后进程不被杀，我们可能还是需要 nohup 或者直接依赖代码中的 daemon 逻辑
# 代码中的 daemon 逻辑使用了 daemonize crate，应该足够

./target/debug/auto-scanner master --daemon

echo "守护进程启动命令已发送。"
