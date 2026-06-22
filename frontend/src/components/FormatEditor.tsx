import { useRef } from "react";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { RefreshCw } from "lucide-react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { type TemplateToken } from "@/lib/settings";
export function FormatEditor({ title, value, defaultValue, tokens, suffix, placeholder, render, onChange }: {
    title: string;
    value: string;
    defaultValue: string;
    tokens: TemplateToken[];
    suffix?: string;
    placeholder?: string;
    render: (template: string) => string;
    onChange: (next: string) => void;
}) {
    const inputRef = useRef<HTMLInputElement | null>(null);
    const insertToken = (token: string) => {
        const input = inputRef.current;
        let next: string;
        let caret: number;
        if (input && input.selectionStart !== null && input.selectionEnd !== null) {
            const start = input.selectionStart;
            const end = input.selectionEnd;
            next = value.slice(0, start) + token + value.slice(end);
            caret = start + token.length;
        }
        else {
            next = value + token;
            caret = next.length;
        }
        onChange(next);
        requestAnimationFrame(() => {
            if (input) {
                input.focus();
                input.setSelectionRange(caret, caret);
            }
        });
    };
    const preview = render(value) + (suffix ?? "");
    return (<div className="space-y-3">
      <Label className="text-sm font-semibold">{title}</Label>
      <div className="relative">
        <Input ref={inputRef} value={value} placeholder={placeholder} onChange={(e) => onChange(e.target.value)} className="font-mono text-sm pr-9"/>
        <button type="button" onClick={() => onChange(defaultValue)} className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors">
          <RefreshCw className="h-4 w-4"/>
        </button>
      </div>
      <div className="rounded-lg border bg-muted/40 px-3 py-2">
        <div className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">Preview</div>
        <div className="font-mono text-sm break-all">{preview || <span className="text-muted-foreground italic">empty</span>}</div>
      </div>
      <div className="flex flex-wrap gap-2">
        {tokens.map((token) => (<Tooltip key={token.key}>
            <TooltipTrigger asChild>
              <button type="button" onMouseDown={(e) => e.preventDefault()} onClick={() => insertToken(token.key)} className="rounded-md border bg-background px-2.5 py-1 text-xs font-mono text-muted-foreground transition-colors hover:text-foreground hover:border-primary/50">
                {token.key}
              </button>
            </TooltipTrigger>
            <TooltipContent side="top">
              <span className="font-mono">{token.example || "—"}</span>
            </TooltipContent>
          </Tooltip>))}
      </div>
    </div>);
}
