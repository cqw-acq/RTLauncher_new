import type { NextConfig } from "next";

const isProd = process.env.NODE_ENV === "production";

const nextConfig: NextConfig = {
  turbopack: {
    root: process.cwd(),
  },
  output: "export",
  images: {
    unoptimized: true,
  },
  trailingSlash: true,
  // ===== 前端体积优化 =====
  productionBrowserSourceMaps: false,
  experimental: {
    optimizePackageImports: [
      "lucide-react",
      "@tauri-apps/api",
      "@tauri-apps/plugin-dialog",
      "class-variance-authority",
    ],
  },
  reactStrictMode: false,
  poweredByHeader: false,
  compress: true,
};

export default nextConfig;
