# docs/ · 交接文档索引

> **接手 AI 的阅读顺序**：`HANDOVER.md` → `AGENTS.md` → 本目录 → 架构 MD。
> 用户是架构师，用中文「许愿式」下指令；背景全在这里，不要追问历史。

## 目录

| 路径 | 内容 | 何时读 |
|---|---|---|
| `../HANDOVER.md` | **总入口**：现状 / 5 分钟跑起来 / 代码地图 / 已知坑 / 实测基线 / 下一步 | 第一个读 |
| `../AGENTS.md` | **仓库级 AI 约定**：构建命令、禁止事项、代码模式、文档同步清单 | 动手改代码前 |
| `../architecture-epub2audiobook.md` | **总架构唯一事实源**（v0.1 + 附录 A v0.2 + 附录 B v0.3 + **附录 C v0.5 mini**） | 设计变更时 |
| `../architecture-epub2audiobook.html` | 架构可视化评审版（浏览器打开） | 评审时 |
| `../README.md` | 用户向说明、命令速查、实现边界、§7.1 缺陷台账、**§10 Mini 与跨机测试** | 查命令 / 对外说明 |
| `environment.md` | **本机环境快照**：绝对路径、版本、网络限制、安全策略、Rust 工具链 | 任何路径/版本问题 |
| `decisions/ADR.md` | 30 条技术决策（含 mini 的 ADR-021~026、阅读器的 ADR-027~030）+ 未决议题 | "为什么这么设计" |
| `defects.md` | 30 条缺陷/问题台账（含 mini 的 #22–#26、阅读器的 #27–#30；现象 → 根因 → 修法 → 验证）+ 工程坑 | 遇到诡异现象 |
| `roadmap.md` | P0/P1/P2 待办（R-13 Mini、R-17 阅读器已落地；R-14/15/16/18 待办）+ 本期已完成存档 + 建议第一步 | 决定做什么 |
| `conversation/01-full-dialogue.md` | 本期完整对话（7 轮用户 / 112 条助手回复 / 10 条系统事件） | 追溯用户原话 |
| `conversation/02-context-summaries.md` | 3 次上下文压缩的历史摘要 | 补齐折叠内容 |
| `conversation/03-action-ledger.md` | 493 次工具调用全量台账 | 追溯改过哪些文件、跑过哪些命令 |
| `conversation/04-v05-handoff.md` | **v0.5 收尾对话与决策摘要**（用户原话、按时间顺序行动、7 项决策、错误修复、待办） | 接手 v0.5 时 |
| `conversation/05-v06-handoff.md` | **v0.6 阅读器交接**：为什么是模式、三个关键设计、数据落点、已修缺陷、回归入口、明确边界 | 接手阅读器时 |
| `worklog/2026-11-23.md` | Day 1 工作记忆：调研 + 架构评审稿 | 追溯 Day 1 |
| `worklog/2026-11-24.md` | Day 2 工作记忆：实现 / 编译 / 验收 / v0.3 / 真语音 / v0.4 | 追溯 Day 2 |
| `worklog/2026-11-24-v05.md` | **v0.5 Mini 产品线收尾**：Rust 工具链、委托链路修复、安装包/卸载验收、跨机测试指引 | 接手 mini 时 |
| `worklog/2026-11-24-v06.md` | **v0.6 阅读器**：段落级同步、跨章预取窗口、播放链路、第三个安装包、缓存安全 | 接手阅读器时 |
| `worklog/2026-11-25.md` | **发布通道落地**：源码/历史 tag 入库、安装包进 Releases、PAT 认证区分、发版脚本与工程坑 | 发版 / 找安装包时 |
| `workflows/01-build-and-run.md` | 环境、构建、GUI/CLI/MCP 三种运行、故障排查 | 跑起来 |
| `workflows/02-pipeline.md` | 六阶段流水线、状态机、checkpoint、产物契约 | 改流水线 |
| `workflows/03-control-plane.md` | CLI 命令表、MCP tools、envelope、编码、便携模式判定 | 改接口 |
| `workflows/04-device-and-sources.md` | 设备决策、源管理、分布式推理结论 | 改设备/源 |
| `workflows/05-tts-engines.md` | 三档引擎、sidecar 协议、模型安装、GPU 适配点 | 改引擎 |
| `workflows/06-packaging.md` | 便携版 / 安装版 / 签名 / **三个 setup.exe（Mini / 阅读器 / Full）** / **§10 GitHub Releases 发布** / 发布检查清单 | 发版 |
| `../packaging/publish-release.ps1` | **一键发版**：查/建 Release、同名资产先删再传、上传三个 setup.exe + 便携 zip（需 `GITHUB_TOKEN`） | 发 Release |
| `../packaging/release-notes-v0.6.0.md` | v0.6.0 Release 正文：该下哪一个（`BookCtl-*` ASCII 资产名 + 中文 label）、系统要求、SHA-256、已知边界、源码入口 | 写 Release 说明 |
| `workflows/07-verification.md` | 验收流程 + 实测基线（含 Mini 回归手册） | 每次改动后 |
| `screenshots/01-simple-mode.png` | 简洁模式界面 | UI 对照 |
| `screenshots/02-detailed-mode.png` | 详细参数模式界面 | UI 对照 |
| `screenshots/03-simple-result-sherpa.png` | 真语音一键完成的结果卡 + 章节卡片 | UI 对照 |
| `screenshots/07-mini-ui.png` | Mini Win32 GUI 初版 | UI 对照 |
| `screenshots/08-mini-ui-fixed.png` | Mini GUI 日志换行修复后 | UI 对照 |
| `screenshots/09-mini-installed.png` | Mini 安装版启动界面 | UI 对照 |
| `screenshots/10-reader-ui.png` | **阅读器**：三栏（章节/正文/段落导航）+ 当前段落高亮 + 后台已备 N 段 | UI 对照 |
| `screenshots/11-reader-installed.png` | **阅读器安装版双击启动**：自动恢复上次书目、后台跨章预取 | 验收对照 |
| `../tools/export_session.py` | 会话转录 → 上述 conversation 文档的导出器 | 重新导出对话 |

## 文档维护约定

1. **架构变更**：先改 `architecture-epub2audiobook.md`（追加附录，不覆写历史），再改代码，
   然后同步 `README.md`、对应 `workflows/`、`decisions/ADR.md`。
2. **修 bug**：追加到 `defects.md` 与 `README.md §7.1`，写清复现现象与根因。
3. **新决策**：`decisions/ADR.md` 追加新编号，不要改旧条目。
4. **环境变化**：更新 `environment.md` + `HANDOVER.md §2` + `workflows/01`。
5. **完成任务 / 新增待办**：更新 `roadmap.md` 状态。
6. **每轮工作结束**：在 `worklog/` 追加当日日志（append-only）。

## 关于「所有对话内容」

`conversation/` 三份文件由 `tools/export_session.py` 从 WorkBuddy 会话 jsonl 导出：
保留用户实际输入、助手文本回复、上下文压缩摘要、全量工具动作与系统事件；
**未导出**宿主注入的运行时上下文（system-reminder / identity / project context）、
模型内部 reasoning、工具结果原始大块输出——这些属于运行时内部信息，不属于对话内容。
原始 jsonl 保留在本机（路径见 `environment.md §8`），需要时用导出器重新生成。
