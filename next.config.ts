import type { NextConfig } from "next";

const isProd = process.env.NODE_ENV === "production";

const internalHost = process.env.TAURI_DEV_HOST || "localhost";

const nextConfig: NextConfig = {
  output: "export",
  images: {
    unoptimized: true,
  },
  assetPrefix: isProd ? undefined : `http://${internalHost}:51515`,
  // 排除 RTL 目录，避免配置文件变化触发热重载
  watchOptions: {
    ignored: ['**/RTL/**', '**/node_modules/**', '**/.git/**'],
  },
};

export default nextConfig;
