# hx-exec

跨平台启动器，为 [Helix](https://helix-editor.com) 的 LSP / 外部工具提供统一的命令展开与 **多平台 / 多 shell** 别名能力。

## 为什么需要它

在 Helix 的 `languages.toml` 里配置 LSP 时，经常遇到：

```toml
command = "ngserver"
args = ["--stdio",
        "--tsProbeLocations", "$(npm -g root)",
        "--ngProbeLocations", "$(npm -g root)"]
```

问题：
- `${SOME_ENV}` / `$(cmd)` 在 **Windows** 上并不会被展开（Helix 不会走 shell）
- Windows / macOS / Linux shell 差异大，难以写同一份配置
- 命令太长，不想硬编码在 `languages.toml`
- 有时需要 **原生 shell 脚本**（pwsh 管道、bash `$(...)` 等）

`hx-exec` 解决这些：**展开参数，按平台选择变体，按需通过 shell 启动**。

## 功能

- ✅ `${VAR}` 和 `$VAR` 环境变量展开（跨平台，Windows 也能用）
- ✅ `$(cmd arg arg)` 命令替换（直接执行，不走 shell；捕获 stdout 并 trim）
- ✅ 嵌套：`$(echo ${FOO})`，转义：`\$` 输出字面量 `$`
- ✅ 预置变量：`${HELIX_CONFIG}` / `${HELIX_RUNTIME}` / `${HELIX_CACHE}`
- ✅ `hx-exec.toml` 别名配置，支持 **按 OS 匹配变体**
- ✅ 别名可指定 `shell`（`bash` / `zsh` / `fish` / `pwsh` / `powershell` / `cmd`…），
  直接写原生 shell 脚本，hx-exec 会自动调起对应 shell
- ✅ **`env` 支持绑定到命令输出**：`env.FOO = { cmd = "npm root -g" }` 或加 `shell = "pwsh"` 指定 shell
- ✅ 不指定 OS 时行为与最初版本完全一致（向后兼容）

### 预置路径解析

`${HELIX_CONFIG}`、`${HELIX_RUNTIME}`、`${HELIX_CACHE}`、`${pwd}` 是 **hx-exec 内置的预置变量**，
不受同名 env 变量影响（它们由 OS 规则直接解析到用户的 helix 目录或当前执行目录）。
若要对某个别名覆写，可在 `alias.env` 中显式指定 —— `alias.env` 优先级高于预置。

| 变量              | Linux                            | macOS              | Windows               |
| ----------------- | -------------------------------- | ------------------ | --------------------- |
| `${HELIX_CONFIG}` | `$XDG_CONFIG_HOME/helix` / `~/.config/helix` | `~/.config/helix`  | `%AppData%\helix`     |
| `${HELIX_RUNTIME}`| `$HELIX_RUNTIME` env 或 `${HELIX_CONFIG}/runtime` | 同左 | 同左 |
| `${HELIX_CACHE}`  | `$XDG_CACHE_HOME/helix` / `~/.cache/helix` | `~/Library/Caches/helix` | `%LocalAppData%\helix` |
| `${pwd}`          | 当前执行目录（`std::env::current_dir()`，不执行外部命令） | 同左 | 同左 |

> `${HELIX_RUNTIME}` 保留对 `$HELIX_RUNTIME` env 的识别，因为 Helix 本体就认这个变量。
> `${HELIX_CONFIG}` 不读任何同名 env —— Helix 本身没有这个变量约定，避免语义被污染。
> `${pwd}` 直接调用 Rust 的 `std::env::current_dir()` 获取当前目录，跨平台一致，无需 shell。

## 安装

```sh
# 从 GitHub 远程安装（推荐）
cargo install --git https://github.com/gemone/hx-exec

# 或从源码安装
git clone https://github.com/gemone/hx-exec.git
cd hx-exec
cargo install --path .
# 产物：target/release/hx-exec
```

## 用法

### 1. 直接启动（参数被展开）

```sh
hx-exec -- ngserver --stdio \
  --tsProbeLocations '$(npm -g root)' \
  --ngProbeLocations '$(npm -g root)'
```

### 2. 使用别名（最常见）

`hx-exec.toml`：

```toml
# 单表形式 —— 不指定 OS，和最初版本行为一致
[alias.rust-analyzer]
cmd = "rust-analyzer"

[alias.angular-lsp]
command = "ngserver"
args = [
  "--stdio",
  "--tsProbeLocations", "${NODE_MODULES}",
  "--ngProbeLocations", "${NODE_MODULES}",
]
env = { NODE_MODULES = "$(npm -g root)" }
```

然后：
```sh
hx-exec -c angular-lsp
hx-exec -c angular-lsp -- --extra-flag   # 附加参数
hx-exec --print -c angular-lsp           # 仅打印展开后的命令
```

### 3. 多平台变体 + 原生 shell

用数组表 `[[alias.NAME]]`，按 **声明顺序** 取第一个匹配当前 OS 的变体；
同名无 `os` 的变体作为 fallback。

```toml
# Windows 下用 pwsh 写原生脚本
[[alias.my-tool]]
os = "windows"
shell = "pwsh"
cmd = "Get-ChildItem $env:APPDATA | Where-Object { $_.Name -match 'helix' }"

# macOS / Linux 下用 bash
[[alias.my-tool]]
os = "unix"
shell = "bash"
cmd = 'ls "${HELIX_CONFIG}" | grep helix'

# 任何平台都匹配的 fallback（可省）
[[alias.my-tool]]
cmd = "echo no-native-impl"
```

### 4. OS + shell 组合 —— 典型跨平台 LSP

```toml
[[alias.angular-lsp]]
os = "windows"
shell = "pwsh"
cmd = 'ngserver --stdio --tsProbeLocations (npm -g root) --ngProbeLocations (npm -g root)'

[[alias.angular-lsp]]
os = "unix"
command = "ngserver"
args = [
  "--stdio",
  "--tsProbeLocations", "${NODE_MODULES}",
  "--ngProbeLocations", "${NODE_MODULES}",
]
env = { NODE_MODULES = "$(npm -g root)" }
```

然后 `languages.toml`：
```toml
[language-server.angular]
command = "hx-exec"
args = ["-c", "angular-lsp"]
```

### 5. env 绑定到命令输出

`alias.env` 的每个值既可以是 **字面量字符串**（现有行为），
也可以是 **命令输出**（新功能）——两者可在同一 `env` 块中混用。

```toml
[alias.eslint]
command = "vscode-eslint-language-server"
args = ["--stdio"]

# 字面量（保持现有行为）
env.EXTRA = "some-literal-value"

# 命令输出（直接执行，不走 shell — 跨平台一致）
env.NODE_PATH = { cmd = "npm root -g" }

# 命令输出 + 指定 shell（在 Windows pwsh 下解析 npm 路径）
env.NODE_PATH = { cmd = "npm root -g", shell = "pwsh" }
```

**行为说明：**
- 命令 stdout 末尾的空白/换行会被自动 trim（与 `$(cmd)` 替换一致）。
- 解析后的值既作为环境变量导出到子进程，也可在 `cmd` / `command` / `args` 的 `${VAR}` 展开中使用。
- 命令失败时报清晰错误，不会静默返回空字符串。
- `shell` 与 `alias.shell` 使用相同的名称空间（`bash` / `sh` / `zsh` / `fish` / `dash` / `pwsh` / `powershell` / `cmd`）。

**跨平台典型用法：**

```toml
# 在 Windows 下用 pwsh 获取 npm root，Unix 下直接执行
[[alias.eslint]]
os = "windows"
command = "vscode-eslint-language-server.cmd"
args = ["--stdio"]
env.NODE_PATH = { cmd = "npm root -g", shell = "pwsh" }

[[alias.eslint]]
os = "unix"
command = "vscode-eslint-language-server"
args = ["--stdio"]
env.NODE_PATH = { cmd = "npm root -g" }
```

### OS 值

| 匹配器                                     | 含义                            |
| ------------------------------------------ | ------------------------------- |
| `windows` / `win` / `win32` / `win64`      | Windows                         |
| `macos` / `mac` / `darwin` / `osx`         | macOS                           |
| `linux`                                    | Linux                           |
| `unix`                                     | 任何非 Windows 系统             |
| `any` / `*` / 省略                         | 任何系统（fallback）            |

### 支持的 shell

`bash` · `sh` · `zsh` · `fish` · `dash` · `pwsh` · `powershell` · `cmd`

`shell` 要求使用 `cmd =`（脚本字符串）而非 `command + args`。

## 展开规则

| 语法             | 不带 `shell`                                             | 带 `shell`                                 |
| ---------------- | -------------------------------------------------------- | ------------------------------------------ |
| `${NAME}`        | alias.env → 预置 → 进程环境，未找到为空                  | 同左                                       |
| `$NAME`          | 同上                                                     | **不展开**，原样传给目标 shell             |
| `$(cmd args)`    | 直接执行（不走 shell），捕获 stdout、trim                | **不展开**，原样传给目标 shell             |
| `\$`             | 输出字面量 `$`                                           | 输出字面量 `$`                             |

> 换句话说：**`shell` 为空时，hx-exec 负责一切展开**；
> **`shell` 有值时，除 `${VAR}` 保留作为跨平台桥接外，其余原生语法由目标 shell 负责**。

> ⚠️ 无 shell 的 `$(...)` 不走 shell —— 不支持管道、重定向、glob。
> 需要这些时就把别名加上 `shell = "bash"` / `"pwsh"` 等。

## 查找 `hx-exec.toml` 的顺序

1. `-f / --file <path>`
2. `./hx-exec.toml`
3. `${HELIX_CONFIG}/hx-exec.toml`
4. `<system config dir>/hx-exec/hx-exec.toml`
   - Linux: `~/.config/hx-exec/hx-exec.toml`
   - macOS: `~/Library/Application Support/hx-exec/hx-exec.toml`
   - Windows: `%AppData%\hx-exec\hx-exec.toml`

## 许可证

MIT OR Apache-2.0
