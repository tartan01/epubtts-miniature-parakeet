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
