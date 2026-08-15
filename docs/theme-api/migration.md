# Theme API migration policy

This document lists required changes for Theme authors.

## API 1.0

API 1.0 is the first public Theme API.

- Use `schemaVersion: "1.0"`.
- Use `apiVersion: "1.0.0"` in the registered definition.
- Use `@rtlauncher/theme-sdk` for types and `defineTheme`.
- Use `@rtlauncher/theme-ui` for host UI components.
- Build with `rtl-theme build` so React uses the host bridge.
- Add `integrity` before archive installation. `rtl-theme build` does this.

## Future minor versions

A minor version can add an optional manifest field, SDK method, route, slot, or UI export. A 1.x Theme must not require an optional host feature without a version check.

## Future major versions

A major version can remove or change an existing contract. To migrate, update these values together:

1. `schemaVersion` in `manifest.json` when the manifest schema changes.
2. `engines.themeApi` in `manifest.json`.
3. `apiVersion` in the registered Theme definition.

Keep a separate build for each supported major version. RTLauncher rejects a bundle when the manifest, definition, and host major versions do not match.
