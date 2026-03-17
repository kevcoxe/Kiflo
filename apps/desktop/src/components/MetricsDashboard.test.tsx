import { fireEvent, render, screen } from "@testing-library/react";
import type { HistoryMetrics } from "@kiflo/shared-types";
import { MetricsDashboard } from "./MetricsDashboard";

const metrics: HistoryMetrics = {
  totalSessions: 4,
  totalWordsSpoken: 32,
  totalSpeakingTimeMs: 48_000,
  estimatedTypingTimeSavedMs: 36_000,
  averageWordsPerSession: 8,
  topWords: [
    { word: "the", count: 6 },
    { word: "i", count: 5 },
    { word: "update", count: 4 },
  ],
  topMeaningfulWords: [
    { word: "update", count: 4 },
    { word: "project", count: 2 },
  ],
};

describe("MetricsDashboard", () => {
  it("hides common words by default and lets users show them", () => {
    render(<MetricsDashboard metrics={metrics} />);

    expect(screen.getByLabelText("Hide common words")).toBeChecked();
    expect(screen.queryByText(/^the$/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/^i$/i)).not.toBeInTheDocument();
    expect(screen.getByText(/^update$/i)).toBeVisible();

    fireEvent.click(screen.getByLabelText("Hide common words"));

    expect(screen.getByText(/^the$/i)).toBeVisible();
    expect(screen.getByText(/^i$/i)).toBeVisible();
    expect(screen.getByText(/^update$/i)).toBeVisible();
  });
});
