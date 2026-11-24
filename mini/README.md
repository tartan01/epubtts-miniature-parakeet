# mini/ · 本地朗读工厂 Mini（轻量入口）

> Rust / Win32 原生 / **零外部 crate** / 零 .NET / 零 Python / 零模型权重。
> 单 exe `BookCtl.Mini.exe` ≈603 KB。总架构见仓库根 `architecture-epub2audiobook.md` **附录 C**，
> 用户向说明见 `README.md` **§10**，封装细节见 `docs/workflows/06-packaging.md` §8。

## 这是什么

Mini 不是「功能更少的主线」，而是**轻量入口 + 模型自动适配/下载/选址管理器**：
用户不用自己找镜像、不用拼路径。检测到重量级主线（Full）就通过 CLI 契约委托真 TTS 模型；
没装 Full 就用 Windows 自带 SAPI 语音兜底；两者都不可用则生成明确标注的占位音。

| 模块 | 职责 |
|---|---|
| `src/main.rs` | GUI（默认）/ CLI / MCP 三入口分发；`capabilities / doctor / device / source / config / run / help` |
| `src/win32.rs` | 手写 Win32 FFI（kernel32 / user32 / comctl32 / comdlg32 / shell32 / gdi32 / advapi32） |
| `src/ui.rs` | 原生窗口与控件、`WM_TIMER` 快照刷新、拖放 `.epub` / `config.json`、进度条、日志 |
| `src/model.rs` | 运行配置 + 内置 8 下载源 + 4 模型 + 自动测速 / 多源下载 / 失败切源 / 选盘安装 |
| `src/audio.rs` | EPUB（系统 `tar.exe`）→ OPF/spine/NCX → 中文分句 → SAPI → 分章 WAV；`delegate_full()` 委托 Full CLI |
| `src/util.rs` | 手写极简 JSON 解析/序列化 + JSON envelope（不引入 serde） |
| `setup/main.rs` | 自解压安装器 stub：PE overlay payload → HKCU 注册表 → 快捷方式 → 两阶段卸载 |
| `_probe/probe.rs` | 换机首次构建时验证 Win32 可链接的最小探针 |
| `.cargo/config.toml` | `rust-lld` + `link-self-contained=yes`，免 MSVC / Windows SDK |
| `build.ps1` | 编译 `BookCtl.Mini.exe` + `setup-stub.exe` |

## 构建（换机）

```powershell
..\packaging\install-toolchain.cmd   # 首次：装 Rust GNU 工具链（rsproxy.cn 镜像）
.\mini\build.ps1                     # 从仓库根运行：mini\build.ps1
..\packaging\make-installer.ps1      # 产出两个 setup.exe
```

本机没有 MSVC / Windows SDK，`rust-lld` 自包含链接，产物不依赖 VC++ Redistributable。
`Cargo.toml` 的 bin 名是 `bookctl_mini`（crate 名不允许 `.`），**实际 exe 名由 `build.ps1`
用 `rustc -o BookCtl.Mini.exe` 指定**，不要直接用 `cargo build` 产出的名字去打包。

## 契约（与 .NET 主线一致）

所有 CLI 输出为 JSON envelope：`{"ok":true,"api_version":"1.0","data":...}` /
`{"ok":false,"api_version":"1.0","error":{...}}`；MCP 走 stdio JSON-RPC
（`initialize` → `2025-06-18`，6 个 tools）。**改命令/tool 名要同步 `capabilities_data()`**。

```powershell
BookCtl.Mini.exe capabilities
BookCtl.Mini.exe device list
BookCtl.Mini.exe source list
BookCtl.Mini.exe source probe sherpa-vits-zh-fanchen
BookCtl.Mini.exe source pull sherpa-vits-zh-fanchen --root D:\本地朗读工厂模型
BookCtl.Mini.exe config show | import <file> | set <k> <v> | path
BookCtl.Mini.exe run --input 书.epub --device auto [--dry-run] [--engine mock]
BookCtl.Mini.exe mcp
BookCtl.Mini.exe doctor
```

## 已知边界

- Mini **自身不加载任意本地模型权重**；模型已下载但 Full 未安装时仍走 SAPI（见 `docs/roadmap.md` R-14）。
- PowerShell 5.1 会把子进程 stderr 按 GBK 解码 → 中文日志显示乱码。
  先 `chcp 65001` 或 `[Console]::OutputEncoding = [Text.Encoding]::UTF8`。
  **字节本身是干净 UTF-8**，不要改程序输出编码（会破坏 CLI/MCP 契约）。
- GUI 是 Win32 原生控件，视觉不如 WinUI 主线精致（R-15），但功能与契约完整。
