# RTLauncher

一款现代化的 Minecraft 桌面启动器，基于 **Tauri 2** (Rust) + **Next.js 16** 构建，轻量、快速且跨平台。

如果需要更多支持,欢迎访问官方文档[RTL官网](rtlauncher.cfd/docs "Docs"),也可以加入RTL用户群:1013525092

## 功能特性

-  **多版本支持** — 支持原版、Forge、NeoForge、Fabric、Quilt、LiteLoader、OptiFine 共 7 种 Mod Loader
-  **整合包管理** — 支持 CurseForge / Modrinth 整合包一键导入、导出与缓存
-  **多人联机** — 基于 OpenP2P 的局域网联机方案，支持房主创建和玩家加入
-  **皮肤系统** — 3D 皮肤预览（skinview3d）、Microsoft / LittleSkin / Yggdrasil 账户登录
-  **国际化** — 内置中文（简体）与英文界面，可轻松扩展
-  **自动安装** — 自动下载 Java 运行时、Mod Loader、依赖库
-  **主题切换** — 亮/暗主题无缝切换
-  **低体积** — Rust 原生打包，安装包体积小、内存占用低

## 技术栈

| 层级 | 技术 |
|------|------|
| 前端 | Next.js 16 (App Router), React 19, TypeScript, Tailwind CSS |
| UI 组件 | shadcn/ui, Framer Motion, Radix UI |
| 后端 | Rust (Tauri 2), reqwest, tokio |
| 构建 | pnpm, Tauri CLI |

##  快速开始

### 环境要求

- **Node.js** >= 18
- **Rust** >= 1.77
- **pnpm** >= 8（推荐）

### 安装依赖

```bash
pnpm install
```

### 开发模式

```bash
pnpm tauri dev
```

### 构建发布

```bash
pnpm tauri build
```

# 项目结构

```
RTLauncher/
├── app/                        # Next.js App Router 页面
├── components/                 # React 组件
│   ├── ui/                     # shadcn/ui 基础组件
│   ├── launcher/               # 启动器相关组件
│   ├── mod/                    # Mod 管理组件
│   ├── multiplayer/            # 联机组件
│   └── settings/               # 设置组件
├── src-tauri/                  # Tauri (Rust) 后端
│   ├── src/
│   │   ├── auth/               # 账户认证
│   │   ├── handler/            # Tauri 命令处理器
│   │   │   ├── launcher.rs     # 游戏启动逻辑
│   │   │   ├── system.rs       # 系统操作
│   │   │   └── ...             # 其他 handler
│   │   ├── downloader/         # 下载与安装
│   │   ├── mutiplayer/         # OpenP2P 联机
│   │   └── version_management/  # 版本资源管理
│   └── Cargo.toml
├── components/i18n/locales/    # i18n 翻译文件
├── crowdin.yml                 # Crowdin 文件同步配置
└── types/                      # TypeScript 类型定义
```

##  Crowdin 翻译同步

Crowdin 使用 `components/i18n/locales/en-US.json` 作为英文源文件，并将目标语言写入同一目录。GitHub Actions 每周同步一次翻译，并创建翻译更新 PR。

仓库需要配置以下 GitHub Actions Secrets：

- `CROWDIN_PROJECT_ID`
- `CROWDIN_PERSONAL_TOKEN`

首次启用时，手动运行 `Sync Crowdin translations` 工作流，并启用 `seed_existing_translations`，以将现有翻译上传到 Crowdin。

## 支持的 Mod Loader

| Loader | 版本获取 | 自动安装 | 取消下载 |
|--------|---------|---------|---------|
| Forge | ✅ | ✅ | ✅ |
| NeoForge | ✅ | ✅ | ✅ |
| Fabric | ✅ | ✅ | ✅ |
| Quilt | ✅ | ✅ | ✅ |
| OptiFine | ✅ | ✅ | ✅ |
| LiteLoader | ✅ | ✅ | ✅ |

## 账户系统

- **Microsoft 账户** — Device Code OAuth 登录
- **LittleSkin** — 第三方皮肤站认证
- **Yggdrasil** — 自定义验证服务器（authlib-injector）
- **离线账户** — 无需登录的本地账户

## 贡献

欢迎贡献代码！可以通过以下方式参与：

1. Fork 本仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'feat: add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 开源协议

本项目基于 [MIT License](LICENSE) 开源。
