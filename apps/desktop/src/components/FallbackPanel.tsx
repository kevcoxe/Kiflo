interface FallbackPanelProps {
  text: string;
  onCopy: () => void;
  onRetry: () => void;
}

export function FallbackPanel({
  text,
  onCopy,
  onRetry,
}: FallbackPanelProps) {
  return (
    <section aria-label="Fallback paste panel" className="panel fallback-panel">
      <div className="panel-header">
        <h2>Paste fallback</h2>
        <span>Accessibility or paste failed</span>
      </div>
      <pre>{text}</pre>
      <div className="button-row">
        <button className="secondary-button" onClick={onCopy} type="button">
          Copy
        </button>
        <button className="primary-button" onClick={onRetry} type="button">
          Retry Paste
        </button>
      </div>
    </section>
  );
}
