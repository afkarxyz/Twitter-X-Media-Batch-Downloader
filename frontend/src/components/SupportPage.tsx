import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { CircleCheck, Copy } from "lucide-react";
import { openExternal } from "@/lib/utils";
import KofiLogo from "@/assets/ko-fi.gif";
import KofiSvg from "@/assets/kofi_symbol.svg";
import UsdtTronQr from "@/assets/usdt.jpg";
import UsdtEvmQr from "@/assets/usdt_evm.jpg";
import UsdcEvmQr from "@/assets/usdc.jpg";
type CryptoCoin = "usdt" | "usdc";
type CryptoNetwork = "ethereum" | "bsc" | "polygon" | "base" | "tron";
const EVM_ADDRESS = "0xB563a7F39770C151e2FacE26926081a00c5EF349";
const TRON_ADDRESS = "THnzAAwZgp2Sq5CAXLP2njQDhTvgZG9EWs";
const EVM_NETWORKS: Array<{
    value: CryptoNetwork;
    label: string;
    shortLabel: string;
}> = [
    { value: "bsc", label: "BNB Smart Chain (BEP20)", shortLabel: "BEP20" },
    { value: "polygon", label: "Polygon", shortLabel: "Polygon" },
    { value: "ethereum", label: "Ethereum (ERC20)", shortLabel: "ERC20" },
    { value: "base", label: "Base", shortLabel: "Base" },
];
const USDT_NETWORKS: Array<{
    value: CryptoNetwork;
    label: string;
    shortLabel: string;
}> = [
    { value: "ethereum", label: "Ethereum (ERC20)", shortLabel: "ERC20" },
    { value: "bsc", label: "BNB Smart Chain (BEP20)", shortLabel: "BEP20" },
    { value: "tron", label: "Tron (TRC20)", shortLabel: "TRC20" },
    { value: "polygon", label: "Polygon", shortLabel: "Polygon" },
];
export function SupportPage() {
    const [selectedCoin, setSelectedCoin] = useState<CryptoCoin>("usdt");
    const [selectedNetwork, setSelectedNetwork] = useState<CryptoNetwork>("tron");
    const [copiedAddress, setCopiedAddress] = useState(false);
    const networkOptions = selectedCoin === "usdt" ? USDT_NETWORKS : EVM_NETWORKS;
    const activeNetwork = networkOptions.find((network) => network.value === selectedNetwork) || EVM_NETWORKS[0];
    const isTron = selectedNetwork === "tron";
    const cryptoAddress = isTron ? TRON_ADDRESS : EVM_ADDRESS;
    const cryptoQr = isTron ? UsdtTronQr : selectedCoin === "usdt" ? UsdtEvmQr : UsdcEvmQr;
    const handleCoinChange = (coin: CryptoCoin) => {
        setSelectedCoin(coin);
        setCopiedAddress(false);
        const nextNetworks = coin === "usdt" ? USDT_NETWORKS : EVM_NETWORKS;
        if (!nextNetworks.some((network) => network.value === selectedNetwork)) {
            setSelectedNetwork("ethereum");
        }
    };
    return (<div className="flex min-h-[70vh] items-center justify-center p-4">
      <div className="flex w-full max-w-3xl flex-col items-stretch rounded-xl border bg-card shadow-sm md:flex-row">
        <div className="flex min-w-0 flex-1 basis-0 flex-col items-center justify-between space-y-6 border-b p-6 md:border-r md:border-b-0">
            <div className="flex flex-col items-center space-y-4">
              <div className="relative flex h-32 w-full items-center justify-center">
                <img src={KofiLogo} className="pointer-events-none absolute w-72" alt="Ko-fi"/>
              </div>
              <h4 className="font-semibold text-foreground">Support via Ko-fi</h4>
              <p className="px-4 text-center text-sm text-muted-foreground">
                Enjoying the project? You can support ongoing development by buying me a coffee.
              </p>
            </div>
            <Button className="h-9 w-4/5 gap-2 bg-[#72a4f2] text-sm font-semibold text-white hover:bg-[#5f8cd6]" onClick={() => openExternal("https://ko-fi.com/afkarxyz")}>
              <img src={KofiSvg} className="h-6 w-6 shrink-0" alt="" aria-hidden="true"/>
              Support me on Ko-fi
            </Button>
          </div>

          <div className="flex min-w-0 flex-1 basis-0 flex-col items-center justify-between space-y-4 p-6">
            <div className="flex w-full flex-col items-center space-y-3">
              <div className="flex h-32 items-center justify-center">
                <div className="rounded-xl border bg-white p-2 shadow-sm">
                  <img src={cryptoQr} className="h-24 w-24 object-contain" alt={`${selectedCoin.toUpperCase()} ${activeNetwork.label} QR code`}/>
                </div>
              </div>
              <h4 className="font-semibold text-foreground">Support via Crypto</h4>
              <p className="px-4 text-center text-sm text-muted-foreground">
                Crypto donations are also accepted. Scan the QR code or copy the address.
              </p>
            </div>
            <div className="w-full space-y-2">
              <div className="grid grid-cols-2 gap-2">
                <Select value={selectedCoin} onValueChange={(value: CryptoCoin) => handleCoinChange(value)}>
                  <SelectTrigger size="sm" className="w-full" aria-label="Coin">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="usdt">USDT</SelectItem>
                    <SelectItem value="usdc">USDC</SelectItem>
                  </SelectContent>
                </Select>
                <Select value={selectedNetwork} onValueChange={(value: CryptoNetwork) => {
            setSelectedNetwork(value);
            setCopiedAddress(false);
        }}>
                  <SelectTrigger size="sm" className="w-full" aria-label="Network">
                    <SelectValue>{activeNetwork.shortLabel}</SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    {networkOptions.map((network) => (<SelectItem key={network.value} value={network.value}>{network.label}</SelectItem>))}
                  </SelectContent>
                </Select>
              </div>
              <div className="flex h-10 w-full items-center justify-between gap-2 rounded-lg border bg-muted/50 py-1.5 pr-1.5 pl-3">
                <code className="min-w-0 flex-1 truncate text-xs font-mono text-muted-foreground" title={cryptoAddress}>
                  {cryptoAddress}
                </code>
                <Button variant="ghost" size="icon" className="h-7 w-7 shrink-0 hover:bg-background" aria-label="Copy wallet address" onClick={() => {
            navigator.clipboard.writeText(cryptoAddress);
            setCopiedAddress(true);
            setTimeout(() => setCopiedAddress(false), 500);
        }}>
                  {copiedAddress ? <CircleCheck className="h-3.5 w-3.5 text-green-500"/> : <Copy className="h-3.5 w-3.5"/>}
                </Button>
              </div>
            </div>
          </div>
      </div>
    </div>);
}
