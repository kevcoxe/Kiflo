import { useState } from "react";
import type { HistoryMetrics } from "@tator/shared-types";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";

interface MetricsDashboardProps {
  metrics: HistoryMetrics;
}

function formatDuration(milliseconds: number): string {
  const totalSeconds = Math.floor(milliseconds / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m ${seconds}s`;
  }

  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }

  return `${seconds}s`;
}

export function MetricsDashboard({ metrics }: MetricsDashboardProps) {
  const [hideCommonWords, setHideCommonWords] = useState(true);
  const visibleWords = hideCommonWords ? metrics.topMeaningfulWords : metrics.topWords;
  const emptyMessage =
    hideCommonWords && metrics.topWords.length > 0
      ? "Only common words matched the current filter."
      : "No spoken words yet.";

  return (
    <Card>
      <CardHeader className="flex flex-row items-baseline justify-between gap-4 pb-3">
        <h2 className="text-base font-semibold leading-none tracking-tight">Dictation metrics</h2>
        <span className="text-sm text-muted-foreground">{metrics.totalSessions} sessions</span>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-[repeat(auto-fit,minmax(170px,1fr))] gap-3">
          <article className="border border-border rounded-2xl bg-white/[0.03] p-3">
            <h3 className="m-0 text-muted-foreground text-xs font-semibold uppercase">Total words spoken</h3>
            <p className="mt-1.5 mb-0 text-xl font-bold">{metrics.totalWordsSpoken.toLocaleString()}</p>
          </article>
          <article className="border border-border rounded-2xl bg-white/[0.03] p-3">
            <h3 className="m-0 text-muted-foreground text-xs font-semibold uppercase">Total speaking time</h3>
            <p className="mt-1.5 mb-0 text-xl font-bold">{formatDuration(metrics.totalSpeakingTimeMs)}</p>
          </article>
          <article className="border border-border rounded-2xl bg-white/[0.03] p-3">
            <h3 className="m-0 text-muted-foreground text-xs font-semibold uppercase">Estimated typing time saved</h3>
            <p className="mt-1.5 mb-0 text-xl font-bold">{formatDuration(metrics.estimatedTypingTimeSavedMs)}</p>
          </article>
          <article className="border border-border rounded-2xl bg-white/[0.03] p-3">
            <h3 className="m-0 text-muted-foreground text-xs font-semibold uppercase">Avg words / session</h3>
            <p className="mt-1.5 mb-0 text-xl font-bold">{metrics.averageWordsPerSession.toFixed(1)}</p>
          </article>
        </div>

        <div>
          <div className="flex items-center justify-between gap-3 flex-wrap">
            <strong>Top words</strong>
            <div className="flex items-center gap-2 text-muted-foreground text-sm">
              <Switch
                id="hide-common-words"
                aria-label="Hide common words"
                checked={hideCommonWords}
                onCheckedChange={setHideCommonWords}
              />
              <Label htmlFor="hide-common-words">Hide common words</Label>
            </div>
          </div>
          {visibleWords.length === 0 ? (
            <p className="text-muted-foreground mt-2">{emptyMessage}</p>
          ) : (
            <ul className="flex flex-wrap gap-2 list-none m-0 mt-2 p-0" aria-label="Top spoken words">
              {visibleWords.map((item) => (
                <li key={item.word}>
                  <Badge variant="secondary" className="gap-1.5">
                    <span>{item.word}</span>
                    <em className="text-muted-foreground not-italic">{item.count}</em>
                  </Badge>
                </li>
              ))}
            </ul>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
