# 本地朗读工厂 v0.6.0 · 直接下载即用

**这是发行版页面**：下面的 `setup.exe` 双击（或 `setup.exe /S` 静默）即可安装，
**不需要** .NET、Python、管理员权限，也不需要本仓库的源码。
源码与全部历史版本在仓库里（`main` 分支 + `v0.3.0` / `v0.4.0` / `v0.5.0` / `v0.6.0` tags）。

## 该下哪一个

| 下载 | 版本 | 用途 | 体积 |
|---|---|---|---|
| `本地朗读工厂-阅读器-0.6.0-setup.exe` | 0.6.0 | **边看 EPUB 文字、边听当前段落朗读**，后台自动预生成后面章节；双击启动即进阅读器 | 720 KB |
| `本地朗读工厂-Mini-0.6.0-setup.exe` | 0.6.0 | 批量模式：一键把整本 EPUB 生成有声书（零依赖轻量入口），附 CLI 与 MCP | 761 KB |
| `本地朗读工厂-Full-0.3.0-setup.exe` | 0.3.0 | 重量级主线（.NET / WinUI 3 / Python sidecar），模型音质整本导出 | 197 MB |
| `本地朗读工厂-Mini-0.6.0-portable.zip` | 0.6.0 | Mini 免安装便携版，解压即用 | 390 KB |
| `本地朗读工厂-阅读器-0.6.0-portable.zip` | 0.6.0 | 阅读器免安装便携版，解压即用 | 350 KB |

> 三个安装版**可以同时安装、互不覆盖**。想要阅读器里「整本导出…」出模型音质，
> 建议先装 Full，再装阅读器 / Mini。
> 阅读器版主程序名为 `BookCtl.Reader.exe`，双击即进阅读器，并会记住上次打开的书。

## 系统要求

- Windows 10 / 11（x64）
- 仅用系统自带组件：`tar.exe`、`PowerShell System.Speech`、`winmm.dll`
- 安装位置：`%LOCALAPPDATA%\本地朗读工厂\<Mini|Reader|Full>`；卸载走系统「程序和功能」或安装目录内 `Uninstall-<Edition>.exe`
- 阅读器朗读缓存在 `%LOCALAPPDATA%\本地朗读工厂\ReaderCache\`，**卸载不删除**

## 阅读器新增（v0.6）

- 三栏界面：章节列表 / 正文（当前朗读段落高亮）/ 段落导航
- 后台只预生成「当前位置往后 6 段 + 下一章前 2 段」——机器合成一段约 0.6 秒，
  人读一段要几秒，所以语音始终跑在阅读进度前面，不用等整本合成完
- 预取窗口可调 2/4/6/10/16 段；语速 0.5×–2.0×（换语速用独立缓存）；跨章自动衔接
- 点段落即跳、朗读立即从新位置续上；「跟随：关」可自由往后翻；「重合成本段」可重试失败段
- 命令行自检：`reader --dry-run` / `--headless --count 5` / `status` / `--play-test`

## 校验（SHA-256）

```
1301509054e984f7d80e9d4ecb380c2307ec3345ad378a96e65483e4d8da209e  本地朗读工厂-Mini-0.6.0-setup.exe
e3e24a281d871ce2142688b54443b50984dda961a4e2ce257873b2141e613202  本地朗读工厂-阅读器-0.6.0-setup.exe
89bf512b092400feb028febfd43b05a32c9ce812adc6371ad59b6f614ecfe1d7  本地朗读工厂-Full-0.3.0-setup.exe
ce74b6aabcbdb761db5e410ef04c0f3a86eab07c550888becc587ce774d3db9c  本地朗读工厂-Mini-0.6.0-portable.zip
bd7a7494642ab51e1477358d2213275224534598c9f694f53947e2ec9cd6381d  本地朗读工厂-阅读器-0.6.0-portable.zip
```

## 已知边界

- 阅读器朗读用系统 SAPI 语音（即时、音质一般）；模型音质请用「整本导出…」或 Mini/Full 批量模式
- 正文只渲染纯文本，EPUB 内的图片 / 脚注 / 表格暂不显示；同步粒度是段落，无逐字高亮
- 无音频设备的环境里 `--play-test` 返回 `playback_failed`，不影响预生成
- 未做代码签名（`docs/roadmap.md` R-01，缺证书），SmartScreen 可能提示「未知发布者」

## 源码 / 接手

- 仓库：`git@github.com:tartan01/epubtts-miniature-parakeet.git`
- 接手入口：`README.md` §0 → `HANDOVER.md` → `AGENTS.md` → `docs/README.md`
- 设计事实源：`architecture-epub2audiobook.md` 附录 C（v0.5 Mini）/ 附录 D（v0.6 阅读器）
- 从源码重建：`mini\build.ps1` → `packaging\make-installer.ps1` → `packaging\publish-release.ps1`
