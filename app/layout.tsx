import type { Metadata } from "next";
import "./globals.css";
import { TitleBar } from "@/components/title-bar";
import { Sidebar } from "@/components/sidebar";
import { ThemeProvider } from "@/components/theme-provider";
import { SettingsProvider } from "@/components/settings/settings-provider";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AccountProvider } from "@/components/accounts/account-provider";
import { DownloadProvider } from "@/components/download/download-provider";
import { DownloadTaskList } from "@/components/download/download-task-list";
import { LaunchProvider } from "@/components/launch/launch-provider";
import { MultiplayerProvider } from "@/components/multiplayer/multiplayer-provider";
import { PageTransition } from "@/components/page-transition";

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
          <SettingsProvider>
            <AccountProvider>
              <LaunchProvider>
                <MultiplayerProvider>
                  <DownloadProvider>
                    <TooltipProvider>
                      <TitleBar />

                      <div className="flex flex-1 overflow-hidden">
                        <Sidebar />
                        <main className="flex-1 overflow-hidden">
                          <PageTransition>{children}</PageTransition>
                        </main>
                      </div>

                      <DownloadTaskList />
                    </TooltipProvider>
                  </DownloadProvider>
                </MultiplayerProvider>
              </LaunchProvider>
            </AccountProvider>
          </SettingsProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}