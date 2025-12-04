import { Button } from "@/components/ui/button";
import { X, User } from "lucide-react";

export interface HistoryItem {
  id: string;
  username: string;
  name: string;
  image: string;
  mediaCount: number;
  timestamp: number;
}

interface FetchHistoryProps {
  history: HistoryItem[];
  onSelect: (item: HistoryItem) => void;
  onRemove: (id: string) => void;
}

export function FetchHistory({ history, onSelect, onRemove }: FetchHistoryProps) {
  if (history.length === 0) return null;

  return (
    <div className="space-y-2">
      <p className="text-sm text-muted-foreground">Recent Fetches</p>
      <div className="flex flex-wrap gap-2">
        {history.map((item) => (
          <div
            key={item.id}
            className="flex items-center gap-2 px-3 py-1.5 bg-muted/50 rounded-full hover:bg-muted transition-colors group cursor-pointer"
            onClick={() => onSelect(item)}
          >
            {item.image ? (
              <img
                src={item.image}
                alt={item.name}
                className="w-5 h-5 rounded-full"
              />
            ) : (
              <User className="w-5 h-5 text-muted-foreground" />
            )}
            <span className="text-sm">@{item.username}</span>
            <Button
              variant="ghost"
              size="icon"
              className="h-4 w-4 opacity-0 group-hover:opacity-100 transition-opacity"
              onClick={(e) => {
                e.stopPropagation();
                onRemove(item.id);
              }}
            >
              <X className="h-3 w-3" />
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
