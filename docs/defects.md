# 已修复的真实缺陷台账（12 条）

> 全部来自本期**实际编译 / 运行 / 端到端验收**，不是推测。
> 与 `README.md §7.1` 同步。修新 bug 时请追加编号并写明「现象 → 根因 → 修法 → 验证」。

---

## #1 · WinUI `CS0103: DataContext` 不存在

- **现象**：首次编译 `BookCtl.App` 失败。
- **根因**：WinUI 3 的 `Window` 不是 WPF 的 `FrameworkElement`，没有 `DataContext`。
- **修法**：`MainWindow.xaml` 根 Grid 加 `x:Name="Root"`；`MainWindow.xaml.cs` 改 `Root.DataContext = _vm;`。
- **验证**：全解决方案 0 error，App 实际启动无 XAML 崩溃。
- **注意**：用户明确要 WinUI 原生，不要因此改 WPF。

## #2 · `WAV_INVALID 非 WAV 文件`

- **现象**：第 1 章合成完成后拼接阶段报错。
- **根因**：`MasterChapter(...)` 在有 ffmpeg 时返回 `audio.m4a`，Pipeline 却用只认 RIFF 的 `Wav.DurationOf()` 读它。
- **修法**：新增 `AudioMaster.ProbeDuration(path)`：WAV 读头 → 非 WAV 用 `ffprobe` → 兜底按 AAC 64 kbps 估算。
- **验证**：3 章全部完成，`book.m4b` 生成。

## #3 · 整本 M4B 阶段卡死 3 分钟以上 / `moov atom not found`

- **现象**：ffmpeg 进程存在但不退出；手动杀掉后 `book.m4b` 无效。
- **根因**：`AudioMaster.Run()` 重定向 stdout/stderr 却从不读取，ffmpeg 输出写满 4 KB 管道后死锁。
- **修法**：并发 `ReadToEndAsync()`；加 `-hide_banner -loglevel error -nostats`；
  超时 `Kill(entireProcessTree: true)`；`concat_list.txt` 路径统一正斜杠并转义单引号。
- **验证**：重新跑 4 秒完成，`ffprobe` 确认时长与章节标记正确。

## #4 · 进程被强杀后任务永久停在 `running`

- **现象**：UI 永远转圈，DB 状态 `running`。
- **根因**：没有执行权 lease / 心跳 / 孤儿回收。
- **修法**：`worker.lock` 存 PID；`WorkerHost.Start()` 调 `RecoverOrphans()`；
  `JobStore.FindRunningJobIds()` / `FailIfStillRunning()` CAS 把残留标为 `WORKER_LOST`，可 `job resume`。

## #5 · resume 后 `chapters_done=6 / total=3`、`chars_done > chars_total`；完成态仍挂 `WORKER_LOST`

- **根因**：resume 时在上次计数上继续累加；done 章节再次 `++`；完成时未清空错误码。
- **修法**：按章节 checkpoint **重算** `ChaptersDone / CharsDone / AudioSeconds`；
  done 章节不再 `++`；完成时 `job.ErrorCode = null; job.ErrorMessage = null;`。
- **验证**：干净工作区跑完显示 `3/3`、`347/352`，无旧错误码。

## #6 · CPU 推理却显示「正在调用显卡进行 TTS 合成」

- **现象**：sherpa + `--device cpu` 跑整本，中控台 `phase` 仍写"正在调用显卡"。
- **根因**：`segment_done` 的 `phase` 是硬编码字符串。
- **修法**：按 `DevicePlan.Device` 分档输出 GPU / GPU+CPU 层卸载 / `正在用 CPU（N 线程）进行 TTS 合成`，
  并带上 `device` / `device_display`。
- **验证**：`job logs` 里 `phase = 正在用 CPU（16 线程）进行 TTS 合成`，`device=cpu`、display 一致。

## #7 · CLI 事件流 JSONL 中文乱码（跨终端不一致）

- **现象**：UTF-8 终端下事件流中文乱码；PowerShell 5.1 重定向日志时又反过来。
- **根因**：只有 `Emit()` 才设 `Console.OutputEncoding = UTF8`，
  **事件流 JSONL 在那之前就已按 OEM(936) 输出**。
- **修法**：`Main()` 开头统一 stdout/stderr 为无 BOM UTF-8，含 `Console.SetError(...)` 重建，
  保证纯文本进度行也一致；移除 `Emit()/EmitError()` 里的临时切换。
- **验证**：`job logs` 与 `run` 实时事件流均严格 UTF-8 无 BOM 解码成功，
  31 行纯文本 + 32 行 JSON 中文全部正常。

## #8 · 便携版数据仍写 `%LOCALAPPDATA%`，U 盘带走会丢

- **现象**：`dist\...\tools\BookCtl.Cli\bookctl.exe doctor` 的 workspace 仍是用户级目录，`edition` 不是 pro。
- **根因**：三个工具发布在 `tools\<proj>\` 子目录，而 `portable.marker` 只在程序根目录；
  `ApplyPortable()` 只查 `AppContext.BaseDirectory`。
- **修法**：从 `BaseDirectory` **逐级向上（≤6 层）** 查找 marker，以 marker 所在目录为便携根。
- **验证**：便携版 `workspace = dist\...\data`、`database = ...\data\bookctl.db`、`edition = pro`。

## #9 · 改工作区后已装模型「消失」，简洁模式静默退回占位音

- **现象**：UI 截图底部显示 `模型源：C:\...\testdata\sources`，manifest `engine=mock`，音频只有 10 秒。
- **根因**：`SourcesRoot` 由 `WorkspaceRoot` 的父目录推导，工作区一动源目录跟着漂移。
- **修法**：`SourcesRoot` 固定 `%LOCALAPPDATA%\bookctl\sources`（便携版由 marker 覆盖）。
- **验证**：任意 `BOOKCTL_WORKSPACE` 下 `source list` 都能找到已安装的 fanchen 模型。

## #10 · GUI 选 sherpa，`manifest` 记 `engine=sherpa`，实际出声却是 mock（0.4 秒/段）★ 最隐蔽

- **现象**：`manifest.json` 写 `engine=sherpa`，但分段 WAV 每段只有 0.4 秒、总时长 10.1 秒。
- **根因**：`Pipeline.cs` 里
  ```csharp
  var provider = job.Engine is null || job.Engine == _cfg.EngineMode ? _tts : TtsProviderFactory.Create(cfg);
  ```
  `_tts` 是**构造时**按当时 `EngineMode`（mock）固化的 provider，而比较用的是**运行时被改过**的
  `_cfg.EngineMode`。App 的 `StartOneClick()` 把它改成 sherpa 后两者恒等 → 复用旧 mock provider。
  CLI 因为显式传 `--engine` 才侥幸正确。
- **修法**：记录 `_ttsEngine`（provider 创建时的引擎名）再比较；
  `job.Engine` 与 `_ttsEngine` 不一致时按 `job.Engine` 新建 provider。
- **验证**：重跑 UI 得到 `engine=sherpa` / 97.9 秒音频 / 34.2 秒合成 / 分段 6.71、7.57、4.67 秒 @16 kHz。

## #11 · `build-portable.ps1` 直接语法崩溃 `意外的标记":"`

- **现象**：PowerShell 5.1 执行脚本直接解析失败，便携版发不出来。
- **根因**：脚本是 UTF-8 **无 BOM**，5.1 按 GBK 解码，中文把 here-string 语法破坏。
- **修法**：三个打包脚本（`.ps1` / `.iss`）统一加 UTF-8 BOM；
  `edition.json` 反过来用 `WriteAllText` + `UTF8Encoding(false)` 写**无 BOM**；
  `engine/` 拷贝跳过 `.venv` / `__pycache__`（避免先拷 65 MB 再删）。
- **验证**：`build-portable.ps1 -Configuration Release -Version 0.3.0` 成功产出 ≈196 MB zip。

## #12 · 产物 `manifest.json` 的 `status` 永远是 `running`

- **现象**：DB 里 job 已 `completed`，但用户检验产物时看到的 `manifest.json` 仍写 `running`。
- **根因**：manifest 在 job 终态落定**之前**写出。
- **修法**：先更新 job 终态并 `UpdateJob()`，再写 `manifest.json` 与 `qc/report.json`。
- **验证**：`manifest.status = completed`。

---

## 附带的稳健性修复（未单独编号，已在 README §7.1 记录）

- `run` 命中 `failed` / `cancelled` 的幂等旧任务时自动 `JobResume`。
- `TextNormalizer` 可能返回空白段 → `if (string.IsNullOrWhiteSpace(s)) continue;`（不再生成 0 字 WAV）。
- `ResolveFfmpeg()` 增加 WinGet Packages 目录扫描兜底（刚装完 ffmpeg、PATH 未刷新时也能找到）。
- `JobStore.P(...)` 参数改 `object?`，消除 nullable warning。
- MCP `PullSource` 改 `static`（`BookCtl.Mcp.Program` 是 static class，曾报 `CS0708`）。
- CLI `source pull` 的 progress 回调签名对齐（`Action<string,double,string>` vs `Action<double,string>`）。
- `SourceManager` 解包到 `tmpDir/extract` 并 `File.Delete(archive)`，避免 119 MB 归档残留 + 目录嵌套。
- 测试 EPUB 用 Python `zipfile` 生成，`mimetype` 第一个且不压缩、路径统一 `/`。

---

## 工程坑（不是产品缺陷，但会浪费大量时间）

| 坑 | 处理 |
|---|---|
| `dotnet build bookctl.sln` 后 `App\bin\...\BookCtl.Core.dll` 可能是旧版 | 改 Core 后**单独重建 App 项目**，先关掉运行中的 App 释放 DLL |
| 长时间 `source pull` 会锁 CLI 的 DLL，导致全解决方案 build 失败 | 等下载完成再编译 |
| 打包脚本清空 `dist/`（1100+ 文件）被安全策略拦截 | 原地增量覆盖 + `Compress-Archive -Force` |
| WinUI 内容用 `gdigrab -i title=` 抓到空白（DirectComposition） | 用 `-i desktop` 全屏抓，配合 App `--on-top` |
| sidecar 后台进程被 shell 回收 / Git Bash `/tmp` 与 Windows Python 路径不一致 | 同一条 shell 命令内启动 + curl + 分析 + 结束；用绝对路径 |
| 截图工具 `Add-Type` / `Reflection.Assembly` 被安全策略拦截 | 改用 ffmpeg `gdigrab` |

---

## 2026-11-24 晚 · 结构 / 交互 / 分发优化（v0.4 增量）

这一批不是"跑不通"，而是"能跑但难用、难找、太重"。逐条记录现象与处理，避免下一位 AI 重复判断。

| # | 现象 | 根因 | 处理 |
|---|---|---|---|
| 13 | 用户"找不到 .exe 启动文件" | 便携包 1127 个文件平铺，`BookCtl.App.exe` 混在 859 个 dll 里；根目录无入口 | 新增根目录 `启动.cmd`（唯一 GUI 入口，按便携包 → Debug → Release 顺序找 exe，支持透传 `.epub` 与 `--detailed`）；README §2 增加「exe 在哪」表格 |
| 14 | 只能点「浏览…」选书，路径操作繁琐 | 从未实现拖放；文件选择器是唯一导入途径 | WinUI 根容器 `AllowDrop=True` + `DragOver/Drop/DragLeave`；加入全窗口拖放提示层（`IsHitTestVisible=False` 防止事件抖动）；两种模式通用 |
| 15 | 导入逻辑分散在 3 处（选择器 / 命令行 / 文件关联） | 各处直接写 `InputPath = ...`，校验与提示不一致 | 统一 `MainViewModel.ImportEpub(path)`：扩展名、存在性、绝对路径、中控台反馈只有一份实现；失败走 `NotifyImportFailed` 留 WARN，不弹窗 |
| 16 | 免安装包 473 MB（zip 197 MB），"太重" | `tools/` 下 CLI/MCP/Worker 各自 `--self-contained`，同一份 .NET 运行时被复制 3 遍（239 MB）；另带本项目用不到的 `onnxruntime.dll`(23 MB) + `DirectML.dll`(18 MB) | `build-portable.ps1` 新增 `-Slim`：GUI 仍完全自包含，三个工具改 framework-dependent；裁掉未使用 ML 组件；`tools\PREREQUISITE.txt` 写明运行时要求。多语言 MUI 实测仅 3.4 MB，评估后**不裁**（见 #19） |
| 17 | `build-portable.ps1` 直接调用裸 `dotnet`，本机不在 PATH 时打包失败 | 脚本沿用开发机早期 PATH 假设 | 脚本内解析 `C:\Program Files\dotnet\dotnet.exe`，找不到才回退 PATH |
| 18 | `.cmd` 脚本双击执行不稳定（中文乱码/行解析失败） | 批处理内容含中文、且写成 LF 换行；cmd.exe 按 ANSI 代码页解析 | 脚本内容保持纯 ASCII（中文路径用通配符匹配），文件转 CRLF；`*.ps1` 继续保留 UTF-8 BOM（ADR-018） |
| 19 | `-Slim` 打包在删 MUI 时抛 `InputObjectNotBound`，改完又撞宿主 `safe-delete` 阈值（≥50 文件需确认） | ① PS 5.1 的 `Remove-Item` 按属性名绑定，`FileInfo.FullName` 对不上 `-Path`；② 本机宿主把批量删除包装成 safe-delete，累计 50 次即拦截 | 先改 `ForEach-Object { Remove-Item $_.FullName }`，随后实测 MUI 只有 3.4 MB，**直接放弃裁剪**；同时把脚本开头的 `Remove-Item -Recurse $out` 改为增量覆盖，避免重跑时被拦。不绕过安全策略，改从设计上避免批量删除 |
| 20 | **便携包双击没反应**：`BookCtl.App.exe` 启动 1 秒即退出，无窗口无提示；Debug 构建正常 | **`dotnet publish` 没有复制 WinUI 的 XAML 编译产物**：publish 目录缺 `App.xbf`、`MainWindow.xbf` 与 `BookCtl.App.pri`（2.3 MB 资源索引）。缺了它们，`Microsoft.UI.Xaml.dll` 在初始化阶段抛 stowed exception，进程以 `0xC000027B` 退出（事件日志错误模块 = Microsoft.UI.Xaml.dll，`combase.dll` E_FAIL） | 打包脚本 publish 后从 `src\BookCtl.App\bin\$Configuration\net10.0-windows10.0.19041.0\win-x64` 补齐 `*.xbf` + `BookCtl.App.pri`。修复后 slim 包实测 `RUNNING / title=[本地朗读工厂] / responding=True` |
| 21 | 上一轮"便携模式验证 ✅"给错了结论 | 当时只验证了 `portable.marker` 路径解析与 CLI 在包内可执行，**从未真正双击 GUI**（或双击后崩溃被忽略） | 本轮回溯确认：v0.3 的标准便携包同样缺 XAML 资源、同样启动崩溃 —— 即此前交付的免安装包一直是坏的。已重跑标准包与 slim 包并逐个实测启动 |

---

## 2026-11-24 深夜 · v0.5 Mini 产品线（#22–#26）

这一批是新建 `mini/` 产品线过程中实测到的缺陷，含 3 条「看起来是 bug、其实不是」的判断，
写清根因避免接手 AI 走回头路。

| # | 现象 | 根因 | 处理 / 验证 |
|---|---|---|---|
| 22 | 官方 Rust 下载卡 4 分钟；`rustup-init.exe` 报「不是有效 Win32 应用」；`cargo` 编译报 `invalid character '.' in crate name`；链接报 `undefined symbol: GetLogicalDrivesStringsW` | ① 国内直连 static.rust-lang.org 慢；② 首次下载被中止留下 4.28 MB 残文件（完整应为 14,862,151 B）；③ crate/bin 名不允许 `.`；④ Win32 API 正确名是 `GetLogicalDriveStringsW`（Drive 单数） | 工具链改 `rsproxy.cn` 镜像（<1s）、重下完整 rustup-init；Cargo.toml 的 bin 名改 `bookctl_mini`，实际 exe 由 `build.ps1` 用 `rustc -o BookCtl.Mini.exe` 指定；API 名全局改正。probe.exe 872,960 B 跑通 `win32-link-ok screen=1920x1080 coinit_hr=0` |
| 23 | Mini GUI 日志全挤成一行；EPUB 书名显示成 `《>测试之书》`；CLI 合成进度显示 `5 B / 14 B` | ① Windows Edit 控件换行必须 `\r\n`，只写 `\n` 不换行；② `<dc:title id="t1">` 的属性段没剥干净；③ 下载字节进度模板被误用在「段落计数」上 | `win32::append_line()` 补 `\r\n`；`between()` 后再取 `find('>')` 之后的 body；`CliReporter::progress()` 仅在 `p.total > 102400` 时按字节显示，否则显示 message 里的 `n/m`。三张截图 07/08/09 记录在 `docs/screenshots/` |
| 24 | **委托 Full 生成时返回全假数据**：`engine="sherpa-via-Full"`（即使用 `--engine mock`）、`chapters/paragraphs/seconds=0`、`wav_bytes=57344`；日志里灌进整行 NDJSON | ① engine 标签硬编码；② 完全没读 Full 的产物，Summary 用 0 填充；③ 字节数只 `read_dir(out)` 统计一层（真实音频在多层子目录，只数到 `bookctl.db`）；④ 事件流 `BufReader.lines()` 原样 `rp.log(整行)` | `delegate_full()` 重写：解析 NDJSON 只输出 `[stage] message` + `segment_done` 驱动 `rp.progress()`；读 Full 自己的 `manifest.json`（`totals.chapters / totals.audio_seconds / engine.name / chapters[]`）回填；`engine` 动态生成；`dir_size_recursive()` 递归统计。修复后实测 `chapters=3 / paragraphs=14 / seconds=10.1 / engine=mock-via-Full / wav_bytes=681002`，Mini manifest 含 3 条真实章节 |
| 25 | 宿主终端里委托日志中文变 `鏁存湰涔﹀凡瀹屾垚` 一类乱码 | **不是程序缺陷**：把 Mini 的 stdout/stderr 重定向到文件后按 UTF-8 解码，中文全部正确（`\xe6\x95\xb4` = 整）；是 PowerShell 5.1 用 GBK(936) 解码子进程 stderr 的**宿主显示问题**。`[Console]::OutputEncoding = [Text.Encoding]::UTF8` 后立刻正常 | 不改程序输出编码（会破坏 CLI/MCP 的 JSON envelope 契约）；在 `help` 与便携包 `使用说明.txt` 写明 `chcp 65001` / `[Console]::OutputEncoding = [Text.Encoding]::UTF8`。GUI 用 UTF-16 控件、stdout 信封为干净 UTF-8，均不受影响。见 ADR-026 |
| 26 | 双击安装目录下的 `Uninstall-Mini.exe`（无参数）报「这不是完整的安装包（尾部缺少 payload 标记）」，卸载不执行 | 卸载器是安装器 stub 的**头部副本**（故意不带 overlay payload）；`main()` 原本只有显式 `/UNINSTALL` 才进卸载分支，无参数时掉进安装路径并校验 payload | `main()` 增加按文件名前缀判定：`Uninstall-*` 一律走卸载（「程序和功能」仍走注册表里显式 `/UNINSTALL /S`）。实测无参数双击能删目录；`/UNINSTALL /S` 静默卸载后 `dir / reg_product / reg_uninstall / lnk_start / lnk_desktop` 全为 False，重装后 `version=0.5.0` |

### Mini 相关的工程坑（非产品缺陷）

| 坑 | 处理 |
|---|---|
| PS 5.1 在 `$ErrorActionPreference='Stop'` 下把 rustc 的 stderr warning 当终止错误 | `mini/build.ps1` 的 `Invoke-Rustc()` 临时切 `Continue` + `Out-Null`，只依据 `$LASTEXITCODE` |
| `@common` 数组不能在表达式里和 `+` 混用、函数 stdout 会污染返回值、GUI 还在运行会锁住 exe | 用 `$mainArgs` / `$stubArgs` 变量、`Out-Null`、编译前关闭 `BookCtl.Mini.exe` |
| `reg.exe` 被宿主安全策略禁用 | 不绕过；改用 `Test-Path` 与 `[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey()` 核验 |
| Full 便携包带 `portable.marker`，装成安装版后数据仍写程序目录 | 安装器 `extract()` 后删除 marker，数据改走 `%LOCALAPPDATA%` |
| 非交互宿主里双击卸载会弹「卸载完成」MessageBox 并阻塞 | 自动化验收统一用 `/UNINSTALL /S`；真实桌面双击点掉弹窗即可 |

---

## 2026-11-24 夜 · v0.6 阅读器（#27–#30）

| # | 现象 | 根因 | 处理 / 验证 |
|---|---|---|---|
| 27 | `reader --dry-run` / MCP `book_inspect` / `job_start` 的 stdout 里，JSON 信封**前面多出一行**「解析完成：《…》· 3 章 · 362 字」 | `NullReporter::log()` 写的是 `println!` 而不是空实现。MCP stdio 与 CLI 都把 stdout 当协议通道，任何非协议文本都会破坏 JSON-RPC 流 / 让 `jq` 解析失败。v0.5 之前的 MCP 实测只跑了 `tools/list`（不触发 log），所以一直没暴露 | `NullReporter::log` 改为彻底静默。验证：`book_inspect` 与 `reader status` 的 stdout 只有一行纯净 JSON；MCP 9 个 tools 的 `tools/call` 全部正常 |
| 28 | 同一本书用 `testdata/测试之书.epub`（CLI 习惯写法）与 `testdata\测试之书.epub`（PowerShell 路径）分别调用，生成**两份缓存**、重复合成 | 缓存 key 直接用了调用方传入的原始路径字符串。相对/绝对、`/` 与 `\`、大小写差异都会算出不同 FNV 值 | `cache_dir_for()` 先用 `fs::canonicalize()` 转绝对路径再算 key。验证：相对与绝对路径两次调用 → `cache_dir` 完全相同，第二次 `cache_hits=3` |
| 29 | **（设计阶段发现，未上线）** 阅读器版安装目录定为 `…\本地朗读工厂\Reader`，而缓存一开始也放在 `…\Reader\cache` | 卸载器对安装目录做 `remove_dir_all`，缓存放在里面等于**卸载即删用户已生成的朗读缓存** | 缓存目录独立为 `…\本地朗读工厂\ReaderCache\`。验证：卸载后 `dir_exists=False` 但 `cache_wav` 数量 3 → 3 不变 |
| 30 | 只读多行 EDIT 里用 `EM_SETSEL` 高亮当前朗读段落，窗口打开后**看不到高亮** | Win32 的只读 EDIT 在**失去焦点**时不渲染选中背景（`HideSelection` 默认为 true） | 创建完控件后 `SetFocus(edit_text)`；截图中当前段落显示为蓝色选中块。另一种做法是 `EM_HIDESELECTION`，但给焦点更简单且不影响按钮点击 |

### 阅读器相关的判断（避免下一轮走回头路）

| 判断 | 说明 |
|---|---|
| **不要**为了"精确到字"去订阅 SAPI `SpeakProgress` | 经 PowerShell 封装后时间戳不可靠；段落级已足够（ADR-028）。 |
| **不要**把整本书一次合成完再开播 | 用户要的是"边看边读"，整本合成让第一句要等几十秒；预取窗口才是正解（ADR-029）。 |
| **不要**改成 `#[link(name="winmm")]` | 动态加载保证 `rust-lld` 自包含链接不依赖 `winmm.lib` 是否存在（ADR-030）。 |
| **不要**把 `ReaderCache` 并回 `Reader` | 会重现 #29 的卸载删数据问题。 |
| 生成线程遇到 `Failed` 段不会自动重试 | 避免坏段无限重试刷日志；GUI 的「重合成本段」是显式重试入口。 |
