# RTLauncher Theme API 1.0

[简体中文](./README.zh-CN.md) | English

The Theme API lets a user replace selected React routes and UI slots. A Theme can also add a local route. RTLauncher keeps the built-in UI for each route or slot that the Theme does not replace.

## Security model

A Theme is trusted local code. It is not a sandbox. It can use the stable SDK and it can call any registered Tauri command through `sdk.unsafe.invoke`. Install a Theme only when you trust its author and source.

RTLauncher applies these package controls:

- It rejects absolute paths, parent traversal, duplicate ZIP entries, symbolic links, and unsafe file reads.
- It limits archive file count, per-file size, and total uncompressed size.
- It checks SHA-256 values before installation.
- It stages an installation and uses an atomic directory rename.
- It keeps the previous UI when setup or activation fails.
- It returns to the built-in Theme after three runtime errors in 30 seconds.

These controls protect package storage. They do not restrict Theme code after the user activates it.

## Package types

RTLauncher accepts two package types:

1. A `.rtltheme` ZIP archive for normal installation.
2. A source directory for development. RTLauncher reads this directory in place and reloads it after an entry file changes.

A built archive has this structure:

```text
theme.rtltheme
├── manifest.json
└── dist
    ├── theme.js
    └── theme.css
```

`manifest.json` must conform to [manifest.schema.json](./manifest.schema.json). An archive must include `integrity`. A development manifest can omit it because the CLI generates it during build.

## Compatibility policy

`schemaVersion`, `engines.themeApi`, and `ThemeDefinition.apiVersion` use major-version compatibility.

- A 1.x host accepts a 1.x manifest schema.
- A 1.x host accepts a Theme definition that uses API major version 1.
- A Theme must declare compatible RTLauncher and Theme API ranges.
- New optional fields can be added in a minor release.
- A breaking field or behavior change requires a new major version.

Put private metadata in `extensions`. Use a reverse-domain key to prevent a name conflict.

## Author workflow

Use Node.js and pnpm from the repository root:

```sh
pnpm install
pnpm --filter @rtlauncher/theme-cli build
pnpm --filter @rtlauncher/example-hello-theme run validate
pnpm --filter @rtlauncher/example-hello-theme run build
pnpm --filter @rtlauncher/example-hello-theme run pack
```

The CLI has four commands:

```text
rtl-theme validate <project-or-manifest>
rtl-theme build <project>
rtl-theme pack <project> [output.rtltheme]
rtl-theme inspect <file.rtltheme>
```

`build` creates a self-registering IIFE. It maps React, the JSX runtime, and `@rtlauncher/theme-ui` to the host bridge. It does not include a second React copy. It also wraps Theme CSS in an `@scope` rule and writes SHA-256 values.

See [the complete Hello Theme](../../examples/themes/hello-theme/README.md).

## Theme entry

Export one definition from the source entry:

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

The bundle must call `window.__RTL_THEME_REGISTER__` exactly once. The CLI adds this call. RTLauncher rejects a missing registration, a second registration, and an ID mismatch.

## Lifecycle

`setup(context)` registers routes and slots. It returns optional lifecycle functions.

- `activate(event)` runs before RTLauncher publishes the new active Theme.
- `deactivate(event)` runs after RTLauncher publishes the next Theme.
- `dispose()` releases Theme resources.
- Each activation and deactivation event has an `AbortSignal`.
- Setup, activation, deactivation, and disposal have time limits.

RTLauncher scopes registrations to one Theme generation. A reload prepares a new generation first. It removes the old generation only after the new generation is ready.

## Routes

A Theme can replace or wrap a core route. It can also add a local route.

| Core route ID | Built-in area |
| --- | --- |
| `core.home` | Home |
| `core.launch` | Launch |
| `core.download` | Downloads |
| `core.download.detail` | Download detail |
| `core.multiplayer` | Multiplayer |
| `core.tools` | Tools |
| `core.settings` | Settings |
| `core.game-settings` | Game settings |
| `core.instance.*` | Mods, worlds, resources, shaders, screenshots, and schematics |

Use `context.routes.override` for a core route. Use `context.routes.add` for a local path. A render error returns only that contribution to its built-in fallback.

## Slots

Slot modes are `replace`, `before`, `after`, and `wrap`. `order` controls two contributions in the same mode.

| Slot group | IDs |
| --- | --- |
| Title bar | `app.titlebar.leading`, `app.titlebar.center`, `app.titlebar.actions` |
| Sidebar | `app.sidebar`, `app.sidebar.header`, `app.sidebar.navigation`, `app.sidebar.footer` |
| Content | `app.content.before`, `app.content.after` |
| Page | `page.header`, `page.header.actions`, `page.footer` |
| Launch | `launch.primary-action` |

## Stable SDK

`context.sdk` provides these stable services:

| Service | Purpose |
| --- | --- |
| `accounts` | List accounts, read the active account, and select an account. Secret tokens are not returned. |
| `instances` | List and select game instances. |
| `launch` | Start or stop a game and read launch status. |
| `downloads` | List and cancel download tasks. |
| `router` | Navigate, go back, and read the current location. |
| `settings` | Read and update launcher settings. |
| `i18n` | Translate a host key and read the locale. |
| `ui` | Show a toast or confirmation. |
| `storage` | Store JSON in a Theme-specific namespace. |
| `events` | Emit and subscribe to Theme-specific events. |
| `platform` | Read OS and API version data. |

Other context services are `routes`, `slots`, `assets`, Theme settings, events, and a Theme-prefixed logger.

`sdk.unsafe.invoke(command, args)` is an explicit escape hatch. It has no stable compatibility guarantee. A Theme must list its use and effect in `disclosures`.

## Native host commands

The frontend host uses these Tauri commands:

```text
theme_list
theme_install_archive
theme_register_dev_directory
theme_remove
theme_read_text
theme_read_binary
theme_set_active
theme_mark_healthy
```

Do not call these commands directly from a normal Theme. Use the stable SDK unless a required operation has no stable service.

## User switching and recovery

The Appearance settings page can install, register, activate, reload, and remove a Theme. RTLauncher asks for trust confirmation before the first activation of each local Theme.

Activation is transactional. RTLauncher prepares the new bundle before it changes the active UI. A failed activation keeps the previous UI and state. A pending activation becomes healthy after a quiet period. If the app exits before confirmation, the native store returns to the last healthy Theme on the next start.
