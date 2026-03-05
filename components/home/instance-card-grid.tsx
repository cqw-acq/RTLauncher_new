"use client";

import { motion } from "framer-motion";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { INSTANCE_CARDS } from "@/constants/data";
import { cn } from "@/lib/utils";
import { staggerContainer, staggerItem } from "@/lib/motion";

export function InstanceCardGrid() {
  return (
    <motion.div
      className="flex-1 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 auto-rows-[minmax(0,1fr)] h-full items-stretch min-h-0"
      variants={staggerContainer}
      initial="initial"
      animate="animate"
    >
      {INSTANCE_CARDS.map((card) => (
        <motion.div key={card.id} variants={staggerItem} className="h-full">
          <Card className="shadow-sm hover:shadow-xl transition-shadow duration-300 cursor-pointer h-full flex flex-col border">
            <CardHeader>
              {/* 图标 */}
              <div
                className={cn(
                  "w-14 h-14 rounded-xl flex items-center justify-center mb-4",
                  card.iconBgColor
                )}
              >
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className={cn("h-7 w-7", card.iconColor)}
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  {card.icon}
                </svg>
              </div>
              <CardTitle className="text-lg">{card.title}</CardTitle>
              <CardDescription className="text-xs">{card.description}</CardDescription>
            </CardHeader>
            <CardContent className="flex-1 flex flex-col justify-between">
              <div className="space-y-2">
                {card.stats.map((stat, index) => (
                  <p key={index} className="text-xs text-muted-foreground">
                    {stat}
                  </p>
                ))}
              </div>
            </CardContent>
          </Card>
        </motion.div>
      ))}
    </motion.div>
  );
}
