# rsime

命令行中文输入工具，基于 RIME，面向无图形输入法的 TUI 环境。

## 构建与开发

```bash
make          # release 构建
make debug    # debug 构建
make test     # 运行测试
make clippy   # lint 检查
make clean    # 清理构建产物
make install  # 安装
```

构建依赖 vcpkg 提供 `rime` 库。`make` 自动调用 `vcpkg install`，需要
`VCPKG_ROOT` 环境变量已设置。构建通过 `RIME_INCLUDE_DIR` / `RIME_LIB_DIR`
环境变量指向 vcpkg 安装的头文件和库文件。

## 项目结构

```
src/main.rs              — 主程序，所有逻辑在单文件中
tests/stdio.rs           — stdio 模式的集成测试
third_party/librime-rs/  — fork 的 librime Rust 绑定 (rime-api / librime-sys)
third_party/librime/     — 作为参考的 librime C++ 源码 (git submodule)
third_party/plum/        — 作为参考的 rime-plum 方案安装脚本 (git submodule)
misc/vcpkg-ports/rime/   — 自定义 vcpkg port，构建 librime + 插件
scripts/ls-todo.sh       — 列出项目中的 TODO/FIME 项
```

## 架构

CLI 使用 `clap` derive 模式定义子命令：

- `tui` — 交互式 TUI，使用 ratatui + crossterm，通过 `/dev/tty` 读写终端
- `stdio` — 编辑器集成模式，Vim 风格按键输入，JSONL 输出
- `install` — 在线安装 RIME 输入方案（下载 plum 脚本并通过 bash 执行）
- `list-schemas` / `current-schema` / `set-schema` — 方案管理
- `shell-init` — 输出 shell 补全脚本和可选的快捷键绑定

RIME 交互通过 `rime-api` crate（fork 的 `third_party/librime-rs`）。按键码使用
`rime_api::KEY_*` 常量（从 `librime-sys` 的 `RimeKeyCode_XK_*` 重新导出），
不要使用硬编码数字。

`init_rime()` 中 shared_data_dir 和 user_data_dir **故意设为同一目录**，
因为本项目通常不依赖系统级 RIME 安装。留空会导致 RIME 回退到当前工作目录。

## 测试

测试通过 `assert_cmd` 以子进程方式运行 `rsime stdio`。`setup_rime_env()` 将
`~/.config/rsime` 复制到临时目录并设为 `RIME_USER_DATA_DIR`，确保测试有可用的
输入方案。

运行测试前需要先完成 debug 构建（`make test` 会自动处理）。

## 注意事项

- `run_tui` 中使用 `libc::dup/dup2` 重定向 stdout 到 `/dev/tty`，以便在 `$()`
  子 shell 中工作时 crossterm 的光标查询能到达终端
- `install_cmd` 通过 HTTP 下载 plum 脚本并 pipe 给 bash，需要网络和 git
- 许可证：代码 GPL-3.0-or-later，文档/配置/脚本 MIT。遵循 REUSE 规范
- commit 风格参考 `git log`：`feat:`, `build(vcpkg):` 等常规前缀
