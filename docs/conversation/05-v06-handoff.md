# v0.6 交接说明：阅读器（边看边读 + 后台预生成）

> 本文只记录 v0.5 → v0.6 这一轮的增量。全量对话与台账见
> `01-full-dialogue.md` / `03-action-ledger.md` / `04-v05-handoff.md`。

## 1. 一句话

在 `mini/` 单 exe 里加了一个 `reader` 模式：中间显示 EPUB 正文、当前段落高亮并朗读、
后台只预生成「往后 6 段 + 下一章前 2 段」，并单独出了一个 `本地朗读工厂-阅读器-0.6.0-setup.exe`。

## 2. 为什么是模式而不是新产品线

用户说「再在这个基础上修改一个另一个版本」，重点在**在基础上**。v0.4 刚花力气把结构理清，
如果新开一条代码线，会立刻出现三份 EPUB 解析、三份 SAPI 调用、三份配置读取。
所以阅读器复用 `audio::parse_epub` / `Config` / JSON envelope / 安装器 stub，
只新增 `mini/src/reader.rs`，并通过**文件名**区分发布形态（`BookCtl.Reader.exe`）。
决策记录：`docs/decisions/ADR.md` ADR-027。

## 3. 三个关键设计（改代码前必须知道）

1. **同步粒度 = 段落**（ADR-028）。每段一个 WAV、一份缓存、一次播放。
   不要为了逐字高亮去订阅 SAPI `SpeakProgress` —— 经 PowerShell 封装后时间戳不可靠。
2. **预取窗口跨章**（ADR-029）。`window_end = max(cursor + prefetch, 下一章首段 + 2)`，
   默认 `prefetch = 6`。只在当前章内预取会在章末卡住等生成。
3. **播放动态加载 `winmm.dll`**（ADR-030）。`SND_FILENAME|SND_NODEFAULT` 同步播完才推进游标，
   暂停用 `SND_PURGE`。不要给链接器加 `winmm.lib`（会破坏 `rust-lld` 自包含链接）。

## 4. 数据落点（卸载/换机相关）

| 路径 | 内容 | 卸载是否删除 |
|---|---|---|
| `…\本地朗读工厂\Reader\` | 阅读器程序 + `Uninstall-Reader.exe` + 使用说明 | **删** |
| `…\本地朗读工厂\ReaderCache\<hash>\` | 段落 WAV 缓存（按书+语速分目录） | **保留** |
| `…\本地朗读工厂\ReaderCache\last.txt` | 上次打开的书（绝对路径） | **保留** |
| `…\本地朗读工厂\config.json` | 运行配置（与 Mini/Full 共用） | 保留 |
| `…\本地朗读工厂\output\` | 批量模式产出 / 整本导出 | 保留 |

> 千万不要把缓存放进 `…\Reader\`：那是安装目录，`uninstall()` 会 `remove_dir_all`（缺陷 #29）。

## 5. 这一轮修的既有缺陷

- **#27 `NullReporter::log` 往 stdout 打印**：会破坏 MCP stdio 与 CLI envelope。
  v0.5 的 MCP 验收只跑了 `tools/list`（不触发 log），所以一直没暴露。
- **#28 缓存键未规范化**：`testdata/a.epub` 与 `testdata\a.epub` 会算出两个缓存目录。
  现在先 `fs::canonicalize()` 再算 FNV。
- **#29 缓存目录与安装目录冲突**（设计阶段发现）：改到 `ReaderCache`。
- **#30 只读 EDIT 失焦不渲染选中**：`SetFocus(edit_text)`。

## 6. 回归入口

```powershell
.\mini\build.ps1
.\packaging\make-installer.ps1            # 三个包；-SkipFull 可跳过 197 MB
$R="dist\release\mini-portable\BookCtl.Mini.exe"
& $R reader --input testdata\测试之书.epub --dry-run
& $R reader --input testdata\测试之书.epub --headless --count 5
& $R reader --input testdata\测试之书.epub --play-test
```

完整断言见 `docs/workflows/07-verification.md` §9.5、发布清单见 `06-packaging.md` §9.5。

## 7. 明确的边界（不要误以为已实现）

- 阅读器**只渲染纯文本正文**：EPUB 的图片、脚注、表格不显示；无逐字高亮、无分页/双栏。
- 阅读器**朗读用系统 SAPI**（即时、音质一般）。要模型音质请点「整本导出…」（走 Full 流水线），
  或在 Mini/Full 的批量模式生成整本。给 Full 加单段接口是 R-18。
- 只记住**上次打开的书**，不记阅读进度（position）；换书后从第一段开始。
- 生成失败的段不会自动重试，需要点「重合成本段」。
