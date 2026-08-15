# RTLauncher Theme API 1.0

简体中文 | [English](./README.md)

第一次编写 Theme？请先阅读 [Theme 开发者快速上手](./DEVELOPER-GUIDE.zh-CN.md)，再查阅 [Theme SDK 1.0 参考](./SDK-REFERENCE.zh-CN.md)。

Theme API 允许用户替换指定的 React 路由和界面插槽。Theme 也可以添加本地路由。对于 Theme 未替换的路由或插槽，RTLauncher 继续使用内置界面。

## 安全模型

Theme 是受信任的本地代码，不在沙箱中运行。Theme 可以使用稳定 SDK，也可以通过 `sdk.unsafe.invoke` 请求主程序允许的 Tauri 命令。只安装你信任其作者和来源的 Theme。

RTLauncher 对 Theme 包执行以下检查：

- 拒绝绝对路径、父目录跳转、重复 ZIP 条目、符号链接和不安全的文件读取。
- 限制归档文件数量、单个文件大小和解压后的总大小。
- 安装前检查 SHA-256 值。
- 先写入暂存目录，再用原子目录重命名完成安装。
- 当准备或激活失败时，继续显示原有界面。
- 当 30 秒内出现三次运行时错误时，切换回内置 Theme。

这些措施保护 Theme 包的存储过程。用户激活 Theme 后，这些措施不会限制 Theme 代码的能力。

## Theme 包类型

RTLauncher 接受两种 Theme 包：

1. 用于正常安装的 `.rtltheme` ZIP 归档。
2. 用于开发的构建输出目录。该目录必须包含 `build` 生成的 `manifest.json` 和可执行入口。修改源码后，请重新构建并在 RTLauncher 中重新加载 Theme。

构建后的归档具有以下结构：

```text
theme.rtltheme
├── manifest.json
└── dist
    ├── theme.js
    └── theme.css
```

`manifest.json` 必须符合 [manifest.schema.json](./manifest.schema.json)。归档必须包含 `integrity`。开发清单可以省略此字段，因为 CLI 会在构建时生成它。

## 兼容性策略

`schemaVersion`、`engines.themeApi` 和 `ThemeDefinition.apiVersion` 使用主版本兼容规则。

- 1.x 主程序接受 1.x 清单格式。
- 1.x 主程序接受使用 Theme API 1.x 的 Theme 定义。
- Theme 必须声明兼容的 RTLauncher 和 Theme API 版本范围。
- 次版本可以增加新的可选字段。
- 破坏字段或行为兼容性的改动必须使用新的主版本。

请把私有元数据放在 `extensions` 中。请使用反向域名作为键，以避免名称冲突。

## 作者工作流

在仓库根目录中使用 Node.js 和 pnpm：

```sh
pnpm install
pnpm --filter @rtlauncher/theme-cli build
pnpm --filter @rtlauncher/example-hello-theme run validate
pnpm --filter @rtlauncher/example-hello-theme run build
pnpm --filter @rtlauncher/example-hello-theme run pack
```

CLI 提供四个命令：

```text
rtl-theme validate <project-or-manifest>
rtl-theme build <project>
rtl-theme pack <project> [output.rtltheme]
rtl-theme inspect <file.rtltheme>
```

`build` 会生成一个自行注册的 IIFE。它把 React、JSX runtime 和 `@rtlauncher/theme-ui` 映射到主程序桥接层，因此不会包含第二份 React。它也会用 `@scope` 规则限制 Theme CSS 的作用范围，并写入 SHA-256 值。

请参阅[完整的 Hello Theme 示例](../../examples/themes/hello-theme/README.md)。

## Theme 入口

从源码入口导出一个 Theme 定义：

```tsx
import { Button } from "@rtlauncher/theme-ui";
import { defineTheme } from "@rtlauncher/theme-sdk";

export default defineTheme({
  id: "com.example.nebula",
  version: "1.0.0",
  apiVersion: "1.0.0",
  setup(context) {
    context.slots.register({
      id: "nebula.page-action",
      target: "page.header.actions",
      mode: "after",
      component: () => <Button>Nebula</Button>,
    });
    return {
      activate() {},
      deactivate() {},
      dispose() {},
    };
  },
});
```

构建产物必须调用一次 `window.__RTL_THEME_REGISTER__`，且只能调用一次。CLI 会添加此调用。当注册缺失、发生第二次注册或 Theme ID 不匹配时，RTLauncher 会拒绝加载。

## 生命周期

`setup(context)` 用于注册路由和插槽。它可以返回生命周期函数。

- `activate(event)` 在 RTLauncher 发布新的活动 Theme 前运行。
- `deactivate(event)` 在 RTLauncher 发布下一个 Theme 后运行。
- `dispose()` 释放 Theme 资源。
- 每个激活和停用事件都包含一个 `AbortSignal`。
- 准备、激活、停用和释放操作都有时间限制。

RTLauncher 按 Theme 代次隔离注册内容。重新加载时，RTLauncher 先准备新代次。只有新代次准备完成后，它才移除旧代次。

## 路由

Theme 可以替换或包装核心路由，也可以添加本地路由。

| 核心路由 ID | 内置区域 |
| --- | --- |
| `core.home` | 首页 |
| `core.launch` | 启动 |
| `core.download` | 下载 |
| `core.download.detail` | 下载详情 |
| `core.multiplayer` | 多人游戏 |
| `core.tools` | 工具 |
| `core.settings` | 设置 |
| `core.game-settings` | 游戏设置 |
| `core.instance.*` | 模组、世界、资源包、光影、截图和投影 |

使用 `context.routes.override` 修改核心路由。使用 `context.routes.add` 添加本地路径。如果贡献内容发生渲染错误，只有该贡献会恢复为内置内容。

## 插槽

插槽模式包括 `replace`、`before`、`after` 和 `wrap`。同一模式中的多个贡献由 `order` 控制顺序。

| 插槽组 | ID |
| --- | --- |
| 标题栏 | `app.titlebar.leading`、`app.titlebar.center`、`app.titlebar.actions` |
| 侧边栏 | `app.sidebar`、`app.sidebar.header`、`app.sidebar.navigation`、`app.sidebar.footer` |
| 内容区 | `app.content.before`、`app.content.after` |
| 页面 | `page.header`、`page.header.actions`、`page.footer` |
| 启动 | `launch.primary-action` |

## 稳定 SDK

`context.sdk` 提供以下稳定服务：

| 服务 | 用途 |
| --- | --- |
| `accounts` | 列出账号、读取当前账号和选择账号。接口不会返回秘密令牌。 |
| `instances` | 列出和选择游戏实例。 |
| `launch` | 启动或停止游戏，并读取启动状态。 |
| `downloads` | 列出和取消下载任务。 |
| `router` | 导航、返回和读取当前位置。 |
| `settings` | 读取和更新启动器设置。 |
| `i18n` | 翻译主程序键并读取当前语言。 |
| `ui` | 显示提示或确认对话框。 |
| `storage` | 在 Theme 专用命名空间中存储 JSON。 |
| `events` | 发送和订阅 Theme 专用事件。 |
| `platform` | 读取操作系统和 API 版本信息。 |

其他上下文服务包括 `routes`、`slots`、`assets`、Theme 设置、事件和带有 Theme 前缀的日志记录器。

`sdk.unsafe.invoke(command, args)` 是明确的底层调用入口。此入口没有稳定兼容性保证。Theme 必须在 `permissions.unsafeCommands` 中请求每个命令，并在 `disclosures` 中说明用途和影响。即使清单请求了某个命令，如果该命令不在主程序允许列表中，RTLauncher 也会拒绝调用。

## 原生主程序命令

前端主程序使用以下 Tauri 命令：

```text
theme_list
theme_install_archive
theme_register_dev_directory
theme_remove
theme_read_text
theme_read_binary
theme_set_active
theme_mark_healthy
theme_is_trusted
theme_set_trusted
```

普通 Theme 不应直接调用这些命令。除非稳定 SDK 没有所需功能，否则请使用稳定 SDK。

## 用户切换和故障恢复

用户可以在“外观”设置页面安装、注册、激活、重新加载和删除 Theme。每个本地 Theme 第一次激活前，RTLauncher 都会要求用户确认信任。

激活操作使用事务流程。RTLauncher 在修改当前界面前准备新的构建产物。激活失败时，RTLauncher 保留原有界面和状态。如果在一段时间内没有错误，待确认的激活会变为健康状态。如果应用在确认前退出，原生存储会在下次启动时恢复到最后一个健康的 Theme。
