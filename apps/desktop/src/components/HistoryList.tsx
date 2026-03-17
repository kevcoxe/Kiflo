import { useState } from "react";
import type { FocusedEditorContext, HistoryRecord } from "@tator/shared-types";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

interface HistoryListProps {
  history: HistoryRecord[];
  editorDebugMap?: Record<string, FocusedEditorContext>;
}

function flag(value: boolean) {
  return value ? "✓" : "✗";
}

function EditorDebugInfo({ ctx }: { ctx: FocusedEditorContext }) {
  const [expanded, setExpanded] = useState(false);

  const confidence = ctx.confidence.toFixed(2);
  const composerColor = ctx.isComposer ? "text-green-400" : "text-yellow-400";

  return (
    <div className="mt-2 border border-white/10 rounded-lg p-2 text-xs font-mono space-y-1">
      <div className="flex flex-wrap gap-x-3 gap-y-0.5 text-muted-foreground">
        <span>
          <span className="opacity-50">app</span>{" "}
          {ctx.appName ?? "—"}
        </span>
        <span className={ctx.isSupportedEditor ? "text-green-400" : "text-red-400"}>
          {flag(ctx.isSupportedEditor)} supported editor
        </span>
        <span className={ctx.isTextInputLike ? "text-green-400" : "text-red-400"}>
          {flag(ctx.isTextInputLike)} text input
        </span>
        <span className={composerColor}>
          {flag(ctx.isComposer)} composer
        </span>
        <span>
          <span className="opacity-50">conf</span>{" "}
          {confidence}
        </span>
      </div>

      <div className="flex flex-wrap gap-x-3 gap-y-0.5">
        {ctx.matchedPositiveCues.length > 0 ? (
          <span className="text-green-400">+{ctx.matchedPositiveCues.join(", ")}</span>
        ) : (
          <span className="text-muted-foreground opacity-50">no positive cues</span>
        )}
        {ctx.matchedNegativeCues.length > 0 ? (
          <span className="text-red-400">−{ctx.matchedNegativeCues.join(", ")}</span>
        ) : null}
      </div>

      <button
        className="text-muted-foreground opacity-50 hover:opacity-100 text-xs"
        onClick={() => setExpanded((v) => !v)}
        type="button"
      >
        {expanded ? "▾ hide raw AX values" : "▸ show raw AX values"}
      </button>

      {expanded ? (
        <div className="space-y-1 pt-1 border-t border-white/10">
          <div>
            <span className="opacity-50">roles: </span>
            {ctx.rawRoleValues.length > 0
              ? ctx.rawRoleValues.join(", ")
              : <span className="opacity-40">none</span>}
          </div>
          <div>
            <span className="opacity-50">text: </span>
            {ctx.rawTextValues.length > 0
              ? ctx.rawTextValues.join(", ")
              : <span className="opacity-40">none</span>}
          </div>
        </div>
      ) : null}
    </div>
  );
}

export function HistoryList({ history, editorDebugMap }: HistoryListProps) {
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
            {history.map((item) => {
              const ctx = editorDebugMap?.[item.id];
              return (
                <li key={item.id} className="rounded-2xl bg-white/[0.03] p-3">
                  <div className="flex flex-wrap gap-2 text-muted-foreground text-xs uppercase">
                    <span>{item.mode}</span>
                    <span>{item.status}</span>
                    <span>{new Date(item.createdAt).toLocaleString()}</span>
                  </div>
                  <p className="mt-2 mb-0">{item.cleanText}</p>
                  {ctx ? <EditorDebugInfo ctx={ctx} /> : null}
                </li>
              );
            })}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
