import { X, Minus, Maximize, Info, Puzzle, FileCode2, Globe } from "lucide-react";
import { WindowMinimise, WindowToggleMaximise, Quit } from "../../wailsjs/runtime/runtime";
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem } from "@/components/ui/dropdown-menu";
import { openExternal } from "@/lib/utils";
const INFO_LINKS = [
    { label: "Chrome Extension", icon: Puzzle, url: "https://chromewebstore.google.com/detail/cboceolmgkoobpfjiojkigmihijfgmdo" },
    { label: "Userscript", icon: FileCode2, url: "https://greasyfork.org/en/scripts/523157" },
    { label: "Website", icon: Globe, url: "https://mediabatchdl.com" },
];
export function TitleBar() {
    const handleMinimize = () => {
        WindowMinimise();
    };
    const handleMaximize = () => {
        WindowToggleMaximise();
    };
    const handleClose = () => {
        Quit();
    };
    return (<>

      <div className="fixed top-0 left-14 right-0 h-10 z-40 bg-background/80 backdrop-blur-sm" style={{ "--wails-draggable": "drag" } as React.CSSProperties} onDoubleClick={handleMaximize}/>


      <div className="fixed top-1.5 right-2 z-50 flex h-7 gap-0.5" style={{ "--wails-draggable": "no-drag" } as React.CSSProperties}>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="w-8 h-7 flex items-center justify-center hover:bg-muted transition-colors rounded" aria-label="Info">
              <Info className="w-3.5 h-3.5"/>
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            {INFO_LINKS.map(({ label, icon: Icon, url }) => (<DropdownMenuItem key={label} onClick={() => openExternal(url)} className="gap-2 cursor-pointer">
              <Icon className="w-4 h-4 text-muted-foreground"/>
              {label}
            </DropdownMenuItem>))}
          </DropdownMenuContent>
        </DropdownMenu>
        <button onClick={handleMinimize} className="w-8 h-7 flex items-center justify-center hover:bg-muted transition-colors rounded" aria-label="Minimize">
          <Minus className="w-3.5 h-3.5"/>
        </button>
        <button onClick={handleMaximize} className="w-8 h-7 flex items-center justify-center hover:bg-muted transition-colors rounded" aria-label="Maximize">
          <Maximize className="w-3.5 h-3.5"/>
        </button>
        <button onClick={handleClose} className="w-8 h-7 flex items-center justify-center hover:bg-destructive hover:text-white transition-colors rounded" aria-label="Close">
          <X className="w-3.5 h-3.5"/>
        </button>
      </div>
    </>);
}
