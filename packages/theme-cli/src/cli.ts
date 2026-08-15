import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  stat,
  writeFile,
} from "node:fs/promises";
import { basename, dirname, isAbsolute, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { build, type Plugin } from "esbuild";
import { strFromU8, unzipSync, zipSync, type Zippable } from "fflate";

export interface ThemeValidationIssue {
  code: string;
  field: string;
  message: string;
}

interface AuthorManifest {
  schemaVersion: string;
  id: string;
  name: string;
  version: string;
  author: { name: string; url?: string };
  engines: { rtlauncher: string; themeApi: string; themeUi?: string };
  entry: { script: string; style?: string };
  supports: { colorSchemes: string[]; locales?: string[]; userOverrides?: string[] };
  icon?: string;
  previews?: string[];
  integrity?: { algorithm: "sha256"; files: Record<string, string> };
  [key: string]: unknown;
}

export interface ThemeBuildResult {
  manifestPath: string;
  scriptPath: string;
  stylePath?: string;
}

export interface ThemeArchiveInspection {
  manifest: AuthorManifest;
  files: string[];
  integrityValid: boolean;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function safePath(value: unknown): value is string {
  if (typeof value !== "string" || !value || isAbsolute(value) || value.includes("\\")) {
    return false;
  }
  return value.split("/").every((part) => part !== "" && part !== "." && part !== "..");
}

export function validateThemeManifest(value: unknown): ThemeValidationIssue[] {
  const issues: ThemeValidationIssue[] = [];
  const manifest = isRecord(value) ? value : {};
  const add = (code: string, field: string, message: string) => {
    issues.push({ code, field, message });
  };
  if (typeof manifest.schemaVersion !== "string" || !manifest.schemaVersion.startsWith("1.")) {
    add("THEME_SCHEMA_INVALID", "schemaVersion", "schemaVersion must use major version 1.");
  }
  if (typeof manifest.id !== "string"
    || !/^[a-z0-9]+(?:[.-][a-z0-9]+)+$/.test(manifest.id)
    || manifest.id.startsWith("builtin.")) {
    add("THEME_ID_INVALID", "id", "id must be a lowercase reverse-domain identifier.");
  }
  if (typeof manifest.name !== "string" || !manifest.name.trim()) {
    add("THEME_NAME_INVALID", "name", "name is required.");
  }
  if (typeof manifest.version !== "string" || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(manifest.version)) {
    add("THEME_VERSION_INVALID", "version", "version must be SemVer.");
  }
  const entry = isRecord(manifest.entry) ? manifest.entry : {};
  if (!safePath(entry.script)) {
    add("THEME_PATH_INVALID", "entry.script", "entry.script must stay inside the package.");
  }
  if (entry.style !== undefined && !safePath(entry.style)) {
    add("THEME_PATH_INVALID", "entry.style", "entry.style must stay inside the package.");
  }
  const author = isRecord(manifest.author) ? manifest.author : {};
  if (typeof author.name !== "string" || !author.name.trim()) {
    add("THEME_AUTHOR_INVALID", "author.name", "author.name is required.");
  }
  const engines = isRecord(manifest.engines) ? manifest.engines : {};
  if (typeof engines.rtlauncher !== "string" || typeof engines.themeApi !== "string") {
    add("THEME_ENGINES_INVALID", "engines", "rtlauncher and themeApi requirements are required.");
  }
  const supports = isRecord(manifest.supports) ? manifest.supports : {};
  if (!Array.isArray(supports.colorSchemes)
    || supports.colorSchemes.length === 0
    || supports.colorSchemes.some((item) => item !== "light" && item !== "dark")) {
    add("THEME_SUPPORTS_INVALID", "supports.colorSchemes", "colorSchemes must contain light or dark.");
  }
  return issues;
}

export function scopeThemeCss(themeId: string, css: string): string {
  return `@scope ([data-rtl-theme="${themeId}"]) {\n${css.trim()}\n}\n`;
}

function sha256(content: Uint8Array): string {
  return `sha256-${createHash("sha256").update(content).digest("hex")}`;
}

function hostModules(): Plugin {
  const reactNames = [
    "Children", "Component", "Fragment", "StrictMode", "Suspense", "cloneElement",
    "createContext", "createElement", "createRef", "forwardRef", "isValidElement",
    "lazy", "memo", "startTransition", "use", "useCallback", "useContext",
    "useDeferredValue", "useEffect", "useId", "useImperativeHandle", "useInsertionEffect",
    "useLayoutEffect", "useMemo", "useReducer", "useRef", "useState",
    "useSyncExternalStore", "useTransition",
  ];
  const uiNames = [
    "Alert", "AlertDescription", "AlertTitle", "Badge", "Button", "Card",
    "CardAction", "CardContent", "CardDescription", "CardFooter", "CardHeader",
    "CardTitle", "Input", "Label", "Progress", "Switch", "Textarea",
  ];
  return {
    name: "rtlauncher-theme-host",
    setup(pluginBuild) {
      pluginBuild.onResolve(
        { filter: /^(react|react\/jsx-runtime|@rtlauncher\/theme-ui|@rtlauncher\/theme-sdk)$/ },
        (args) => ({ path: args.path, namespace: "rtl-host" }),
      );
      pluginBuild.onLoad({ filter: /.*/, namespace: "rtl-host" }, (args) => {
        if (args.path === "react/jsx-runtime") {
          return { loader: "js", contents: `
            const runtime = globalThis.__RTL_THEME_HOST__.jsxRuntime;
            export const Fragment = runtime.Fragment;
            export const jsx = runtime.jsx;
            export const jsxs = runtime.jsxs;
          ` };
        }
        if (args.path === "react") {
          return { loader: "js", contents: `
            const React = globalThis.__RTL_THEME_HOST__.React;
            export default React;
            ${reactNames.map((name) => `export const ${name} = React.${name};`).join("\n")}
          ` };
        }
        if (args.path === "@rtlauncher/theme-ui") {
          return { loader: "js", contents: `
            const ui = globalThis.__RTL_THEME_HOST__.ui;
            ${uiNames.map((name) => `export const ${name} = ui.${name};`).join("\n")}
          ` };
        }
        return { loader: "js", contents: `
          export const defineTheme = (definition) => definition;
          export const THEME_API_VERSION = "1.0.0";
          export const BUILTIN_THEME_ID = "builtin.default";
        ` };
      });
    },
  };
}

async function readManifest(projectDirectory: string): Promise<AuthorManifest> {
  const path = join(projectDirectory, "manifest.json");
  const manifest = JSON.parse(await readFile(path, "utf8")) as AuthorManifest;
  const issues = validateThemeManifest(manifest);
  if (issues.length > 0) {
    throw new Error(issues.map((issue) => `${issue.code} ${issue.field}: ${issue.message}`).join("\n"));
  }
  return manifest;
}

export async function buildTheme(projectDirectory: string): Promise<ThemeBuildResult> {
  const root = resolve(projectDirectory);
  const manifest = await readManifest(root);
  const outputDirectory = join(root, "build");
  const distDirectory = join(outputDirectory, "dist");
  await mkdir(distDirectory, { recursive: true });
  const entryPath = resolve(root, manifest.entry.script);
  const scriptPath = join(distDirectory, "theme.js");
  await build({
    stdin: {
      contents: `
        import definition from ${JSON.stringify(entryPath)};
        globalThis.__RTL_THEME_REGISTER__(definition);
      `,
      resolveDir: root,
      sourcefile: "rtl-theme-entry.ts",
      loader: "ts",
    },
    outfile: scriptPath,
    bundle: true,
    format: "iife",
    platform: "browser",
    target: ["chrome120"],
    jsx: "automatic",
    plugins: [hostModules()],
    sourcemap: false,
    legalComments: "none",
  });

  let stylePath: string | undefined;
  if (manifest.entry.style) {
    stylePath = join(distDirectory, "theme.css");
    const css = await readFile(resolve(root, manifest.entry.style), "utf8");
    await writeFile(stylePath, scopeThemeCss(manifest.id, css));
  }

  const outputManifest: AuthorManifest = {
    ...manifest,
    entry: {
      script: "dist/theme.js",
      ...(stylePath ? { style: "dist/theme.css" } : {}),
    },
    integrity: {
      algorithm: "sha256",
      files: {
        "dist/theme.js": sha256(await readFile(scriptPath)),
        ...(stylePath ? { "dist/theme.css": sha256(await readFile(stylePath)) } : {}),
      },
    },
  };
  const manifestPath = join(outputDirectory, "manifest.json");
  await writeFile(manifestPath, `${JSON.stringify(outputManifest, null, 2)}\n`);
  return { manifestPath, scriptPath, stylePath };
}

async function listFiles(root: string, relative = ""): Promise<string[]> {
  const directory = join(root, relative);
  const entries = await readdir(directory);
  const files: string[] = [];
  for (const name of entries.sort()) {
    const child = join(relative, name);
    if ((await stat(join(root, child))).isDirectory()) files.push(...await listFiles(root, child));
    else files.push(child.replaceAll("\\", "/"));
  }
  return files;
}

export async function packTheme(
  projectDirectory: string,
  outputPath?: string,
): Promise<string> {
  const root = resolve(projectDirectory);
  const buildDirectory = join(root, "build");
  const manifest = JSON.parse(await readFile(join(buildDirectory, "manifest.json"), "utf8")) as AuthorManifest;
  const files = await listFiles(buildDirectory);
  const archive: Zippable = {};
  const fixedTime = new Date(1980, 0, 1, 0, 0, 0, 0);
  for (const path of files) {
    archive[path] = [new Uint8Array(await readFile(join(buildDirectory, path))), { mtime: fixedTime }];
  }
  const target = resolve(outputPath ?? join(root, `${manifest.id}-${manifest.version}.rtltheme`));
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, zipSync(archive, { level: 9 }));
  return target;
}

export async function inspectThemeArchive(path: string): Promise<ThemeArchiveInspection> {
  const archive = unzipSync(new Uint8Array(await readFile(path)));
  const files = Object.keys(archive).sort();
  const manifestFile = archive["manifest.json"];
  if (!manifestFile) throw new Error("THEME_MANIFEST_MISSING manifest.json is missing.");
  const manifest = JSON.parse(strFromU8(manifestFile)) as AuthorManifest;
  const integrityValid = Object.entries(manifest.integrity?.files ?? {}).every(
    ([file, expected]) => archive[file] !== undefined && sha256(archive[file]) === expected,
  );
  return { manifest, files, integrityValid };
}

async function run(argv: string[]): Promise<void> {
  const [command, path = ".", output] = argv;
  if (command === "validate") {
    const manifestPath = basename(path) === "manifest.json" ? path : join(path, "manifest.json");
    const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
    const issues = validateThemeManifest(manifest);
    if (issues.length) throw new Error(JSON.stringify(issues, null, 2));
    console.log("Theme manifest is valid.");
    return;
  }
  if (command === "build") {
    console.log(JSON.stringify(await buildTheme(path), null, 2));
    return;
  }
  if (command === "pack") {
    console.log(await packTheme(path, output));
    return;
  }
  if (command === "inspect") {
    console.log(JSON.stringify(await inspectThemeArchive(path), null, 2));
    return;
  }
  throw new Error("Usage: rtl-theme <validate|build|pack|inspect> [path] [output]");
}

const invokedAsExecutable = process.argv[1]
  && (pathToFileURL(process.argv[1]).href === import.meta.url
    || import.meta.url.endsWith("/dist/cli.js"));

if (invokedAsExecutable) {
  run(process.argv.slice(2)).catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
