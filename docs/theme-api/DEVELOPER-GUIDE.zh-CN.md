# Theme 开发者快速上手

本指南说明如何在 RTLauncher 仓库中创建、调试和打包一个前端 Theme。完整协议请参阅 [Theme API 1.0](./README.zh-CN.md)，清单字段定义请参阅 [manifest.schema.json](./manifest.schema.json)。

> 当前 Theme SDK、UI 组件和 CLI 是仓库内工作区包。请先在 RTLauncher 仓库中开发 Theme。

## 1. 准备工作区

安装 Node.js、pnpm 和项目依赖，然后构建 Theme CLI：

```sh
pnpm install
pnpm --filter @rtlauncher/theme-cli build
```

最快的起点是复制 Hello Theme：

```sh
cp -R examples/themes/hello-theme examples/themes/my-theme
```

一个最小 Theme 目录如下：

```text
my-theme
├── manifest.json
├── package.json
└── src
    ├── theme.tsx
    └── theme.css
```

修改 `package.json` 中的包名，并把 `manifest.json` 与 `theme.tsx` 中的 Theme ID 和版本改成你自己的值。

## 2. 编写清单

开发目录中的 `manifest.json` 可以使用源码入口：

```json
{
  "schemaVersion": "1.0",
  "id": "com.example.my-theme",
  "name": "My Theme",
  "version": "1.0.0",
  "author": { "name": "Example" },
  "engines": {
    "rtlauncher": ">=1.0.0 <2.0.0",
    "themeApi": "^1.0.0",
    "themeUi": "^1.0.0"
  },
  "entry": {
    "script": "src/theme.tsx",
    "style": "src/theme.css"
  },
  "supports": {
    "colorSchemes": ["light", "dark"]
  }
}
```

请遵守以下规则：

- `id` 使用小写反向域名格式，且不能使用 `builtin.*`。
- `version` 使用完整 SemVer，例如 `1.2.0`。
- 文件路径必须位于 Theme 目录内，不能包含绝对路径、反斜杠、`.` 或 `..` 路径段。
- 贡献 ID 使用带命名空间的小写 ID，例如 `my-theme.page-action`。
- 新增页面的路径必须位于 `/theme/` 下。
- 开发清单可以省略 `integrity`；CLI 构建时会生成完整的 SHA-256 数据。

## 3. 编写入口

使用 `defineTheme` 导出一个 Theme 定义。下面的示例向页面标题栏添加一个按钮：

```tsx
import { Button } from "@rtlauncher/theme-ui";
import { defineTheme, type ThemeSlotComponentProps } from "@rtlauncher/theme-sdk";

function PageAction({ slotId }: ThemeSlotComponentProps) {
  return <Button onClick={() => window.alert(slotId)}>My Theme</Button>;
}

export default defineTheme({
  id: "com.example.my-theme",
  version: "1.0.0",
  apiVersion: "1.0.0",
  setup(context) {
    context.slots.register({
      id: "my-theme.page-action",
      target: "page.header.actions",
      mode: "after",
      order: 100,
      component: PageAction,
    });

    return {
      activate() {
        context.logger.info("My Theme is active.");
      },
    };
  },
});
```

`setup` 用于注册路由和插槽。需要释放计时器、订阅或其他资源时，请返回 `deactivate` 或 `dispose` 生命周期函数。

优先使用 `context.sdk` 提供的稳定接口。只有稳定 SDK 无法完成操作时，才使用 `sdk.unsafe.invoke`。每个不安全命令都必须在 `permissions.unsafeCommands` 中声明，并且仍受主程序允许列表限制。

## 4. 编写样式

Theme CSS 会在构建时放入以下作用域：

```css
@scope ([data-rtl-theme="com.example.my-theme"]) {
  /* Theme CSS */
}
```

源码 CSS 不需要手动添加这个作用域。请优先使用主程序提供的颜色变量和 `@rtlauncher/theme-ui` 组件，以保持浅色和深色模式兼容。

## 5. 校验、构建和打包

从仓库根目录运行：

```sh
pnpm --filter @rtlauncher/example-hello-theme validate
pnpm --filter @rtlauncher/example-hello-theme build
pnpm --filter @rtlauncher/example-hello-theme pack
```

如果你修改了示例包名，请在命令中使用新的包名。也可以直接调用已经构建的 CLI：

```sh
node packages/theme-cli/dist/cli.js validate examples/themes/my-theme
node packages/theme-cli/dist/cli.js build examples/themes/my-theme
node packages/theme-cli/dist/cli.js pack examples/themes/my-theme my-theme.rtltheme
node packages/theme-cli/dist/cli.js inspect my-theme.rtltheme
```

`build` 会生成 `build/manifest.json`、`build/dist/theme.js` 和可选的 `build/dist/theme.css`。`pack` 会生成可安装的 `.rtltheme` 归档。

`inspect` 输出中的 `integrityValid` 必须为 `true`。不要手动修改已经打包的文件；任何修改都会使 SHA-256 校验失败。

## 6. 在 RTLauncher 中调试

打开“设置 → 外观 → 前端 Theme”：

1. 完成一次构建，然后选择“添加开发目录”，并选择 `examples/themes/my-theme/build`。
2. 在 Theme 下拉列表中选择新 Theme，并确认信任提示。
3. 修改源码后先重新构建，再选择“重新加载”。
4. 测试完成后，用“安装 Theme”选择 `.rtltheme` 归档，验证正式安装流程。

RTLauncher 会按 Theme ID 和版本保存信任状态。修改版本后，用户需要重新确认信任。

如果 Theme 无法激活，主程序会保留原有界面。页面或插槽渲染失败时，对应区域会回退到内置内容。界面左下角的恢复按钮始终可以切换回内置 Theme。

## 7. 发布前检查

- Theme ID、版本和入口定义保持一致。
- `validate`、`build`、`pack` 和 `inspect` 均成功。
- `integrityValid` 为 `true`。
- 浅色与深色模式均可用。
- Theme 停用和重新加载后没有遗留订阅、计时器或 DOM 状态。
- 不替换系统窗口控制和内置恢复入口。
- `disclosures` 说明 Theme 的重要行为和不安全命令用途。
- 归档只包含运行所需文件，不包含密钥、令牌或本机路径。

可运行的完整示例位于 [examples/themes/hello-theme](../../examples/themes/hello-theme/README.md)。
