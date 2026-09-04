# 统一桌面端发布流程

一个 GitHub Release 可以同时挂载 macOS 与 Windows 的多个资产。为了避免先看到只有 macOS 文件、Windows 文件散落在另一个发布页，仓库把“构建与验收”和“上传到 Release”分开：只有两个平台的文件都校验合格后，提升工作流才创建或更新同一个 Release。

## 对 v1.5.3 的冻结契约

- 标签：`v1.5.3`
- 源码提交：`05023b7269fb4991f8a9ebe4d8524b328382f09e`
- 详细资产、校验和验收基线：[`release-contracts/v1.5.3.json`](../release-contracts/v1.5.3.json)

Windows 构建会生成并实际验收 x64 EXE、MSI 和便携 ZIP：安装、启动、从 `v1.5.0` MSI 升级，以及卸载。它不会创建或改写 GitHub Release。

## 发布顺序

1. 运行 **Build macOS release assets**，记下成功运行 ID。
2. 运行 **Qualify Windows installers**，选择对应的冻结发布契约，记下成功运行 ID。
3. 运行 **Promote qualified desktop release**，填入 Windows 运行 ID 与 macOS 运行 ID。

第三步会验证两个运行的提交、文件名、SHA-256 和 Windows 安装验收记录；通过后才将所有文件上传到同一个 GitHub Release，并使用中文说明。

对于已经先发布 macOS 文件的历史版本，第三步可以留空 `macos_run_id`：它会复用同一 Release 内已有的 macOS 文件，但仍会校验文件名和 SHA-256。

## 范围

这是 GitHub Release 下载页的统一列表，不是应用内自动更新功能。本仓库尚未在此流程中配置和验证签名的自动更新元数据；用户应手动下载对应操作系统与架构的安装包。
