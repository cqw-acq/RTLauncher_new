# Theme SDK 1.0 参考

本文面向 RTLauncher Theme 开发者，说明 `@rtlauncher/theme-sdk` 的公开接口、生命周期、返回值和资源清理规则。开始第一个 Theme 前，请先阅读 [Theme 开发者快速上手](./DEVELOPER-GUIDE.zh-CN.md)。

## 1. 导入与版本

Theme 入口使用 `defineTheme` 声明定义：

```tsx
import {
  defineTheme,
  type JsonValue,
  type ThemeContext,
  type ThemeRouteComponentProps,
  type ThemeSlotComponentProps,
} from "@rtlauncher/theme-sdk";
```

包根目录公开以下常用类型：

| 导出 | 用途 |
| --- | --- |
| `defineTheme` | 保留 Theme 定义的类型信息。 |
| `ThemeDefinition` | Theme 的 ID、版本、API 版本和 `setup`。 |
| `ThemeContext` | `setup` 收到的全部主程序能力。 |
| `ThemeLifecycle` | `activate`、`deactivate` 和 `dispose`。 |
| `RTLauncherThemeSDK` | `context.sdk` 的稳定服务接口。 |
| `ThemeManifest` | 已校验的清单结构。 |
| `ThemeRouteComponentProps` | Theme 路由组件的参数。 |
| `ThemeSlotComponentProps<T>` | Theme 插槽组件的数据和子内容。 |
| `CoreRouteId`、`CoreSlotId` | 主程序公开的核心路由和插槽 ID。 |
| `JsonValue` | SDK 存储、事件和设置允许的数据类型。 |

`ThemeDefinition.apiVersion` 使用主版本兼容规则。API 1.x Theme 必须声明 `apiVersion: "1.0.0"` 或其他兼容的 1.x 版本，并在清单中声明 `engines.themeApi`。

Theme 定义中的 `id` 和 `version` 必须与 `manifest.json` 完全一致。主程序会拒绝元数据不一致的 Bundle。

## 2. `ThemeContext` 总览

`setup(context)` 收到一个 `ThemeContext`：

| 字段 | 类型 | 作用 |
| --- | --- | --- |
| `manifest` | `ThemeManifest` | 当前 Theme 的只读清单。 |
| `runtime` | `ThemeRuntimeInfo` | 主程序版本、平台、开发模式和准备时的活动 Theme。 |
| `sdk` | `RTLauncherThemeSDK` | 账号、实例、启动、下载和其他稳定服务。 |
| `routes` | `ThemeRouteRegistryAPI` | 替换、包装或新增路由。 |
| `slots` | `ThemeSlotRegistryAPI` | 向公开插槽注册组件。 |
| `assets` | `ThemeAssetService` | 读取 Theme 包内资源并释放 URL。 |
| `settings` | `ThemeSettingsService` | 读写当前 Theme 自己的配置。 |
| `events` | `ThemeEventService` | 发送和订阅带 Theme 命名空间的事件。 |
| `logger` | `ThemeLogger` | 输出带 Theme ID 前缀的日志。 |

`context.settings` 和 `context.sdk.settings` 不是同一个服务：

- `context.settings` 只保存当前 Theme 的数据。
- `context.sdk.settings` 读取和修改 RTLauncher 的公开设置。

`context.events` 与 `context.sdk.events` 指向同一个 Theme 专用事件服务。

## 3. 生命周期与资源清理

```ts
export default defineTheme({
  id: "com.example.my-theme",
  version: "1.0.0",
  apiVersion: "1.0.0",
  async setup(context) {
    const removeSlot = context.slots.register({
      id: "my-theme.page-action",
      target: "page.header.actions",
      mode: "after",
      component: () => null,
    });
    const stopEvents = context.events.on(
      "com.example.my-theme:refresh",
      () => context.logger.info("Refresh requested."),
    );

    return {
      async activate({ previousThemeId, signal }) {
        if (signal.aborted) return;
        context.logger.info("Theme activated.", { previousThemeId });
      },
      async deactivate({ nextThemeId, signal }) {
        if (signal.aborted) return;
        context.logger.info("Theme deactivated.", { nextThemeId });
      },
      dispose() {
        stopEvents();
        removeSlot();
      },
    };
  },
});
```

生命周期顺序如下：

1. `setup(context)` 注册能力并返回可选生命周期对象。
2. `activate(event)` 在主程序发布新活动 Theme 前运行。
3. `deactivate(event)` 在下一个 Theme 已发布后运行。
4. `dispose()` 在该 Theme 代次被移除时运行。

当前 1.0 主程序默认给 `setup` 和 `activate` 3 秒，给 `deactivate` 和 `dispose` 2 秒。`activate` 与 `deactivate` 收到 `AbortSignal`；超时时主程序会中止信号并继续故障恢复流程。

所有注册和订阅函数都返回可重复调用的取消函数。请在 `dispose` 中清理事件订阅、资源 URL、计时器和你自己创建的全局监听器。路由与插槽会按 Theme 代次隔离，但显式清理仍能让重新加载行为更清楚。

## 4. 路由 API

### `context.routes.override`

替换或包装一个核心路由：

```ts
const remove = context.routes.override({
  id: "my-theme.home",
  target: "core.home",
  mode: "wrap",
  component: HomeFrame,
});
```

参数：

| 字段 | 说明 |
| --- | --- |
| `id` | 当前 Theme 代次内唯一的贡献 ID。 |
| `target` | 一个公开的 `CoreRouteId`。 |
| `mode` | `replace` 或 `wrap`。 |
| `component` | 接收 `ThemeRouteComponentProps` 的 React 组件。 |

同一 Theme 不能为同一个核心路由注册两个 `replace`。可以注册多个 `wrap`。

`ThemeRouteComponentProps` 包含：

- `routeId`：当前路由 ID。
- `params`：只读路径参数。
- `search`：`URLSearchParams`。
- `children`：被包装的内置内容；`wrap` 组件应渲染它。

### `context.routes.add`

新增 Theme 本地页面：

```ts
const remove = context.routes.add({
  id: "my-theme.about",
  path: "/theme/my-theme/about",
  component: AboutPage,
});
```

新增路径必须位于 `/theme/` 下。相同 Theme 代次内不能重复使用路径或贡献 ID。注册函数返回取消函数。

## 5. 插槽 API

### `context.slots.register`

```tsx
type HeaderData = { title?: string };

function HeaderAction({ slotId, data, children }: ThemeSlotComponentProps<HeaderData>) {
  return (
    <div data-slot={slotId}>
      {children}
      <span>{data.title}</span>
    </div>
  );
}

const remove = context.slots.register<HeaderData>({
  id: "my-theme.header-action",
  target: "page.header.actions",
  mode: "after",
  order: 100,
  component: HeaderAction,
});
```

插槽模式：

| 模式 | 行为 |
| --- | --- |
| `replace` | 替换内置插槽内容；同一 Theme 和目标只能注册一个。 |
| `before` | 在内置内容前渲染。 |
| `after` | 在内置内容后渲染。 |
| `wrap` | 包装当前内容，组件通过 `children` 渲染被包装内容。 |

`order` 较小的贡献先处理。`order` 相同时，先注册的贡献先处理。`data` 的实际结构由目标插槽定义；使用泛型声明你需要的字段，并对可选字段提供回退值。

不要尝试替换原生窗口关闭、最小化、最大化控件或内置 Theme 恢复控件。这些控件位于可替换插槽之外。

## 6. 资源 API

### `context.assets.url(path)`

读取 Theme 包内文件并返回可用于 `src` 或 CSS 的 URL：

```tsx
let logoUrl: string | undefined;

async function loadLogo(context: ThemeContext) {
  logoUrl = await context.assets.url("assets/logo.png");
}

function Logo() {
  return logoUrl ? <img src={logoUrl} alt="" /> : null;
}
```

路径必须是安全的包内相对路径。不要传入绝对路径、URL、反斜杠或父目录跳转。

### `context.assets.release(url)`

不再使用资源时释放 URL：

```ts
dispose() {
  if (logoUrl) context.assets.release(logoUrl);
}
```

不要释放不是由当前 Theme 的 `assets.url` 返回的 URL。

## 7. Theme 专用设置

`context.settings` 保存一个属于当前 Theme ID 的 JSON 值。

```ts
type ThemePreferences = {
  density: "comfortable" | "compact";
  showGreeting: boolean;
};

const defaults: ThemePreferences = {
  density: "comfortable",
  showGreeting: true,
};

const preferences = await context.settings.get<ThemePreferences>();
await context.settings.update<ThemePreferences>({
  ...defaults,
  ...preferences,
  density: "compact",
});
```

| 方法 | 返回值 | 说明 |
| --- | --- | --- |
| `get<T>()` | `Promise<T>` | 读取完整值；没有数据时当前实现返回空对象。 |
| `update<T>(value)` | `Promise<T>` | 用新值替换完整值，并通知订阅者。 |
| `reset()` | `Promise<void>` | 删除当前 Theme 的设置并通知空对象。 |
| `subscribe(listener)` | 取消函数 | 值变化时接收新的 `JsonValue`。 |
| `registerMigration(migration)` | 取消函数 | 为未来设置迁移预留；1.0 当前不会执行迁移函数。 |

请在读取后与默认值合并，不要假设所有字段都存在。设置只能包含 `JsonValue`，不能保存函数、`Date`、`Map`、循环引用或二进制对象。

## 8. Theme 事件

事件名必须使用完整 Theme ID 作为前缀：

```ts
const eventName = "com.example.my-theme:panel.changed";
const stop = context.events.on<{ panel: string }>(eventName, ({ panel }) => {
  context.logger.debug("Panel changed.", { panel });
});

context.events.emit(eventName, { panel: "home" });
```

规则：

- `emit` 的事件名必须以 `${themeId}:` 开头。
- payload 必须是 `JsonValue`。
- 一个监听器抛错不会阻止同一事件的其他监听器。
- `on` 返回取消函数；请在 `dispose` 中调用它。
- Theme 不能用 `emit` 冒充另一个 Theme 或 `core:` 事件。

事件只在当前应用进程内传递。它不是持久化存储，也不是跨设备通信机制。

## 9. 日志

```ts
context.logger.debug("Preparing view.");
context.logger.info("Theme activated.", { version: context.manifest.version });
context.logger.warn("Optional asset is missing.", { path: "assets/banner.png" });
context.logger.error("Request failed.", { code: "NETWORK_ERROR" });
```

日志方法是 `debug`、`info`、`warn` 和 `error`。第二个参数必须是 `JsonValue`。主程序会自动添加 `[Theme <id>]` 前缀。

不要记录访问令牌、密码、私有路径或其他秘密数据。

## 10. 稳定 SDK 服务

### `sdk.accounts`

```ts
const accounts = await context.sdk.accounts.list();
const active = await context.sdk.accounts.getActive();
if (accounts[0] && accounts[0].id !== active?.id) {
  await context.sdk.accounts.setActive(accounts[0].id);
}
```

| 方法 | 返回值 | 说明 |
| --- | --- | --- |
| `list()` | `Promise<readonly ThemeAccountSummary[]>` | 返回可显示的账号摘要。 |
| `getActive()` | `Promise<ThemeAccountSummary \| null>` | 返回当前账号。 |
| `setActive(accountId)` | `Promise<void>` | 选择一个已存在账号。 |

账号摘要包含 `id`、`name`、可选 `uuid`、`authType` 和可选 `skinUrl`。SDK 不会返回访问令牌或 Yggdrasil 私有配置。

### `sdk.instances`

```ts
const instances = await context.sdk.instances.list("/path/to/minecraft/versions");
if (instances[0]) await context.sdk.instances.select(instances[0].name);
```

`list(instancesPath)` 扫描指定实例目录。虽然类型允许省略路径，但当前 1.0 实现要求非空路径，否则抛出 `THEME_SDK_ARGUMENT_INVALID`。返回摘要包含 `name`、可选 `minecraftVersion`、`loader`、`path` 和 `modsCount`。

`select(instanceId)` 把指定实例设为启动配置中的当前版本。请传入主程序返回的实例标识；当前扫描结果使用 `name`。

### `sdk.launch`

```ts
if (context.sdk.launch.getStatus() === "idle") {
  await context.sdk.launch.start();
}
```

| 方法 | 返回值 | 说明 |
| --- | --- | --- |
| `start(overrides?)` | `Promise<void>` | 使用当前配置启动游戏，可传 JSON 覆盖项。 |
| `stop()` | `Promise<void>` | 停止或取消当前启动流程。 |
| `getStatus()` | `string` | 同步读取当前启动状态。 |

除非主程序文档明确公开某个覆盖字段，否则不要依赖内部启动配置键。启动按钮应在调用期间防止重复提交。

### `sdk.downloads`

```ts
const tasks = context.sdk.downloads.list();
const activeTask = tasks.find((task) => task.status === "running");
if (activeTask) await context.sdk.downloads.cancel(activeTask.id);
```

`list()` 同步返回下载任务快照。任务包含 `id`、`label`、`status` 和可选 `progress`。`cancel(taskId)` 取消指定任务。不要根据数组对象引用判断变化；需要刷新时重新调用 `list()`。

### `sdk.router`

```ts
await context.sdk.router.navigate("/settings");
context.sdk.router.back();
const location = context.sdk.router.getLocation();
```

`navigate(target)` 使用 RTLauncher 路由器导航，`back()` 返回历史记录上一项，`getLocation()` 同步返回当前路径。Theme 自己新增的页面应使用 `/theme/` 命名空间。

### `sdk.settings`

```ts
const readAppearance = () => context.sdk.settings.get().appearance;
const stop = context.sdk.settings.subscribe(() => {
  context.logger.debug("Launcher settings changed.");
});

await context.sdk.settings.update({
  appearance: { fontSize: "large" },
});
```

| 方法 | 返回值 | 说明 |
| --- | --- | --- |
| `get()` | 只读对象 | 同步读取 RTLauncher 设置快照。 |
| `update(patch)` | `Promise<void>` | 更新公开设置分区。 |
| `subscribe(listener)` | 取消函数 | 设置变化时通知；监听器不接收参数。 |

当前 1.0 主程序只接受 `general` 和 `appearance` 分区补丁。先读取现有分区并保留不需要修改的字段。不要保存 Theme 私有数据到此服务；请使用 `context.settings` 或 `sdk.storage`。

### `sdk.i18n`

```ts
const label = context.sdk.i18n.t("settings.title");
const locale = context.sdk.i18n.getLocale();
```

`t(key)` 翻译主程序已有键，`getLocale()` 返回当前语言标识。Theme 自己的文案应由 Theme 管理；不要依赖未公开或可能删除的内部翻译键。

### `sdk.ui`

```ts
const accepted = await context.sdk.ui.confirm("Apply this Theme action?");
if (accepted) context.sdk.ui.toast("Action complete.");
```

`toast(message)` 请求显示短消息并返回通知 ID。`confirm(message)` 返回用户选择。不要用连续确认框阻塞主程序交互。

### `sdk.storage`

```ts
type CacheRecord = { lastTab: string; visits: number };

await context.sdk.storage.set("ui.cache", {
  lastTab: "home",
  visits: 3,
});
const cache = await context.sdk.storage.get<CacheRecord>("ui.cache");
await context.sdk.storage.remove("ui.cache");
```

存储键只能包含英文字母、数字、点、下划线和连字符。SDK 会自动按 Theme ID 添加命名空间，因此两个 Theme 可以使用相同的逻辑键。

值必须是 `JsonValue`。不存在的键返回 `null`。无效 JSON 会抛出 `THEME_STORAGE_VALUE_INVALID`。此存储位于本机前端环境，不适合保存密码、令牌或大型二进制数据。

### `sdk.events`

接口和 `context.events` 相同。请参阅 [Theme 事件](#8-theme-事件)。

### `sdk.platform`

```ts
const { os, appVersion, themeApiVersion } = context.sdk.platform;
```

`os` 是 `windows`、`macos` 或 `linux`。平台对象是只读快照。使用能力检测决定功能是否可用，不要只根据操作系统字符串推断未公开能力。

### `sdk.unsafe.invoke`

`unsafe.invoke` 是受限的原生命令入口，不属于稳定兼容层。使用它需要同时满足两个条件：

1. Theme 在清单 `permissions.unsafeCommands` 中请求命令。
2. RTLauncher 主程序允许该命令。

```json
{
  "permissions": {
    "unsafeCommands": ["get_system_info"]
  },
  "disclosures": [
    "读取操作系统和硬件摘要，用于显示诊断信息。"
  ]
}
```

```ts
const systemInfo = await context.sdk.unsafe.invoke<Record<string, unknown>>(
  "get_system_info",
);
```

当前 1.0 主程序允许：

- `get_launcher_paths_config`
- `get_system_info`
- `get_system_memory`

清单请求不会自动获得权限。不在主程序允许列表中的命令会抛出 `THEME_UNSAFE_COMMAND_DENIED`，且不会调用原生命令。原生命令执行失败会包装为 `THEME_UNSAFE_INVOKE_FAILED`。

不要通过 `unsafe.invoke` 调用 `theme_set_active`、文件写入、账号认证或其他未批准命令。新命令必须先经过主程序安全审查和协议设计。

## 11. 错误处理

SDK 错误可能包含以下字段：

```ts
type ThemeSDKErrorLike = Error & {
  code?: string;
  retryable?: boolean;
  details?: Readonly<Record<string, unknown>>;
};

try {
  await context.sdk.unsafe.invoke("get_system_info");
} catch (error) {
  const sdkError = error as ThemeSDKErrorLike;
  context.logger.error("SDK request failed.", {
    code: sdkError.code ?? "UNKNOWN",
    message: sdkError.message,
    retryable: sdkError.retryable ?? false,
  });
}
```

常见错误码：

| 错误码 | 含义 |
| --- | --- |
| `THEME_SDK_ARGUMENT_INVALID` | 必需参数为空或格式错误。 |
| `THEME_STORAGE_KEY_INVALID` | 存储键包含不允许的字符。 |
| `THEME_STORAGE_VALUE_INVALID` | 已保存内容不是有效 JSON。 |
| `THEME_UNSAFE_COMMAND_DENIED` | Theme 未请求命令或主程序未允许命令。 |
| `THEME_UNSAFE_INVOKE_FAILED` | 已允许的原生命令执行失败。 |

只在 `retryable === true` 或你明确知道操作可安全重试时重试。不要向用户显示原始堆栈、内部路径或可能含有敏感数据的参数。

## 12. 完整的清理模式

下面的模式集中管理 `setup` 创建的资源：

```ts
setup(context) {
  const disposers: Array<() => void> = [];
  let previewUrl: string | undefined;
  let disposed = false;

  disposers.push(context.events.on(
    `${context.manifest.id}:refresh`,
    () => context.logger.debug("Refresh requested."),
  ));

  disposers.push(context.sdk.settings.subscribe(() => {
    context.events.emit(`${context.manifest.id}:launcher-settings.changed`, null);
  }));

  void context.assets.url("assets/preview.png").then((url) => {
    if (disposed) {
      context.assets.release(url);
      return;
    }
    previewUrl = url;
  });

  return {
    dispose() {
      disposed = true;
      disposers.splice(0).forEach((dispose) => dispose());
      if (previewUrl) context.assets.release(previewUrl);
    },
  };
}
```

异步资源可能在 `dispose` 后完成。生产 Theme 应同时使用布尔状态或 `AbortController`，并在迟到结果到达时立即释放资源。

## 13. 兼容性建议

- 只使用 `@rtlauncher/theme-sdk`、`@rtlauncher/theme-ui` 和文档明确公开的能力。
- 不要读取 `window.__RTL_THEME_HOST__` 或直接调用内部 Tauri 命令。
- 对可选字段提供默认值。
- 把私有清单数据放在带反向域名键的 `extensions` 中。
- 新功能先检测 API 版本或能力，再提供回退路径。
- 重新加载、停用和切换回内置 Theme 后，确认没有遗留资源。

协议入口、路由 ID 和插槽 ID 的完整列表位于 [Theme API 1.0](./README.zh-CN.md)。
