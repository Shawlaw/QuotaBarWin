import { render, screen } from "@testing-library/react";
import { ProviderCard } from "./ProviderCard";
import type { ProviderSnapshot } from "../types";

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
    }
  ]
};

test("ProviderCard renders provider name", () => {
  render(<ProviderCard provider={provider} />);

  expect(screen.getByRole("heading", { name: "Codex Mock" })).toBeInTheDocument();
});
