#!/bin/bash
# 运行全链条测试

echo "正在运行单元测试..."
cargo test

if [ $? -ne 0 ]; then
    echo "单元测试失败。"
    exit 1
fi

echo "正在运行集成测试..."
cargo test --test integration_test

if [ $? -ne 0 ]; then
    echo "集成测试失败。"
    exit 1
fi

echo "所有测试通过！"
