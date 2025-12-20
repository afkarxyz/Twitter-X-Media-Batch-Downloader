import { HomeIcon } from "@/components/ui/home";
import { SettingsIcon } from "@/components/ui/settings";
import { ArchiveIcon } from "@/components/ui/archive";
import { TerminalIcon } from "@/components/ui/terminal";
import { GithubIcon } from "@/components/ui/github";
import { BlocksIcon } from "@/components/ui/blocks";
import { CoffeeIcon } from "@/components/ui/coffee";
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
  return (
    <div className="fixed left-0 top-0 h-full w-14 bg-card border-r border-border flex flex-col items-center py-14 z-30">
      <div className="flex flex-col gap-2 flex-1">
        {/* Home */}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant={currentPage === "main" ? "secondary" : "ghost"}
              size="icon"
              className="h-10 w-10"
              onClick={() => onPageChange("main")}
            >
              <HomeIcon size={20} />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            <p>Home</p>
          </TooltipContent>
        </Tooltip>

        {/* Settings */}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant={currentPage === "settings" ? "secondary" : "ghost"}
              size="icon"
              className="h-10 w-10"
              onClick={() => onPageChange("settings")}
            >
              <SettingsIcon size={20} />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            <p>Settings</p>
          </TooltipContent>
        </Tooltip>

        {/* Database - using animated archive icon */}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant={currentPage === "database" ? "secondary" : "ghost"}
              size="icon"
              className="h-10 w-10"
              onClick={() => onPageChange("database")}
            >
              <ArchiveIcon size={20} />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            <p>Saved Accounts</p>
          </TooltipContent>
        </Tooltip>

        {/* Debug - using animated terminal icon */}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant={currentPage === "debug" ? "secondary" : "ghost"}
              size="icon"
              className="h-10 w-10"
              onClick={() => onPageChange("debug")}
            >
              <TerminalIcon size={20} />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right">
            <p>Debug Logs</p>
          </TooltipContent>
        </Tooltip>
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
              <GithubIcon size={20} />
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
              <BlocksIcon size={20} />
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
              <CoffeeIcon size={20} />
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
