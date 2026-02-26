"use client";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { INSTANCE_CARDS } from "@/constants/data";
import { cn } from "@/lib/utils";

const getGradientStyle = (colorFrom: string, colorTo: string) => {
  return `linear-gradient(to right, ${colorFrom}, ${colorTo})`;
};

export function InstanceCardGrid() {
  return (
    <div className="flex-1 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 auto-rows-[minmax(0,1fr)] h-full items-stretch min-h-0">
      {INSTANCE_CARDS.map((card) => (
        <Card
          key={card.id}
          className="group relative overflow-hidden hover:shadow-xl transition-all duration-300 cursor-pointer h-full flex flex-col border hover:border-primary/50"
        >
          {/* 悬停时的渐变背景 */}
          <div
            className="absolute inset-0 opacity-0 group-hover:opacity-15 transition-opacity duration-300"
            style={{
              background: getGradientStyle(card.colorFrom, card.colorTo),
            }}
          />
          {/* 卡片边框发光效果 */}
          <div
            className="absolute inset-0 rounded-2xl opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none"
            style={{
              boxShadow: `inset 0 0 20px ${card.colorFrom}, inset 0 0 40px ${card.colorTo}`,
            }}
          />
          <CardHeader className="relative">
            {/* 图标背景和容器 */}
            <div
              className={cn(
                "w-14 h-14 rounded-xl flex items-center justify-center mb-4 transition-all duration-300 group-hover:scale-110 group-hover:shadow-lg",
                card.iconBgColor
              )}
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                className={cn(
                  "h-7 w-7 transition-colors duration-300",
                  card.iconColor
                )}
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                {card.icon}
              </svg>
            </div>
            <CardTitle className="text-lg group-hover:text-primary transition-colors duration-300">
              {card.title}
            </CardTitle>
            <CardDescription className="text-xs group-hover:text-muted-foreground transition-colors duration-300">
              {card.description}
            </CardDescription>
          </CardHeader>
          <CardContent className="relative flex-1 flex flex-col justify-between">
            <div className="space-y-2">
              {card.stats.map((stat, index) => (
                <p
                  key={index}
                  className="text-xs text-muted-foreground group-hover:text-foreground/70 transition-colors duration-300"
                >
                  {stat}
                </p>
              ))}
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
