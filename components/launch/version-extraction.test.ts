/**
 * 测试版本提取逻辑
 */

import { describe, expect, test } from "vitest";

// 复制自 version-selector-dialog.tsx 的函数
function extractMinecraftVersion(name: string): string {
  // 模式 1：快照版本格式，如 25w42a, 24w12a
  const snapshotRe = /^\d{2}w\d{2}[a-z]$/;
  if (snapshotRe.test(name)) {
    return name;
  }
  
  // 模式 2：以数字开头，后面跟 . 和数字，即 "x.y.z" 或 "x.y" 格式
  const standardRe = /^(\d+\.\d+(?:\.\d+)?)/;
  const match = name.match(standardRe);
  if (match) {
    return match[1];
  }
  
  // 模式 3：同时包含加载器版本和 MC 版本时，选择真实 MC 版本。
  const candidates = name.match(/\d+\.\d+(?:\.\d+)?/g) ?? [];
  const legacyRelease = candidates.find((candidate) => candidate.startsWith("1."));
  if (legacyRelease) {
    return legacyRelease;
  }
  const calendarRelease = [...candidates].reverse().find((candidate) => {
    const major = Number.parseInt(candidate.split(".")[0], 10);
    return major >= 20;
  });
  if (calendarRelease) {
    return calendarRelease;
  }
  
  // 模式 4：处理类似 "26.3-snapshot-5" 的格式
  const snapshotVerRe = /^(\d+\.\d+)-snapshot/;
  const snapshotMatch = name.match(snapshotVerRe);
  if (snapshotMatch) {
    return snapshotMatch[1];
  }
  
  // 模式 5：处理单个数字版本，如 "26" (用于新的快照格式)
  const singleVerRe = /^(\d+)(?:[-_.]|$)/;
  const singleMatch = name.match(singleVerRe);
  if (singleMatch) {
    const ver = singleMatch[1];
    const num = parseInt(ver, 10);
    // 只有当数字大于等于 20 时才认为是版本号（避免误判其他数字）
    if (num >= 20) {
      return ver;
    }
  }
  
  // fallback：原样返回
  return name;
}

// 复制自 use-minecraft-versions.ts 的函数
function isLikelySnapshot(versionId: string): boolean {
  // 快照版本通常以年份+周数开头，如 24w12a, 25w42a
  const weeklySnapshot = /^\d{2}w\d{2}[a-z]$/;
  // 包含 pre, rc, snapshot 等关键词
  const preRelease = /(pre|rc|snapshot|beta|alpha)/i;
  
  return weeklySnapshot.test(versionId) || preRelease.test(versionId);
}

describe('Version Extraction', () => {
  describe('extractMinecraftVersion', () => {
    test('标准版本格式', () => {
      expect(extractMinecraftVersion('1.21.1')).toBe('1.21.1');
      expect(extractMinecraftVersion('1.20.4')).toBe('1.20.4');
      expect(extractMinecraftVersion('1.21')).toBe('1.21');
    });

    test('加载器版本格式', () => {
      expect(extractMinecraftVersion('1.21.1-neoforge-4.0.1.20')).toBe('1.21.1');
      expect(extractMinecraftVersion('1.21.1-forge-52.0.0')).toBe('1.21.1');
      expect(extractMinecraftVersion('fabric-loader-0.15.0-1.21.1')).toBe('1.21.1');
      expect(extractMinecraftVersion('quilt-loader-0.25.0-1.21.1')).toBe('1.21.1');
      expect(extractMinecraftVersion('1.21.1-OptiFine_HD_U_I7_pre1')).toBe('1.21.1');
    });

    test('快照版本格式', () => {
      expect(extractMinecraftVersion('25w42a')).toBe('25w42a');
      expect(extractMinecraftVersion('24w12a')).toBe('24w12a');
      expect(extractMinecraftVersion('26.3-snapshot-5')).toBe('26.3');
      expect(extractMinecraftVersion('26')).toBe('26');
    });

    test('复杂格式', () => {
      expect(extractMinecraftVersion('1.20.1-fabric-0.15.11')).toBe('1.20.1');
      expect(extractMinecraftVersion('1.19.4-forge-45.2.0')).toBe('1.19.4');
    });

    test('边界情况', () => {
      // 低版本号不应被误判为版本号
      expect(extractMinecraftVersion('5-something')).toBe('5-something');
      // 高版本号应该被正确识别
      expect(extractMinecraftVersion('20')).toBe('20');
      expect(extractMinecraftVersion('21')).toBe('21');
    });
  });

  describe('isLikelySnapshot', () => {
    test('周快照格式', () => {
      expect(isLikelySnapshot('25w42a')).toBe(true);
      expect(isLikelySnapshot('24w12a')).toBe(true);
      expect(isLikelySnapshot('23w14a')).toBe(true);
    });

    test('预发布版本', () => {
      expect(isLikelySnapshot('1.21-pre1')).toBe(true);
      expect(isLikelySnapshot('1.21-rc1')).toBe(true);
      expect(isLikelySnapshot('1.21-beta')).toBe(true);
      expect(isLikelySnapshot('1.21-alpha')).toBe(true);
      expect(isLikelySnapshot('1.21-snapshot')).toBe(true);
    });

    test('正式版', () => {
      expect(isLikelySnapshot('1.21.1')).toBe(false);
      expect(isLikelySnapshot('1.20.4')).toBe(false);
      expect(isLikelySnapshot('1.21')).toBe(false);
    });

    test('大小写不敏感', () => {
      expect(isLikelySnapshot('1.21-PRE1')).toBe(true);
      expect(isLikelySnapshot('1.21-RC1')).toBe(true);
      expect(isLikelySnapshot('1.21-SNAPSHOT')).toBe(true);
    });
  });
});
