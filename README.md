# 洛璃 (Luoli) - 个人终端助手

👩‍💻 一个安全、可控的个人终端助手，参考 OpenClaw 设计，使用 Rust 和 Python 构建。

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Python](https://img.shields.io/badge/Python-3.8%2B-blue.svg)](https://www.python.org)

---

## 功能特性

### 🔒 安全管控

- **命令黑白名单**: 支持正则表达式匹配
- **工具黑白名单**: MCP 工具使用管控
- **API 黑白名单**: 外部接口访问控制
- **禁止模式检测**: 自动识别敏感信息（密码、密钥等）
- **双重运行模式**:
  - **普通模式**: 允许大部分命令，仅检查黑名单
  - **严格模式**: 只允许白名单内的命令、工具和 API

### 📊 操作监控

- 完整的命令执行审计日志
- 安全违规检测和告警
- 日志轮转和导出（JSON/CSV）
- 执行统计分析

### 🛠️ MCP 工具

- 文件系统操作（读/写/列出）
- Shell 命令执行
- Web 搜索
- 可扩展的工具框架

### 🔌 API 集成

- HTTP 客户端封装
- GitHub API 支持
- OpenAI API 支持
- 自定义 API 端点

---

## 快速开始

### 安装

#### 从源码构建

```bash
# 克隆仓库
git clone https://github.com/xzwx666/luoli-AI.git
cd luoli-AI

# 构建 Rust 项目
cargo build --release

# 安装 Python 包
cd python
pip install -e .
```

#### 使用 Cargo 安装

```bash
cargo install luoli-assistant
```

### 初始化配置

```bash
# 初始化默认配置
luoli init

# 或手动创建配置目录
mkdir -p ~/.luoli/logs
```

### 基本使用

#### 交互式终端

```bash
# 启动普通模式
luoli shell

# 启动严格模式
luoli shell --mode strict
```

#### 执行单个命令

```bash
luoli exec "ls -la"
luoli exec "cat file.txt" --mode strict
```

#### 配置黑白名单

```bash
# 添加命令白名单
luoli config allow-command "^ls\s*"
luoli config allow-command "^cat\s+"

# 添加命令黑名单
luoli config deny-command "rm\s+-rf\s+/"

# 添加工具白名单
luoli config allow-tool filesystem

# 查看所有规则
luoli config list
```

#### 查看日志

```bash
# 查看最近 50 条日志
luoli logs

# 查看最近 100 条日志
luoli logs --limit 100
```

---

## Python API 使用

### 基础用法

```python
from luoli import Assistant

# 创建助手实例（普通模式）
assistant = Assistant(mode="normal")

# 执行命令
result = assistant.execute("ls -la")
print(result['stdout'])

# 切换到严格模式
assistant.set_mode("strict")

# 添加白名单
assistant.add_to_whitelist("^ls\s*")
assistant.add_to_whitelist("^cat\s+")

# 执行命令（严格模式下）
result = assistant.execute("ls -la")  # 成功
result = assistant.execute("echo hello")  # 失败，不在白名单
```

### 交互式 Shell

```python
from luoli import Assistant

assistant = Assistant()
assistant.interactive_shell()
```

### 查看日志和统计

```python
# 获取最近日志
logs = assistant.get_logs(limit=10)
for log in logs:
    print(f"[{log['timestamp']}] {log['command']} - {log['status']}")

# 获取统计信息
stats = assistant.get_statistics()
print(f"总执行: {stats['total']}")
print(f"成功: {stats['success']}")
print(f"失败: {stats['failed']}")
print(f"阻止: {stats['blocked']}")
```

---

## 配置文件

配置文件位于 `~/.luoli/config.json`：

```json
{
  "security": {
    "default_mode": "normal",
    "command_allowlist": [
      "^ls\\s*",
      "^cat\\s+",
      "^pwd$",
      "^echo\\s+"
    ],
    "command_denylist": [
      "rm\\s+-rf\\s+/",
      ">/dev/sda",
      "mkfs\\."
    ],
    "tool_allowlist": [
      "filesystem",
      "shell"
    ],
    "tool_denylist": [],
    "api_allowlist": [
      "https://api.github.com"
    ],
    "api_denylist": [],
    "max_command_length": 4096,
    "forbidden_patterns": [
      "password",
      "secret",
      "token",
      "api_key"
    ]
  },
  "monitor": {
    "enabled": true,
    "log_file": "~/.luoli/logs/activity.log",
    "max_log_size": 10485760,
    "log_rotation": 5,
    "capture_output": true,
    "capture_env": false
  },
  "mcp": {
    "enabled": true,
    "tools": [
      {
        "name": "filesystem",
        "enabled": true,
        "config": {}
      }
    ]
  },
  "api": {
    "enabled": true,
    "endpoints": []
  }
}
```

---

## 项目结构

```
luoli-AI/
├── Cargo.toml              # Rust 项目配置
├── src/
│   ├── main.rs            # CLI 入口
│   ├── lib.rs             # Python 绑定
│   ├── config.rs          # 配置管理
│   ├── security.rs        # 安全管控
│   ├── monitor.rs         # 操作监控
│   ├── executor.rs        # 命令执行
│   ├── mcp.rs             # MCP 工具
│   └── api.rs             # API 客户端
├── python/
│   ├── setup.py           # Python 包配置
│   └── luoli/
│       ├── __init__.py
│       ├── assistant.py   # 主助手类
│       ├── security.py    # 安全策略
│       └── monitor.py     # 监控模块
├── docs/                  # 文档
├── VERSION.md             # 版本历史
└── README.md              # 本文件
```

---

## 架构说明

### Rust 核心层

- **高性能**: 使用 Rust 实现核心引擎，确保命令执行效率
- **内存安全**: Rust 的所有权系统保证内存安全
- **异步支持**: 基于 Tokio 的异步运行时

### Python API 层

- **易用性**: Python 友好的 API 设计
- **生态整合**: 可轻松集成到 Python 项目和工作流
- **PyO3 绑定**: 高性能的 Rust-Python 互操作

### 安全架构

```
用户输入
    ↓
安全策略检查（黑白名单）
    ↓
命令执行
    ↓
活动监控（日志记录）
    ↓
结果返回
```

---

## 开发指南

### 构建项目

```bash
# 调试构建
cargo build

# 发布构建
cargo build --release

# 运行测试
cargo test

# 代码格式化
cargo fmt

# 静态检查
cargo clippy
```

### Python 开发

```bash
cd python

# 安装开发依赖
pip install -e ".[dev]"

# 运行测试
pytest

# 代码格式化
black luoli/

# 类型检查
mypy luoli/
```

---

## 安全建议

1. **生产环境使用严格模式**: 在生产环境中建议启用严格模式，只允许必要的命令
2. **定期审查日志**: 定期检查活动日志，发现异常行为
3. **配置白名单**: 明确允许执行的命令，避免意外执行危险操作
4. **保护配置文件**: 配置文件可能包含敏感信息，确保适当的文件权限
5. **及时更新**: 保持软件更新，获取最新的安全修复

---

## 贡献指南

欢迎贡献代码、报告问题或提出功能建议！

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建 Pull Request

---

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

---

## 致谢

- 参考 [OpenClaw](https://github.com/openclaw/openclaw) 的设计理念和架构
- Rust 社区提供的优秀工具和库
- PyO3 项目提供的 Python 绑定支持

---

## 联系方式

- 项目主页: https://github.com/xzwx666/luoli-AI
- 问题反馈: https://github.com/xzwx666/luoli-AI/issues

---

**用 ❤️ 和 👩‍💻 构建**
