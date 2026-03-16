import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";

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
    <section aria-label="Fallback paste panel">
      <Alert variant="destructive" className="mb-3">
        <AlertTitle>Paste fallback</AlertTitle>
        <AlertDescription>Accessibility or paste failed</AlertDescription>
      </Alert>
      <pre className="overflow-x-auto rounded-2xl bg-black/25 mb-3 p-3.5 whitespace-pre-wrap text-sm">
        {text}
      </pre>
      <div className="flex flex-wrap gap-3">
        <Button variant="outline" onClick={onCopy} type="button">
          Copy
        </Button>
        <Button onClick={onRetry} type="button">
          Retry Paste
        </Button>
      </div>
    </section>
  );
}
