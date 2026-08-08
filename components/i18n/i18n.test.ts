import { readdirSync, readFileSync } from "node:fs";
import { extname, join, relative, sep } from "node:path";
import { describe, expect, it } from "vitest";
import ts from "typescript";
import enUS from "./locales/en-US.json";
import zhCN from "./locales/zh-CN.json";

function getLeafKeys(value: unknown, prefix = ""): string[] {
  if (!value || typeof value !== "object") return [prefix];

  return Object.entries(value).flatMap(([key, child]) =>
    getLeafKeys(child, prefix ? `${prefix}.${key}` : key),
  );
}

function getSourceFiles(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) return getSourceFiles(path);
    return [".ts", ".tsx"].includes(extname(entry.name)) ? [path] : [];
  });
}

describe("i18n catalogs", () => {
  it("keeps the same translation keys in each locale", () => {
    expect(getLeafKeys(enUS).sort()).toEqual(getLeafKeys(zhCN).sort());
  });

  it("keeps localized copy in JSON catalogs instead of source files", () => {
    const root = process.cwd();
    const i18nDirectory = `${join(root, "components", "i18n")}${sep}`;
    const files = [join(root, "app"), join(root, "components")]
      .flatMap(getSourceFiles)
      .filter((path) => !path.startsWith(i18nDirectory));
    const violations = files.flatMap((path) => {
      const source = readFileSync(path, "utf8");
      const sourceFile = ts.createSourceFile(
        path,
        source,
        ts.ScriptTarget.Latest,
        true,
        path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
      );
      const lines: number[] = [];
      const visit = (node: ts.Node) => {
        if (
          ts.isPropertyAssignment(node) &&
          ts.isStringLiteral(node.name) &&
          node.name.text === "zh-CN"
        ) {
          lines.push(sourceFile.getLineAndCharacterOfPosition(node.getStart()).line + 1);
        }
        ts.forEachChild(node, visit);
      };
      visit(sourceFile);
      return lines.map((line) => `${relative(root, path)}:${line}`);
    });

    expect(violations).toEqual([]);
  });
});
