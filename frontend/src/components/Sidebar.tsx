import { Home, Settings, Bug, Database, LayoutGrid, Coffee, Github } from "lucide-react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import { openExternal } from "@/lib/utils";

export type PageType = "main" | "settings" | "debug" | "database";

interface SidebarProps {
  currentPage: PageType;
  onPageChange: (page: PageType) => void;
}

export function Sidebar({ currentPage, onPageChange }: SidebarProps) {
  const navItems = [
    { id: "main" as PageType, icon: Home, label: "Home" },
    { id: "settings" as PageType, icon: Settings, label: "Settings" },
    { id: "database" as PageType, icon: Database, label: "Saved Accounts" },
    { id: "debug" as PageType, icon: Bug, label: "Debug Logs" },
  ];

  return (
    <div className="fixed left-0 top-0 h-full w-14 bg-card border-r border-border flex flex-col items-center py-14 z-30">
      <div className="flex flex-col gap-2 flex-1">
        {navItems.map((item) => (
          <Tooltip key={item.id} delayDuration={0}>
            <TooltipTrigger asChild>
              <Button
                variant={currentPage === item.id ? "secondary" : "ghost"}
                size="icon"
                className="h-10 w-10"
                onClick={() => onPageChange(item.id)}
              >
                <item.icon className="h-5 w-5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="right">
              <p>{item.label}</p>
            </TooltipContent>
          </Tooltip>
        ))}
      </div>
      
      {/* Bottom icons */}
      <div className="mt-auto flex flex-col gap-2">
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-10 w-10"
              onClick={() => openExternal("https://github.com/afkarxyz/Twitter-X-Media-Batch-Downloader/issues")}
            >
              <Github className="h-5 w-5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            <p>Report Bug</p>
          </TooltipContent>
        </Tooltip>
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-10 w-10"
              onClick={() => openExternal("https://exyezed.cc/")}
            >
              <LayoutGrid className="h-5 w-5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            <p>Other Projects</p>
          </TooltipContent>
        </Tooltip>
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-10 w-10"
              onClick={() => openExternal("https://ko-fi.com/afkarxyz")}
            >
              <Coffee className="h-5 w-5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            <p>Support me on Ko-fi</p>
          </TooltipContent>
        </Tooltip>
      </div>
    </div>
  );
}
