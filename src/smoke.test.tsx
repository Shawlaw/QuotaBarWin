import { render, screen } from "@testing-library/react";
import { ProviderCard } from "./components/ProviderCard";
import type { ProviderSnapshot } from "./types";

test("smoke renders V0 provider card from a standard snapshot shape", () => {
  const provider: ProviderSnapshot = {
    id: "codex-mock",
    name: "Codex Mock",
    status: "ok",
    source: "mock",
    updatedAt: "2026-06-08T00:00:00.000Z",
    error: null,
    diagnostics: null,
    metadata: null,
    windows: [
      {
        id: "5h",
        label: "5h window",
        used: 28,
        limit: 100,
        unit: "percent",
        usedPercent: 28,
        remainingPercent: 72,
        resetAt: null,
        resetText: null,
        confidence: "estimated"
      },
      {
        id: "weekly",
        label: "weekly window",
        used: 65,
        limit: 100,
        unit: "percent",
        usedPercent: 65,
        remainingPercent: 35,
        resetAt: null,
        resetText: null,
        confidence: "estimated"
      }
    ]
  };

  render(<ProviderCard provider={provider} />);

  expect(screen.getByText("5h window")).toBeInTheDocument();
  expect(screen.getByText("weekly window")).toBeInTheDocument();
});
