# Roadmap · 未完成事项与接手指引

> 状态口径：`⬜ 未开始` / `🔨 进行中` / `✅ 已完成`。
> 本期（2026-11-23 ~ 24）已完成项见 `HANDOVER.md §0/§7` 与 `docs/worklog/`。
> **接手后建议从 P0 第一条开始**，每条都要按 `docs/workflows/07-verification.md` 跑回归。

---

## P0 · 交付闭环（优先）

### ⬜ R-01 · 真实签名安装包
- **为什么**：用户明确要求"签名封装形成 Pro 版可移动版 + exe 安装版"。便携版已能构建，安装版与签名脚本已写好但未实跑。
- **阻塞**：需要用户提供证书：`BOOKCTL_PFX` + `BOOKCTL_PFX_PASS`，或 `BOOKCTL_CERT_SHA1`，或 Azure Trusted Signing 的 `BOOKCTL_TS_*`。**私钥绝不进仓库**。
- **步骤**：
  1. `winget install JRSoftware.InnoSetup`
  2. `iscc packaging\bookctl.iss`
  3. `.\packaging\sign.ps1`（先签目录内 exe/dll，再单独签 setup.exe）
  4. `signtool verify /pa /q`
  5. 实机安装，验证 `.epub` 关联、per-user 数据目录、SmartScreen 不再拦截
- **参考**：`docs/workflows/06-packaging.md`。

### ⬜ R-02 · `source pull` 从干净目录重跑解包流程
- **为什么**：`Sources.cs` 修过"归档残留 + 目录嵌套"（解包到 `tmpDir/extract` + 删除归档），
  但当时是**手工修正**本机已安装目录，尚未从干净 sources 目录完整复验新流程。
- **步骤**：`BOOKCTL_SOURCES=<临时目录> bookctl source pull sherpa-vits-zh-fanchen`，
  检查目标目录直接含 `*.onnx` / `tokens.txt` / `lexicon.txt` / `*.fst` / `.installed.json`，无 `.tar.bz2` 残留、无嵌套子目录。
- **注意**：国内下载约 20–30 分钟，走 `gh-proxy.com` 镜像回退；下载期间不要编译（会锁 DLL）。

---

## P1 · 引擎与硬件能力

### ⬜ R-03 · 接入 CosyVoice 3 真实权重（GPU）
- **为什么**：架构初版把它定为默认引擎（Apache-2.0 双许可、中文 CER 0.81%），
  目前只是 `engine/tts_server.py` 里的适配点 + `Sources` 的 `manual` 条目。
- **阻塞**：需要 ≥6 GB 显存机器（本机 GTX 1050 2GB 跑不了）与权重文件。
- **适配点**：`ensure_model()` / `synth_real()`；`Device.RequiredVram("cosyvoice3")=6144` 已就位；
  需保证 `sample_rate` / `voice` / `seed` 语义与 `SynthesisProfile` 一致。
- **参考**：`docs/workflows/05-tts-engines.md §7`、架构 MD §A.7。

### ⬜ R-04 · 多显卡 0/1/2 实机验证
- **为什么**：`GpuIndex` + `device list` 逻辑已实现，但本机只有 1 张卡，从未实机验证切换。
- **步骤**：多卡机器上跑 `device list`，分别 `--gpu 0/1/2 --device gpu`，核对 `nvidia-smi` 与 `DevicePlan.Display`。

### ⬜ R-05 · 音色一致性 A/B 盲测
- **为什么**：架构 MD §3.2 指出"整本书音色一致性是成败点"，当前未做量化验收。
- **步骤**：同章不同 seed / 不同分段策略的盲测，固定 `voice` + `seed`，检查跨段韵律漂移。

---

## P2 · 质量与产品化

### ⬜ R-06 · S5 ASR 回验闭环
- 当前 `qc/report.json` 是 `verify="skipped"`。接入 faster-whisper 做抽样回验（可选全量），
  把 CER / 漏字写进 `qc/report.json`，失败章节标 `needs_review`。

### ⬜ R-07 · 母带处理
- ffmpeg 两遍 `loudnorm`（EBU R128）、章节间静音、封面/元数据完善、可选 `m4b` 章节封面。

### ⬜ R-08 · Pro 版功能门禁
- `BookCtlConfig.Edition` 已预留（`community` / `pro`），`edition.json` 已写出，
  但授权校验与差异化功能尚未实现。

### ⬜ R-09 · 简洁模式 UI 细节打磨
- 按用户给的 Apple 风格提示词补：500ms 缓动、hover 上浮、卡片阴影、Vibrancy 顶栏、
  减少动态效果（无障碍）、键盘焦点、空/错/加载三态。当前已实现配色、圆角、双模式、结果卡。

### ⬜ R-10 · 打包脚本健壮化
- `build-portable.ps1` 目前若先删 `dist/` 会被安全策略拦截；脚本本身应改为**原地增量覆盖**逻辑，
  避免接手者手动敲 §6 的等价命令。

### ⬜ R-11 · 更大 / 更复杂的 EPUB 回归
- 当前只用自造 3 章中文书（352 字）。补：EPUB3 `nav.xhtml`、无 NCX、CSS hidden、
  图片章节、超长段落、繁体/英文混排、DRM 拒绝路径、非 UTF-8 编码、中文路径。

### ⬜ R-12 · CI / 自动化
- GitHub Actions（Windows runner）：`dotnet build` → mock 全链路 → ffprobe 校验 →
  便携版 publish 产物 artifact。注意仓库里不含模型，真语音链路只能在本地跑。

---

## 本期已完成（存档，不要重做）

| 项 | 状态 |
|---|---|
| 联网调研 + 架构评审稿（MD + HTML） | ✅ |
| 总架构 MD 附录 A（v0.2 WinUI）/ 附录 B（v0.3 双模式/设备/源/封装） | ✅ |
| Core / CLI / MCP / Worker / WinUI App 五项目代码 | ✅ |
| .NET SDK 10.0.404 安装 + 全解决方案 0 error 编译 | ✅ |
| mock 校样链路端到端（3 章 M4B + 章节标记 + manifest/qc） | ✅ |
| MCP `initialize` / `tools/list` 验证 | ✅ |
| WinUI App 实际启动 + Apple 风格双模式 UI + 截图 | ✅ |
| 设备选择 / 显卡枚举 / CPU-GPU-hybrid 决策 | ✅ |
| 分布式 TTS 推理可行性结论 | ✅ |
| sherpa-onnx CPU 真语音引擎 + 模型安装 + sidecar 协议 | ✅ |
| 源管理 / 内置目录 / 一键下载 / 镜像回退 / 解包安装 | ✅ |
| 便携版 / 安装版 / 签名脚本 | ✅（签名实跑缺证书） |
| CPU 真语音整本链路（97.9 s / RTF 0.35）+ UI 一键生成可视化 | ✅ |
| 12 个真实缺陷修复 | ✅ |
| 便携版 zip（≈196 MB）+ 便携模式验证 | ✅ |
| 完整交接文档体系 + 推送代码仓库 | ✅ 本次 |

---

## 建议的第一步动作（给接手 AI）

1. 读 `HANDOVER.md` → `AGENTS.md` → `architecture-epub2audiobook.md`（附录 A/B）。
2. 按 `docs/workflows/01-build-and-run.md` 跑一遍 `build` + `doctor` + mock 回归，确认基线。
3. 问用户要签名证书参数，推进 **R-01**；或先做无需外部依赖的 **R-02 / R-10 / R-11**。
4. 每改一处，按 `AGENTS.md §6` 同步文档，按 `docs/workflows/07-verification.md` 回归。
