# minicon-harness 签名开通 — 业主网页操作单（S1）

预计 10 分钟，全部在浏览器点选，不用命令行。做完把两个 ID 发我即可。

## 第 1 步：建 Entra 应用（约 3 分钟）

1. 打开 **https://portal.azure.com**（`wjcpns` 工作台，用公司 work 账号登录）
2. 顶部搜索 **App registrations** → **+ New registration**
3. Name 填 `minicon-harness-release-signing`
4. Supported account types 选 **Accounts in this organizational directory only**
5. 点 **Register**

## 第 2 步：加 GitHub 联合凭据（约 3 分钟）

在刚建好的 app 页面：

1. 左侧 **Certificates & secrets** → **Federated credentials** 标签 → **+ Add credential**
2. 场景选 **GitHub Actions deploying Azure resources**
3. 表单填写：
   - Organization: `partnernetsoftware`
   - Repository: `minicon-harness`
   - Entity type: **Environment**
   - GitHub environment name: `release-signing`
4. Credential name 填 `github-minicon-harness-release-signing` → **Add**

> 网页表单会自动拼出 subject，正确值应等于：
> `repo:partnernetsoftware@53246189/minicon-harness@1435216282:environment:release-signing`
> （可以展开凭据详情核对一眼）

## 第 3 步：给签名 profile 授权（约 3 分钟）

1. 顶部搜索并打开 **Artifact Signing**（或 Subscription → 资源组 `rg-signing` → 账号 `partnernetsigning`）
2. 左侧 **Certificate profiles** → 点 **minicon-public**
3. 左侧 **Access control (IAM)** → **+ Add** → **Add role assignment**
4. Role 搜 **Artifact Signing Certificate Profile Signer** → 下一步
5. Members → **+ Select members** → 搜 `minicon-harness-release-signing` → 选中 → 下一步 → **Review + assign**

## 第 4 步：把两个 ID 发我（约 1 分钟）

在 app 的 **Overview** 页复制：

- **Application (client) ID** → 发我
- **Directory (tenant) ID** → 发我

Subscription ID 在 Subscription 概览页（搜索 **Subscriptions**）→ 也发我。

我拿到后把它们写进 harness 仓的 `release-signing` environment secrets，跑双检查，然后 dispatch qualification 签名——你不用再做任何事。

## 参考

- 笔记出处：`company-dev-hub/skills/sign-windows-artifacts/references/azure-artifact-signing-setup.md` §4（CLI 版；本单是等价的 portal 点选版）、`product-signing-log.md`（winmgr 2026-10-29 同型先例）
- 公司规则：`wjcpns` 工作台操作；不要把 client/tenant/subscription ID 写进任何仓库文件
