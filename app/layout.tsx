import type { Metadata } from "next";
import { Geist, Geist_Mono, Figtree } from "next/font/google";
import "./globals.css";
import { TitleBar } from "@/components/title-bar";
import { Sidebar } from "@/components/sidebar";
import { ThemeProvider } from "@/components/theme-provider";
import { TooltipProvider } from "@/components/ui/tooltip";
import { AccountProvider } from "@/components/accounts/account-provider";
import { DownloadProvider } from "@/components/download/download-provider";
import { DownloadTaskList } from "@/components/download/download-task-list";
import { LaunchProvider } from "@/components/launch/launch-provider";
import { MultiplayerProvider } from "@/components/multiplayer/multiplayer-provider";

const figtree = Figtree({ subsets: ["latin"], variable: "--font-sans" });

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

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
    <html lang="zh-CN" className={figtree.variable} suppressHydrationWarning>
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased h-screen flex flex-col overflow-hidden bg-background`}
      >
        <ThemeProvider
          attribute="class"
          defaultTheme="system"
          enableSystem
          disableTransitionOnChange
        >
          <AccountProvider>
            <LaunchProvider>
            <MultiplayerProvider>
            <DownloadProvider>
              <TooltipProvider>
                <TitleBar />

                <div className="flex flex-1 overflow-hidden">
                  <Sidebar />
                  <main className="flex-1 overflow-hidden">{children}</main>
                </div>

                <DownloadTaskList />
              </TooltipProvider>
            </DownloadProvider>
            </MultiplayerProvider>
            </LaunchProvider>
          </AccountProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
