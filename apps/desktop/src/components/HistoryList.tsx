import type { HistoryRecord } from "@tator/shared-types";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

interface HistoryListProps {
  history: HistoryRecord[];
}

export function HistoryList({ history }: HistoryListProps) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-baseline justify-between gap-4 pb-3">
        <CardTitle className="text-base">Recent dictation</CardTitle>
        <span className="text-sm text-muted-foreground">{history.length} items</span>
      </CardHeader>
      <CardContent>
        {history.length === 0 ? (
          <p className="text-muted-foreground">No dictation history yet.</p>
        ) : (
          <ul className="grid gap-3 list-none m-0 p-0">
            {history.map((item) => (
              <li key={item.id} className="rounded-2xl bg-white/[0.03] p-3">
                <div className="flex flex-wrap gap-2 text-muted-foreground text-xs uppercase">
                  <span>{item.mode}</span>
                  <span>{item.status}</span>
                  <span>{new Date(item.createdAt).toLocaleString()}</span>
                </div>
                <p className="mt-2 mb-0">{item.cleanText}</p>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
