import type { Metadata } from "next";
import "./globals.css";
import { ThemeProvider } from "@/components/theme-provider";
import { SettingsProvider } from "@/components/settings/settings-provider";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AccountProvider } from "@/components/accounts/account-provider";
import { DownloadProvider } from "@/components/download/download-provider";
import { DownloadTaskList } from "@/components/download/download-task-list";
import { LaunchProvider } from "@/components/launch/launch-provider";
import { MultiplayerProvider } from "@/components/multiplayer/multiplayer-provider";
import { DeferredGlobalFeatures } from "@/components/global/deferred-global-features";
import { UIConfigProvider } from "@/components/ui-config/ui-config-provider";
import { ThemeRuntimeProvider } from "@/components/themes/theme-runtime-provider";
import { ThemeShell } from "@/components/themes/theme-shell";
import { StartupUpdateNotifier } from "@/components/settings/startup-update-notifier";

export const metadata: Metadata = {
  title: "RTLauncher",
  description: "RTLauncher Desktop App",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="zh-CN" suppressHydrationWarning>
      <body className="antialiased h-screen flex flex-col overflow-hidden bg-background">
        
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          <UIConfigProvider>
            <SettingsProvider>
              <AccountProvider>
                <LaunchProvider>
                  <MultiplayerProvider>
                    <DownloadProvider>
                      <TooltipProvider>
                        <ThemeRuntimeProvider>
                          {/* 非关键全局能力在首屏可交互后再加载 */}
                          <DeferredGlobalFeatures />
                          <StartupUpdateNotifier />
                          <ThemeShell>{children}</ThemeShell>
                          <DownloadTaskList />
                        </ThemeRuntimeProvider>
                      </TooltipProvider>
                    </DownloadProvider>
                  </MultiplayerProvider>
                </LaunchProvider>
              </AccountProvider>
            </SettingsProvider>
          </UIConfigProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
