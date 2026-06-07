import { useCallback, useEffect, useState } from "react";
import { Header } from "./components/Header";
import { ProviderCard } from "./components/ProviderCard";
import { refreshSnapshot } from "./lib/api";
import type { AppSnapshot } from "./types";

function fallbackSnapshot(error: unknown): AppSnapshot {
  return {
    schemaVersion: 1,
    refreshedAt: new Date().toISOString(),
    providers: [
      {
        id: "mock-error",
        name: "Codex Mock",
        status: "error",
        source: "mock",
        updatedAt: new Date().toISOString(),
        error: error instanceof Error ? error.message : "Unable to refresh snapshot",
        diagnostics: null,
        metadata: null,
        windows: []
      }
    ]
  };
}

export function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const loadSnapshot = useCallback(async () => {
    setIsLoading(true);
    try {
      setSnapshot(await refreshSnapshot());
    } catch (error) {
      setSnapshot(fallbackSnapshot(error));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadSnapshot();
  }, [loadSnapshot]);

  return (
    <main className="app-shell">
      <Header isLoading={isLoading} onRefresh={loadSnapshot} />
      <section className="provider-list" aria-label="Providers">
        {snapshot?.providers.map((provider) => (
          <ProviderCard key={provider.id} provider={provider} />
        ))}
      </section>
    </main>
  );
}
