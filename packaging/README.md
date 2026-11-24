# 封装与分发（Pro 版：便携版 + 安装版）

面向最终用户的两种形态，源码同一套，靠一个标记文件区分数据落点。

| 形态 | 产物 | 数据位置 | 适用 |
|---|---|---|---|
| **便携版**（Pro Portable） | `dist\本地朗读工厂-<ver>-win-x64.zip` | 程序目录内 `data\`、`sources\`（存在 `portable.marker`） | U 盘 / 离线机 / 免安装客户 |
| **Slim 轻量版** | `dist\本地朗读工厂-<ver>-win-x64-slim.zip` | 同上 | 体积敏感；CLI 需 .NET 10（GUI 零依赖） |
| **安装版**（Pro Setup） | `dist\本地朗读工厂-<ver>-setup.exe` | `%LOCALAPPDATA%\bookctl\` | 普通用户、桌面快捷方式、`.epub` 关联 |

两版都是 **self-contained + WindowsAppSDKSelfContained**：目标机不需要装 .NET 10，也不需要 Windows App Runtime。

## 一、构建

**最省事**：`packaging\make-release.cmd`（标准）或 `make-release.cmd -Slim`（轻量），一条命令产出便携包；
装了 Inno Setup 会顺带产出 setup.exe。下面是拆开的等价步骤。

实测体积（2026-11-24）：标准 **475 MB / zip 198 MB**；Slim **200 MB / zip 78 MB**。

```powershell
# 1) 便携版（发布 + 补齐 WinUI XAML 资源 + 目录整理 + zip）
powershell -ExecutionPolicy Bypass -File packaging\build-portable.ps1 -Version 0.3.0
powershell -ExecutionPolicy Bypass -File packaging\build-portable.ps1 -Version 0.3.0 -Slim

# 2) 签名（强烈建议：不签名 = 首次运行会被 SmartScreen 拦）
powershell -ExecutionPolicy Bypass -File packaging\sign.ps1 -Path dist\本地朗读工厂-0.3.0-win-x64

# 3) 安装包（需 Inno Setup 6：winget install JRSoftware.InnoSetup）
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" packaging\bookctl.iss /DMyVersion=0.3.0

# 4) 对 setup.exe 再签一次
powershell -ExecutionPolicy Bypass -File packaging\sign.ps1 -Path dist\本地朗读工厂-0.3.0-setup.exe
```

## 二、签名要点（决定用户敢不敢装）

1. **必须用代码签名证书**，否则 Windows 会显示"未知发布者"，SmartScreen 直接拦首次运行。
   - OV 证书：便宜，但需要累积信誉才不弹警告；
   - EV 证书：需硬件令牌（USB Key），首次即通过 SmartScreen；
   - Azure Trusted Signing：云签名，按签名次数计费，CI 友好。
2. **必须带时间戳**（脚本已带 `/tr`），否则证书到期后已分发的旧包签名全部失效。
3. 签名顺序：先签 exe/dll，再打 zip；安装包生成后再单独签 setup.exe。
4. 证书私钥不要进仓库：脚本只读环境变量（`BOOKCTL_PFX` / `BOOKCTL_CERT_SHA1` / `BOOKCTL_TS_*`）。

## 三、安装版行为

- `PrivilegesRequired=lowest`：默认 **per-user 安装**，不弹 UAC，装到 `%LOCALAPPDATA%\Programs`；
- 可选任务：桌面快捷方式、把 `.epub` 关联到本软件（写 `HKCU\Software\Classes`，不需要管理员）；
- 安装时删除 `portable.marker`，让数据走 `%LOCALAPPDATA%\bookctl`（Program Files 只读，绝不能写库）；
- 卸载默认保留用户的 `data`（成品音频很值钱），只在用户主动清理时删除。

## 四、Pro 版差异化（预留位）

代码里已有 `Edition` 字段（便携版自动写 `pro`），建议后续按此挂：
- 批量队列（多本书排队）、无限制并发；
- MCP 常驻服务模式（Streamable HTTP，供团队内网调用）；
- 商用音色库授权校验（离线 license 文件，非对称签名校验，不要联网强制激活）；
- 硬件绑定可选，但**不要**做联网心跳——离线软件的用户最反感这个。

## 五、发布前检查清单

- [ ] `dotnet build bookctl.sln -c Release` 零错误
- [ ] 便携版在**干净机**（无 .NET / 无 VS）双击可启动
- [ ] 中文路径、U 盘路径、OneDrive 路径均能导入 EPUB
- [ ] `bookctl doctor` 全绿；ffmpeg 缺失时 M4B 降级有明确提示
- [ ] 断网状态下 mock / 已下载模型可完整跑完一本书
- [ ] 卸载后 `%LOCALAPPDATA%\bookctl` 中成品仍在
