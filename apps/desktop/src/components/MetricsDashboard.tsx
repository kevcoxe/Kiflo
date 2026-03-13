import { useState } from "react";
import type { HistoryMetrics } from "@tator/shared-types";

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
    <section className="panel">
      <div className="panel-header">
        <h2>Dictation metrics</h2>
        <span>{metrics.totalSessions} sessions</span>
      </div>

      <div className="metrics-grid">
        <article className="metric-card">
          <h3>Total words spoken</h3>
          <p>{metrics.totalWordsSpoken.toLocaleString()}</p>
        </article>
        <article className="metric-card">
          <h3>Total speaking time</h3>
          <p>{formatDuration(metrics.totalSpeakingTimeMs)}</p>
        </article>
        <article className="metric-card">
          <h3>Estimated typing time saved</h3>
          <p>{formatDuration(metrics.estimatedTypingTimeSavedMs)}</p>
        </article>
        <article className="metric-card">
          <h3>Avg words / session</h3>
          <p>{metrics.averageWordsPerSession.toFixed(1)}</p>
        </article>
      </div>

      <div className="result-block">
        <div className="metrics-top-words-header">
          <strong>Top words</strong>
          <label className="toggle metrics-toggle">
            <input
              aria-label="Hide common words"
              checked={hideCommonWords}
              onChange={(event) => setHideCommonWords(event.target.checked)}
              type="checkbox"
            />
            <span>Hide common words</span>
          </label>
        </div>
        {visibleWords.length === 0 ? (
          <p className="muted">{emptyMessage}</p>
        ) : (
          <ul className="word-chip-list" aria-label="Top spoken words">
            {visibleWords.map((item) => (
              <li key={item.word} className="word-chip">
                <span>{item.word}</span>
                <em>{item.count}</em>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
