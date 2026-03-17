import type { ChangeEvent, MutableRefObject } from "react";
import { useEffect, useRef, useState } from "react";
import type { Settings } from "@kiflo/shared-types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

interface DictionaryPanelProps {
  settings: Settings;
  onChange: (settings: Settings) => void;
}

interface DictionaryRow {
  id: number;
  source: string;
  replacement: string;
}

export function DictionaryPanel({ settings, onChange }: DictionaryPanelProps) {
  const nextDictionaryRowId = useRef(0);
  const syncedDictionaryRef = useRef(settings.customDictionary);
  const [dictionaryRows, setDictionaryRows] = useState<DictionaryRow[]>(() =>
    parseCustomDictionary(settings.customDictionary, () =>
      createDictionaryRow(nextDictionaryRowId),
    ),
  );

  useEffect(() => {
    if (settings.customDictionary === syncedDictionaryRef.current) {
      return;
    }

    syncedDictionaryRef.current = settings.customDictionary;
    setDictionaryRows(
      parseCustomDictionary(settings.customDictionary, () =>
        createDictionaryRow(nextDictionaryRowId),
      ),
    );
  }, [settings.customDictionary]);

  function updateDictionary(nextRows: DictionaryRow[]) {
    const customDictionary = serializeCustomDictionary(nextRows);
    syncedDictionaryRef.current = customDictionary;
    setDictionaryRows(nextRows);
    onChange({ ...settings, customDictionary });
  }

  function addDictionaryRow() {
    setDictionaryRows((current) => [
      ...current,
      createDictionaryRow(nextDictionaryRowId),
    ]);
  }

  function updateDictionaryRow(
    rowId: number,
    key: "source" | "replacement",
    value: string,
  ) {
    const nextRows = dictionaryRows.map((row) =>
      row.id === rowId ? { ...row, [key]: value } : row,
    );
    updateDictionary(nextRows);
  }

  function removeDictionaryRow(rowId: number) {
    updateDictionary(dictionaryRows.filter((row) => row.id !== rowId));
  }

  return (
    <div className="space-y-6 p-4">
      <div className="space-y-2">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground px-1">
          Custom Dictionary
        </h2>
        <div className="rounded-xl border bg-card overflow-hidden">
          <div className="px-4 py-3 flex flex-col gap-3">
            <p className="text-sm text-muted-foreground m-0">
              Each row replaces whole words or phrases only, case-insensitively.
            </p>
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  {dictionaryRows.length > 0 ? (
                    <TableRow>
                      <TableHead>Heard</TableHead>
                      <TableHead>Replace with</TableHead>
                      <TableHead aria-label="Actions" />
                    </TableRow>
                  ) : null}
                </TableHeader>
                <TableBody>
                  {dictionaryRows.length > 0 ? (
                    dictionaryRows.map((row, index) => (
                      <TableRow key={row.id}>
                        <TableCell>
                          <Input
                            aria-label={`Dictionary source ${index + 1}`}
                            onChange={(event: ChangeEvent<HTMLInputElement>) =>
                              updateDictionaryRow(row.id, "source", event.target.value)
                            }
                            placeholder="Rebecca"
                            spellCheck={false}
                            type="text"
                            value={row.source}
                          />
                        </TableCell>
                        <TableCell>
                          <Input
                            aria-label={`Dictionary replacement ${index + 1}`}
                            onChange={(event: ChangeEvent<HTMLInputElement>) =>
                              updateDictionaryRow(row.id, "replacement", event.target.value)
                            }
                            placeholder="Rebekah"
                            spellCheck={false}
                            type="text"
                            value={row.replacement}
                          />
                        </TableCell>
                        <TableCell className="w-px whitespace-nowrap">
                          <Button
                            variant="outline"
                            className="border-red-800/40 bg-red-950/20 text-red-300 hover:bg-red-950/40 hover:text-red-200"
                            onClick={() => removeDictionaryRow(row.id)}
                            type="button"
                          >
                            Remove
                          </Button>
                        </TableCell>
                      </TableRow>
                    ))
                  ) : (
                    <TableRow>
                      <TableCell colSpan={3} className="text-muted-foreground py-3.5">
                        No custom replacements yet.
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            </div>
            <div className="flex justify-start">
              <Button variant="outline" onClick={addDictionaryRow} type="button">
                Add word
              </Button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function createDictionaryRow(nextDictionaryRowId: MutableRefObject<number>): DictionaryRow {
  const row = {
    id: nextDictionaryRowId.current,
    source: "",
    replacement: "",
  };
  nextDictionaryRowId.current += 1;
  return row;
}

function parseCustomDictionary(
  customDictionary: string,
  createRow: () => DictionaryRow,
): DictionaryRow[] {
  return customDictionary
    .split("\n")
    .map((line) => parseDictionaryLine(line, createRow))
    .filter((row): row is DictionaryRow => row !== null);
}

function parseDictionaryLine(
  line: string,
  createRow: () => DictionaryRow,
): DictionaryRow | null {
  const trimmed = line.trim();
  if (!trimmed) {
    return null;
  }

  const entry = trimmed.match(/^(.*?)(?:=>|->|=)(.*)$/);
  if (!entry) {
    return null;
  }

  const source = entry[1]?.trim() ?? "";
  const replacement = entry[2]?.trim() ?? "";
  if (!source || !replacement) {
    return null;
  }

  return {
    ...createRow(),
    source,
    replacement,
  };
}

function serializeCustomDictionary(rows: DictionaryRow[]): string {
  return rows
    .map((row) => ({
      source: row.source.trim(),
      replacement: row.replacement.trim(),
    }))
    .filter((row) => row.source && row.replacement)
    .map((row) => `${row.source} => ${row.replacement}`)
    .join("\n");
}
