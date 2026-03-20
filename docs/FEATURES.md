# 功能说明文档

## 洛璃 (Luoli) - 个人终端助手功能详解

---

## 目录

1. [安全管控功能](#安全管控功能)
2. [操作行为监控](#操作行为监控)
3. [命令执行系统](#命令执行系统)
4. [MCP 工具系统](#mcp-工具系统)
5. [API 集成](#api-集成)
6. [运行模式](#运行模式)
7. [配置系统](#配置系统)

---

## 安全管控功能

### 1. 命令黑白名单

#### 功能描述
通过正则表达式定义允许或禁止执行的命令，支持灵活的匹配规则。

#### 使用场景

**白名单示例**（严格模式）：
```regex
^ls\s*          # 允许 ls 命令及其变体
^cat\s+         # 允许 cat 命令
^pwd$           # 允许 pwd 命令
^echo\s+        # 允许 echo 命令
^git\s+status   # 允许 git status
```

**黑名单示例**：
```regex
rm\s+-rf\s+/           # 禁止删除根目录
>/dev/sda              # 禁止直接写入磁盘
mkfs\.                 # 禁止格式化文件系统
:(){ :|:& };:         # 禁止 fork 炸弹
dd\s+if=.*/dev/zero.*of=/dev/sda  # 禁止磁盘清零
```

#### API 使用

```python
from luoli import Assistant

assistant = Assistant()

# 添加白名单
assistant.add_to_whitelist("^ls\\s*")
assistant.add_to_whitelist("^cat\\s+")

# 添加黑名单
assistant.add_to_blacklist("rm\\s+-rf\\s+/")
```

#### CLI 使用

```bash
# 添加命令白名单
luoli config allow-command "^ls\\s*"

# 添加命令黑名单  
luoli config deny-command "rm\\s+-rf\\s+/"

# 查看所有规则
luoli config list
```

---

### 2. 工具黑白名单

#### 功能描述
管控 MCP（Model Context Protocol）工具的使用权限。

#### 内置工具

| 工具名称 | 描述 | 风险等级 |
|---------|------|---------|
| filesystem | 文件系统操作（读/写/列出） | 中 |
| shell | Shell 命令执行 | 高 |
| web_search | Web 搜索 | 低 |

#### 使用示例

```python
from luoli.security import ToolPolicy

policy = ToolPolicy()

# 添加工具白名单
policy.add_whitelist("filesystem")
policy.add_whitelist("web_search")

# 添加工具黑名单
policy.add_blacklist("shell")

# 检查工具权限
can_use = policy.check_tool("filesystem", mode="strict")
```

---

### 3. API 黑白名单

#### 功能描述
控制对外部 API 的访问权限，防止数据泄露或恶意调用。

#### 配置示例

```json
{
  "api_allowlist": [
    "https://api.github.com",
    "https://api.openai.com"
  ],
  "api_denylist": [
    "http://internal.company.com",
    "https://malicious-site.com"
  ]
}
```

#### 使用示例

```python
from luoli.security import ApiPolicy

policy = ApiPolicy()

# 添加 API 白名单
policy.add_whitelist("https://api.github.com")

# 检查 API 访问权限
can_access = policy.check_api("https://api.github.com/users/octocat", mode="strict")
```

---

### 4. 禁止模式检测

#### 功能描述
自动检测命令中是否包含敏感信息或危险模式。

#### 检测模式

| 模式 | 说明 | 示例 |
|-----|------|------|
| password | 密码相关 | `echo password123` |
| secret | 密钥相关 | `export secret_key=xxx` |
| token | 令牌相关 | `curl -H "Authorization: token xxx"` |
| api_key | API 密钥 | `api_key=sk-xxx` |
| private_key | 私钥 | `cat private_key.pem` |

#### 自定义禁止模式

```python
from luoli.security import SecurityPolicy

policy = SecurityPolicy()
policy.forbidden_patterns.append("credit_card")
policy.forbidden_patterns.append("ssn")
```

---

## 操作行为监控

### 1. 活动记录

#### 记录内容

每条活动记录包含以下信息：

```json
{
  "id": "a1b2c3d4",
  "timestamp": "2026-03-19T10:30:00Z",
  "activity_type": "command_execution",
  "user": "username",
  "command": "ls -la",
  "args": ["-la"],
  "working_dir": "/home/user",
  "status": "success",
  "duration": 0.05,
  "output": "...",
  "metadata": {}
}
```

#### 活动类型

- `command_execution`: 命令执行
- `tool_invocation`: 工具调用
- `api_call`: API 调用
- `file_access`: 文件访问
- `mode_change`: 模式切换
- `config_change`: 配置变更
- `security_violation`: 安全违规

---

### 2. 日志管理

#### 日志轮转

- **触发条件**: 日志文件达到 10MB
- **保留数量**: 默认保留 5 个历史文件
- **文件命名**: `activity.log.1`, `activity.log.2`, ...

#### 日志导出

```python
from luoli import Assistant

assistant = Assistant()

# 导出为 JSON
assistant.monitor.export_logs("/path/to/export.json", format="json")

# 导出为 CSV
assistant.monitor.export_logs("/path/to/export.csv", format="csv")
```

#### CLI 导出

```bash
# 导出日志（待实现）
luoli logs export --format json --output /path/to/export.json
```

---

### 3. 统计分析

#### 统计指标

```python
stats = assistant.get_statistics()

# 输出示例
{
    "total": 150,
    "success": 142,
    "failed": 5,
    "blocked": 3,
    "by_type": {
        "command_execution": 120,
        "tool_invocation": 20,
        "api_call": 10,
        "security_violation": 3
    }
}
```

---

## 命令执行系统

### 1. 基础执行

```python
from luoli import Assistant

assistant = Assistant()

# 执行命令
result = assistant.execute("ls -la")
print(result['stdout'])
print(result['stderr'])
print(result['returncode'])
print(result['duration'])
```

### 2. 管道命令

```python
# 执行管道命令
result = assistant.executor.execute_pipeline("cat file.txt | grep error | wc -l")
```

### 3. 超时控制

```python
# 设置 30 秒超时
result = assistant.executor.execute_with_timeout("long_running_command", timeout_secs=30)
```

### 4. 后台执行

```python
# 后台执行命令
pid = assistant.executor.execute_background("python server.py")
print(f"后台进程 PID: {pid}")
```

---

## MCP 工具系统

### 1. 文件系统工具

#### 读取文件

```python
result = assistant.execute_tool("filesystem", {
    "action": "read",
    "path": "/path/to/file.txt"
})
print(result['content'])
```

#### 写入文件

```python
result = assistant.execute_tool("filesystem", {
    "action": "write",
    "path": "/path/to/file.txt",
    "content": "Hello, World!"
})
```

#### 列出目录

```python
result = assistant.execute_tool("filesystem", {
    "action": "list",
    "path": "/path/to/dir"
})
for entry in result['entries']:
    print(f"{entry['name']} - {'目录' if entry['is_dir'] else '文件'}")
```

---

### 2. Shell 工具

```python
result = assistant.execute_tool("shell", {
    "command": "echo Hello"
})
print(result['stdout'])
```

---

### 3. Web 搜索工具

```python
result = assistant.execute_tool("web_search", {
    "query": "Rust programming language"
})
print(result['results'])
```

---

## API 集成

### 1. HTTP 请求

```python
from luoli.api import ApiClient

client = ApiClient(config, security, monitor)

# GET 请求
response = client.get("https://api.github.com/users/octocat")

# POST 请求
response = client.post("https://api.example.com/data", {
    "key": "value"
})
```

### 2. GitHub API

```python
from luoli.api import GitHubApi

github = GitHubApi(client)

# 获取用户信息
user = github.get_user("octocat")

# 获取仓库信息
repo = github.get_repo("openclaw", "openclaw")

# 列出仓库文件
contents = github.list_contents("openclaw", "openclaw", "src")
```

### 3. OpenAI API

```python
from luoli.api import OpenAiApi

openai = OpenAiApi(client, api_key="your-api-key")

# 发送聊天请求
response = openai.chat_completion(
    model="gpt-4",
    messages=[
        {"role": "user", "content": "Hello!"}
    ]
)
```

---

## 运行模式

### 1. 普通模式 (Normal)

**特点**:
- 允许执行大部分命令
- 仅检查黑名单
- 适合日常开发使用

**使用场景**:
- 开发环境
- 个人日常使用
- 信任的执行环境

```bash
luoli shell --mode normal
```

---

### 2. 严格模式 (Strict)

**特点**:
- 只允许白名单内的命令
- 只允许白名单内的工具
- 只允许白名单内的 API
- 最高安全性

**使用场景**:
- 生产环境
- 自动化脚本
- 不可信输入处理
- 多用户环境

```bash
luoli shell --mode strict
```

---

### 3. 模式切换

```python
# 切换到严格模式
assistant.set_mode("strict")

# 切换到普通模式
assistant.set_mode("normal")

# 查看当前模式
current_mode = assistant.get_mode()
```

---

## 配置系统

### 1. 配置文件结构

```json
{
  "security": {
    "default_mode": "normal",
    "command_allowlist": [],
    "command_denylist": [],
    "tool_allowlist": [],
    "tool_denylist": [],
    "api_allowlist": [],
    "api_denylist": [],
    "max_command_length": 4096,
    "forbidden_patterns": []
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
    "tools": []
  },
  "api": {
    "enabled": true,
    "endpoints": []
  }
}
```

### 2. 配置加载顺序

1. 默认配置
2. 配置文件 (`~/.luoli/config.json`)
3. 环境变量
4. 运行时参数

### 3. 环境变量

| 变量名 | 说明 | 示例 |
|-------|------|------|
| `LUOLI_MODE` | 默认运行模式 | `strict` |
| `LUOLI_CONFIG` | 配置文件路径 | `/path/to/config.json` |
| `LUOLI_LOG_LEVEL` | 日志级别 | `debug` |

---

## 最佳实践

### 1. 安全配置

```python
# 生产环境配置
assistant = Assistant(mode="strict")

# 添加必要的白名单
assistant.add_to_whitelist("^ls\\s*")
assistant.add_to_whitelist("^cat\\s+")
assistant.add_to_whitelist("^git\\s+status$")
assistant.add_to_whitelist("^git\\s+log$")
```

### 2. 监控和审计

```python
# 定期检查日志
logs = assistant.get_logs(limit=100)
blocked = [log for log in logs if log['status'] == 'blocked']

if blocked:
    print(f"发现 {len(blocked)} 个被阻止的操作")
    for log in blocked:
        print(f"  - {log['command']}: {log['metadata'].get('reason')}")
```

### 3. 错误处理

```python
from luoli.assistant import SecurityError, ExecutionError

try:
    result = assistant.execute("rm -rf /")
except SecurityError as e:
    print(f"安全错误: {e}")
except ExecutionError as e:
    print(f"执行错误: {e}")
```

---

## 故障排除

### 常见问题

#### 1. 命令被拒绝

**问题**: 命令在严格模式下被拒绝

**解决**: 添加命令到白名单
```python
assistant.add_to_whitelist("^your_command\\s*")
```

#### 2. 日志文件过大

**问题**: 日志文件占用太多磁盘空间

**解决**: 调整日志轮转配置
```json
{
  "monitor": {
    "max_log_size": 5242880,  // 5MB
    "log_rotation": 3
  }
}
```

#### 3. 工具无法使用

**问题**: MCP 工具被阻止

**解决**: 检查工具白名单
```python
# 列出可用工具
tools = assistant.list_tools()
print(tools)

# 添加工具白名单
assistant.security.tool_policy.add_whitelist("tool_name")
```

---

## 更多资源

- [API 文档](API.md)
- [开发指南](DEVELOPMENT.md)
- [安全白皮书](SECURITY.md)
- [版本历史](../VERSION.md)
