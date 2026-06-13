# Shell 内联 preedit（渲染真实 prompt）设计

- 日期：2026-06-13
- 相关代码：`rsime/src/main.rs`（`run_tui` / `tui_loop` / `print_shell_bind` / `read_shell_context`）
- 相关 commit：`3ae87ad`（feat: inline preedit with shell command context）、`6ce81ff`（fix: zsh keybinding）

## 背景

`rsime tui` 在 shell 快捷键绑定（bash `bind -x` / zsh `zle` / fish `bind`）下被唤起时，
会读取 shell 传来的命令行上下文（`RSIME_READLINE_LINE` / `RSIME_READLINE_POINT`），
在 TUI 第一行内联显示 preedit。

当前第一行使用 rsime 自己的 `❯` 符号**替换**掉 shell 的真实 prompt：

```
❯ cd rsimeda zi ce shi|
  1.打字测试  2.打字  3.大字
```

存在两个问题：

1. **prompt 残影**：ratatui 的 `Inline(2)` 视口第 0 行画在光标所在行（即 prompt 那一行），
   但 ratatui 是增量 diff 渲染，只写自己有内容的格子、不清整行。当 rsime 画的 `❯ ...`
   比 shell 真实 prompt 短时（例如 fish 的长 prompt `black-desk-ThinkPad... ~/D/w/r/rsime (tui)>`），
   prompt 右半段没被覆盖，残留在 TUI 文字右侧，显示错乱：

   ```
   ❯ cd da zi ce shi|esk-ThinkPad-L14-Gen-2 ~/D/w/r/rsime (tui)> cd rsime
   ```

2. **看不到真实 prompt**：用 `❯` 替换后，用户在 TUI 里看不到自己的真实 prompt（主机名、cwd、git 分支等），
   上下文丢失，体验不像原生输入法。

## 目标

把 preedit 真正插入到 shell 的**真实 prompt** 里（保留 prompt 颜色），同时根治残影问题。
覆盖 bash、zsh、fish 三种 shell。

## 非目标（YAGNI）

- 多行命令（命令本身含换行、`\` 续行）下光标不在 prompt 最后一行的场景——按已知限制处理，不专门支持。
- 异步 prompt 段（powerlevel10k 等在捕获后还会更新的段）与屏幕略有出入——接受小瑕疵。
- zsh 原生 `zle -F` + `POSTDISPLAY` 集成（"路线 B"）——zsh 专属且 bash 无法覆盖，留作未来可选增强。

## 总体方案

shell 的快捷键绑定把**渲染好的 prompt 字符串**（带 ANSI 颜色码）通过新环境变量
`RSIME_PROMPT` 传给 `rsime tui`。rsime 用 `ansi-to-tui` 把该字符串解析成 ratatui 的
带样式 `Text`，取最后一行的 spans 作为 TUI 第一行的前缀，再拼接命令与 preedit，
继续用 ratatui 渲染。prompt 颜色得以保留，且因为第一行长度 ≥ 原 prompt，残影根治。

## 环境变量契约

绑定共传递三个环境变量：

| 变量 | 含义 | bash | zsh | fish |
|------|------|------|-----|------|
| `RSIME_PROMPT` | 渲染后 prompt（含 ANSI 颜色码） | `${PS1@P}` | `${(%)PROMPT}` | `(fish_prompt)` |
| `RSIME_READLINE_LINE` | 命令行全文 | `$READLINE_LINE` | `$BUFFER` | `(commandline)` |
| `RSIME_READLINE_POINT` | 光标字符偏移（0-based） | `$READLINE_POINT` | `$CURSOR` | `(commandline --cursor)` |

各 shell 获取渲染 prompt 的机制均已实测可用：

- bash `${PS1@P}`（bash ≥ 4.4）：展开 `\u`/`\h`/`\w` 及 `\[ \e[32m \]` 等转义，输出带 SGR 的字符串。
- zsh `${(%)PROMPT}`：展开 `%F{green}`/`%n`/`%~` 等 prompt 码。
- fish `(fish_prompt)`：运行 prompt 函数并捕获输出（含颜色码）。

## rsime 改动

### 依赖

`rsime/Cargo.toml` 在 `cli` feature 下新增：

```toml
ansi-to-tui = { version = "8.0", optional = true }
```

并在 `cli` feature 列表里加入 `ansi-to-tui`。`ansi-to-tui` 8.0.x 对应 ratatui 0.30（项目当前 0.30.1）。

### 读取 prompt

扩展 `read_shell_context()`（或新增 `read_shell_prompt()`）：读取 `RSIME_PROMPT` 环境变量。
`run_tui` 在进入 `tui_loop` **之前**（prompt 静态、无需每帧解析）将其解析为 ratatui `Text`：

- 用 `ansi-to-tui` 把 `RSIME_PROMPT` 解析为 `Text<'static>`。
- 取 `text.lines` 的最后一个**非空**行（处理 fish prompt 尾部换行）的 `spans`，作为 TUI 第一行前缀。
- 解析失败时回退：把原始字符串当作 `Span::raw` 处理（退到无颜色 prompt）。
- `RSIME_PROMPT` 缺失时：保留现有 `❯` 行行为（向后兼容旧绑定）。

### 渲染

`tui_loop` 的 draw 闭包 shell 分支改为：

```
第一行 = Line::from([
    ...prompt 最后一行的 spans,   // 保留 prompt 颜色
    Span::raw(cmd[..point]),       // 命令前半（默认样式）
    Span::styled(preedit[..cursor_pos], preedit 样式),  // preedit 前段
    Span::raw("|"),                                      // composition 光标标记（保留，沿用现有行为）
    Span::styled(preedit[cursor_pos..], preedit 样式),   // preedit 后段
    Span::raw(cmd[point..]),       // 命令后半
])
```

- preedit 样式：**黄色 + 下划线**（`Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED)`），
  类似原生 IME 的"未提交"提示。可调。
- 命令部分用默认样式（不再 dim，与带色 prompt 搭配更自然）。
- 视口继续 `Viewport::Inline(2)`。
- 候选词仍渲染在 `area.y + 1`。

### 多行 prompt

光标必在 prompt 最后一行；`Inline(2)` 视口从光标行往下铺。因此 prompt 上方各行在视口之外，
ratatui 不会触碰，屏幕上原样保留。**只需重画最后一行**，视口无需扩大。

### shell-init 绑定（`print_shell_bind`）

三 shell 的 widget 均加上 `RSIME_PROMPT`：

- bash：
  ```bash
  output=$(RSIME_PROMPT="${PS1@P}" RSIME_READLINE_LINE="$READLINE_LINE" \
           RSIME_READLINE_POINT="$READLINE_POINT" rsime tui)
  ```
- zsh：
  ```zsh
  output=$(RSIME_PROMPT="${(%)PROMPT}" RSIME_READLINE_LINE="$BUFFER" \
           RSIME_READLINE_POINT="$CURSOR" rsime tui)
  ```
- fish：
  ```fish
  bind {key} 'RSIME_PROMPT=(fish_prompt) RSIME_READLINE_LINE=(commandline) \
  RSIME_READLINE_POINT=(commandline --cursor) rsime tui | read -l output; \
  and commandline --insert "$output"'
  ```

### bash 版本检查

`${PS1@P}` 需要 bash ≥ 4.4。`shell_init_cmd()` 中现有的 bash major < 4 报错改为 < 4.4
（系统自带 bash 3.2 的 macOS 本就被排除，主流 Linux 为 5.x，影响小）。

## 残影根治原理

原残影源于 rsime 第一行短于 shell prompt。新方案第一行 = 真实 prompt + 命令 + preedit，
长度恒 ≥ 原 prompt 行，把原行整行覆盖，不再有未覆盖的残影区域。即使 preedit 为空，
第一行 = 真实 prompt + 命令，与原行内容一致、长度覆盖，首帧由 ratatui 写入文本格子，
等同覆写，无残影。

## 兜底与边界

- `RSIME_PROMPT` 缺失 → 回退现有 `❯` 行（向后兼容）。
- ANSI 解析失败 → 把原始串当 `Span::raw`（无颜色 prompt），仍可用。
- fish prompt 尾部换行 → 取最后非空行。
- prompt+命令超宽 → ratatui 在 1 行高 rect 内自然截断，不换行。
- 异步 prompt 段捕获时与屏幕略有出入 → 接受。

## 测试

- **纯函数单测**：把"解析 prompt → 取最后非空行 spans → 与命令/preedit 拼装成 `Line`"
  抽成无副作用的纯函数，覆盖：单行 prompt、多行 prompt（取最后行）、空 prompt（回退）、
  含 ANSI 的 prompt（颜色保留）、解析失败回退。
- **shell-init 输出**：`rsime shell-init {bash,zsh,fish} --bind` 输出含 `RSIME_PROMPT`
  赋值；zsh 输出 `zsh -n` 语法通过。
- **bash 版本检查**：构造 bash < 4.4 场景验证报错。
- **手测**：三 shell 实际绑定按键，确认 prompt 颜色保留 + preedit 内联 + 无残影 + 退出后命令正确插入。

## 实现拆分（供 writing-plans 参考）

1. 加 `ansi-to-tui` 依赖。
2. 扩展 prompt 读取 + 解析（纯函数 + 单测）。
3. 改 `tui_loop` draw 闭包 shell 分支渲染逻辑。
4. 更新 `print_shell_bind` 三 shell 绑定 + bash 4.4 检查。
5. 验证（单测 + shell-init 输出 + 手测）。
