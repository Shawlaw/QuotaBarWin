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

test("provider_card_renders_error_state", () => {
  render(<ProviderCard provider={{ ...provider, status: "error", error: "Command failed" }} />);

  expect(screen.getByText("Command failed")).toBeInTheDocument();
});

test("provider_card_renders_used_and_remaining", () => {
  render(<ProviderCard provider={provider} />);

  expect(screen.getByText("28% used / 72% remaining")).toBeInTheDocument();
});
