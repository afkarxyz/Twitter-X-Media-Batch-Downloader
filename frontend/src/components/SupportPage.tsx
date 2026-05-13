import { useState } from "react";
import { Button } from "@/components/ui/button";
import { CircleCheck, Copy } from "lucide-react";
import { openExternal } from "@/lib/utils";
import KofiLogo from "@/assets/ko-fi.gif";
import KofiSvg from "@/assets/kofi_symbol.svg";
import UsdtBarcode from "@/assets/usdt.jpg";

const USDT_ADDRESS = "THnzAAwZgp2Sq5CAXLP2njQDhTvgZG9EWs";

export function SupportPage() {
    const [copiedUsdt, setCopiedUsdt] = useState(false);

    return (<div className="flex min-h-[70vh] items-center justify-center p-4">
      <div className="flex w-full max-w-3xl flex-col items-stretch rounded-xl border bg-card shadow-sm md:flex-row">
        <div className="flex flex-1 flex-col items-center justify-between space-y-6 border-b p-6 md:border-r md:border-b-0">
            <div className="flex flex-col items-center space-y-4">
              <div className="relative flex h-32 w-full items-center justify-center">
                <img src={KofiLogo} className="pointer-events-none absolute w-72" alt="Ko-fi"/>
              </div>
              <h4 className="font-semibold text-foreground">Support via Ko-fi</h4>
              <p className="px-4 text-center text-sm text-muted-foreground">
                Enjoying the project? You can support ongoing development by buying me a coffee.
              </p>
            </div>
            <Button className="h-10 w-full gap-2 bg-[#72a4f2] text-sm font-semibold text-white hover:bg-[#5f8cd6]" onClick={() => openExternal("https://ko-fi.com/afkarxyz")}>
              <img src={KofiSvg} className="h-5 w-5 shrink-0" alt="" aria-hidden="true"/>
              Support me on Ko-fi
            </Button>
          </div>

          <div className="flex flex-1 flex-col items-center justify-between space-y-6 p-6">
            <div className="flex w-full flex-col items-center space-y-4">
              <div className="flex h-32 items-center justify-center">
                <div className="rounded-xl border bg-white p-2 shadow-sm">
                  <img src={UsdtBarcode} className="h-24 w-24 object-contain" alt="USDT Barcode"/>
                </div>
              </div>
              <h4 className="font-semibold text-foreground">USDT (TRC20)</h4>
              <p className="px-4 text-center text-sm text-muted-foreground">
                Crypto donations are also accepted. Scan the QR code or copy the address.
              </p>
            </div>
            <div className="flex h-10 w-full items-center justify-between gap-2 rounded-lg border bg-muted/50 py-1.5 pr-1.5 pl-3">
              <code className="truncate text-xs font-mono text-muted-foreground" title={USDT_ADDRESS}>
                {USDT_ADDRESS}
              </code>
              <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0 hover:bg-background" onClick={() => {
            navigator.clipboard.writeText(USDT_ADDRESS);
            setCopiedUsdt(true);
            setTimeout(() => setCopiedUsdt(false), 500);
        }}>
                {copiedUsdt ? <CircleCheck className="h-3.5 w-3.5 text-green-500"/> : <Copy className="h-3.5 w-3.5"/>}
              </Button>
            </div>
          </div>
      </div>
    </div>);
}
