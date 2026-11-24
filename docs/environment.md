# 环境快照（2026-11-24）

> 所有绝对路径、版本、网络限制集中在此。**接手 AI 不要凭记忆猜路径，以本文件为准**。
> 环境变化时请更新本文件并同步 `HANDOVER.md §2` / `docs/workflows/01-build-and-run.md`。

## 1. 开发机

| 项 | 值 |
|---|---|
| OS | Windows 11 x64 |
| Shell | Git Bash（POSIX sh）/ Windows PowerShell 5.1 |
| 工作区根 | `C:\Users\romanovsky\WorkBuddy\2026-11-23-13-38-07` |
| 用户目录 | `C:\Users\romanovsky` |
| CPU | 16 逻辑核 |
| GPU | **1 张 NVIDIA GTX 1050 / 2 GB 显存**（算力 6.1），无第二张卡 |
| 语言环境 | 简中 Windows，控制台默认 OEM(936/GBK) → CLI 必须主动设 UTF-8 |

## 2. 工具链绝对路径

| 工具 | 路径 | 备注 |
|---|---|---|
| dotnet | `C:\Program Files\dotnet\dotnet.exe` | **不在 PATH**，一律用绝对路径；`--version` = `10.0.404` |
| SDK 列表 | `C:\Program Files\dotnet\sdk\10.0.404` | |
| ffmpeg / ffprobe | `C:\Users\romanovsky\AppData\Local\Microsoft\WinGet\Packages\Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe\ffmpeg-9.1-full_build\bin\` | 刚装完 PATH 未刷新时靠 `ResolveFfmpeg` 目录扫描兜底 |
| Python（sidecar venv） | `engine\.venv\Scripts\python.exe` | Python 3.13.14；`sherpa-onnx 2.0.3` + `sherpa-onnx-core 2.0.3` |
| git | Git for Windows 2.55.0.windows.3 | |
| gh CLI | **未安装** | |
| SSH 配置 | `C:\Users\romanovsky\.ssh\config` | `Host github.com` → `IdentityFile ~/.ssh/mainstationkey_ed25519`，`IdentitiesOnly yes` |
| 目标仓库 | `git@github.com:tartan01/epubtts-miniature-parakeet.git` | SSH 认证已验证可达（`git ls-remote` 返回 0） |
| **Rust（mini）** | `.rustup\` + `.cargo\`（工作区内，**均不入库**）；`rustc` = `.cargo\bin\rustc.exe` | `stable-x86_64-pc-windows-gnu`；`rustup-init 1.29.3`；经 `rsproxy.cn` 镜像安装 |
| **MSVC / Windows SDK** | ❌ 本机未安装 | 靠 `mini\.cargo\config.toml` 的 `rust-lld` + `link-self-contained=yes` 自包含链接 |
| 系统 curl / tar / PowerShell | `C:\Windows\System32\curl.exe`、`tar.exe`、`WindowsPowerShell\v1.0\powershell.exe` | Mini 的运行时依赖（下载 / 解 EPUB / SAPI 合成），Windows 自带 |

## 3. 构建产物路径（Debug）

| 入口 | 路径 |
|---|---|
| 解决方案 | `bookctl.sln` |
| WinUI 窗口程序 | `src\BookCtl.App\bin\Debug\net10.0-windows10.0.19041.0\win-x64\BookCtl.App.exe` |
| CLI | `src\BookCtl.Cli\bin\Debug\net10.0\bookctl.exe`（同目录 `bookctl.dll`） |
| MCP | `src\BookCtl.Mcp\bin\Debug\net10.0\bookctl-mcp.exe` |
| Worker | `src\BookCtl.Worker\bin\Debug\net10.0\bookctl-worker.exe` |
| 便携版目录 | `dist\本地朗读工厂-0.3.0-win-x64\` |
| 便携版 zip | `dist\本地朗读工厂-0.3.0-win-x64.zip`（≈196 MB） |
| 便携版 CLI | `dist\本地朗读工厂-0.3.0-win-x64\tools\BookCtl.Cli\bookctl.exe` |
| **Mini 主程序** | `dist\release\mini-portable\BookCtl.Mini.exe`（679,936 B） |
| **阅读器主程序** | `dist\release\reader-portable\BookCtl.Reader.exe`（679,936 B，同一份 exe 改名） |
| **安装器 stub** | `dist\release\setup-stub.exe`（370,688 B） |
| **Mini 一键安装包** | `dist\release\本地朗读工厂-Mini-0.6.0-setup.exe`（761,141 B） |
| **阅读器一键安装包** | `dist\release\本地朗读工厂-阅读器-0.6.0-setup.exe`（720,628 B） |
| **Full 一键安装包** | `dist\release\本地朗读工厂-Full-0.3.0-setup.exe`（206,943,055 B） |
| Mini / 阅读器便携 zip | `dist\release\本地朗读工厂-{Mini,阅读器}-0.6.0-portable.zip`（≈390 / 350 KB） |
| 阅读器朗读缓存 | `%LOCALAPPDATA%\本地朗读工厂\ReaderCache\<hash>\`（不入库，卸载也保留） |

> 生成这些产物需要本机编译；**仓库里不含 bin/ 内容**，`dist/` 与 `dist/release/` 也被 `.gitignore` 排除。
> clone 后按 `docs/workflows/01-build-and-run.md` 重建 .NET 主线，
> 按 `packaging/install-toolchain.cmd` → `mini/build.ps1` → `packaging/make-installer.ps1` 重建 Mini。
> 跨机测试只需拷需要的 `*-setup.exe`（三个包可同时安装，互不覆盖）。

## 3.1 安装后的用户目录（v0.5）

| 项 | 路径 |
|---|---|
| Mini 程序目录 | `%LOCALAPPDATA%\本地朗读工厂\Mini\` |
| Full 程序目录 | `%LOCALAPPDATA%\本地朗读工厂\Full\` |
| Mini 配置 | `%LOCALAPPDATA%\本地朗读工厂\config.json`（`BookCtl.Mini.exe config path`） |
| 模型根（用户选盘） | 例 `D:\本地朗读工厂模型`（默认 `C:\本地朗读工厂模型`） |
| 注册表产品信息 | `HKCU\Software\本地朗读工厂\<Mini\|Full>` |
| 卸载项 | `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\本地朗读工厂 <Mini\|Full>` |
| 卸载程序 | 安装目录下 `Uninstall-Mini.exe` / `Uninstall-Full.exe` |

## 4. 运行时数据目录（仓库外）

| 用途 | 路径 |
|---|---|
| 默认 workspace | `%LOCALAPPDATA%\bookctl\workspace` |
| 默认数据库 | `<workspace>\bookctl.db`（WAL 模式，另有 `-wal` / `-shm`） |
| 默认模型源 | `%LOCALAPPDATA%\bookctl\sources` |
| 已装中文模型 | `%LOCALAPPDATA%\bookctl\sources\sherpa-vits-zh-fanchen`（约 132 MB，含 `.installed.json`） |
| 便携版覆盖 | `<程序目录>\data` + `<程序目录>\sources` |
| TTS sidecar | `http://127.0.0.1:8765`（`--mode sherpa --device cpu`） |

模型文件 `vits-zh-hf-fanchen-C.onnx` / `tokens.txt` / `lexicon.txt` / `*.fst` / `dict/` 都在这里，
**不入库**（体积 + 许可/分发考虑）。clone 后需 `bookctl source pull sherpa-vits-zh-fanchen` 重新下载。

## 5. 关键版本

| 组件 | 版本 |
|---|---|
| .NET SDK | 10.0.404 |
| 目标框架 | `net10.0` / `net10.0-windows10.0.19041.0` |
| Windows App SDK | 2.7.0（`Directory.Build.props` 统一） |
| RID | `win-x64` |
| Python | 3.13.14 |
| sherpa-onnx | 2.0.3 |
| ffmpeg | 9.1（Gyan full build） |
| 产品版本 | 0.3.0（`edition.json` 里 `pro-portable`） |

## 6. 网络限制（很重要）

- **国内直连 GitHub Release 返回 502**（`CONNECT tunnel failed, response 502`）。
- `https://gh-proxy.com/` 前缀可用（HTTP 200）；`ghfast.top` 返回 403。
- HuggingFace 用 `https://hf-mirror.com` 替换。
- 因此 `SourceManager.Candidates(...)` 实现了多线路回退：
  原始 URL → 用户 `mirrors` → `BOOKCTL_MIRROR` → `gh-proxy.com` / `hf-mirror.com`。
- 实测 `source pull sherpa-vits-zh-fanchen` 耗时约 **26 分 39 秒**；下载期间会锁 CLI 的 DLL，不要同时编译。

## 7. 安全 / 策略限制（本机 WorkBuddy 沙箱）

- 批量删除 >1000 文件的目录会被拦截 → 打包脚本不要先删 `dist/`，改原地增量覆盖。
- `Add-Type` / `Reflection.Assembly` 截图方式被拦截 → 用 ffmpeg `gdigrab`。
- 后台进程会随 shell 结束被回收 → sidecar 与调用要在同一条命令内，或用受管后台任务。
- Git Bash `/tmp` 与 Windows Python 的路径语义不同 → 跨语言传路径一律用绝对 Windows 路径。
- `ffmpeg -f gdigrab -i title=<窗口标题>` 抓 WinUI 得到空白（DirectComposition）→ 用 `-i desktop` 全屏，配合 App `--on-top`。

## 8. 测试资产（本机，部分入库）

| 资产 | 位置 | 入库 |
|---|---|---|
| 3 章中文测试 EPUB（2.6 KB） | `testdata\测试之书.epub` | ✅ |
| UI 截图（简洁 / 详细 / 结果卡） | `docs\screenshots\*.png` | ✅ |
| 各轮工作区产物（M4B / WAV / DB / 日志） | `testdata\ws-*` | ❌ |
| 单句真语音试听 | `testdata\sherpa-probe.wav` | ❌ |
| 便携版 zip / 目录 | `dist\` | ❌ |
| Mini / 阅读器 / Full 三个 setup.exe、便携 zip、stub | `dist\release\` | ❌（跨机测试手工拷贝） |
| Rust 工具链 | `.rustup\` / `.cargo\` | ❌（例外：`mini\.cargo\config.toml` ✅） |
| Rust 构建中间产物 | `mini\target\` / `mini\_probe\*.exe` / `mini\*.exe` | ❌ |
| Mini 界面截图（07/08/09） | `docs\screenshots\07~09-*.png` | ✅ |
| 阅读器界面截图（10/11） | `docs\screenshots\10~11-*.png` | ✅ |
| 阅读器缓存 / last.txt | `%LOCALAPPDATA%\本地朗读工厂\ReaderCache\` | ❌（用户数据，不在仓库内） |
| Python venv | `engine\.venv\` | ❌ |
| 原始会话 jsonl | `C:\Users\romanovsky\.workbuddy\projects\...\08257854-....jsonl` | ❌（含宿主运行时上下文） |

## 9. 交接仓库

```
git@github.com:tartan01/epubtts-miniature-parakeet.git
```
首次提交包含：源码、脚本、配置、总架构 MD/HTML、README、
`HANDOVER.md`、`AGENTS.md`、`docs/` 全套交接文档、`tools/export_session.py`、测试 EPUB、3 张截图。
不含任何编译产物、模型、密钥、大音频。
