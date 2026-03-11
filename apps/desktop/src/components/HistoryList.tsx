import type { HistoryRecord } from "@tator/shared-types";

interface HistoryListProps {
  history: HistoryRecord[];
}

export function HistoryList({ history }: HistoryListProps) {
  return (
    <section className="panel">
      <div className="panel-header">
        <h2>Recent dictation</h2>
        <span>{history.length} items</span>
      </div>
      {history.length === 0 ? (
        <p className="muted">No dictation history yet.</p>
      ) : (
        <ul className="history-list">
          {history.map((item) => (
            <li key={item.id} className="history-card">
              <div className="history-meta">
                <span>{item.mode}</span>
                <span>{item.status}</span>
                <span>{new Date(item.createdAt).toLocaleString()}</span>
              </div>
              <p>{item.cleanText}</p>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
