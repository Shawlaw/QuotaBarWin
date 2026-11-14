import type {
  AppConfig,
  AppSnapshot,
  ProviderSnapshot,
  QuotaWindow,
  RemoteProviderConfig,
} from "../types";

function configuredWindowIds(visibleWindowIds: string[] | undefined): string[] {
  return Array.from(
    new Set((visibleWindowIds ?? []).map((id) => id.trim()).filter(Boolean)),
  );
}

/**
 * Applies the user's visible-window list to quota windows.
 *
 * Mirrors `remote_provider_runner::apply_visible_windows`: entries match by id
 * first, then by the provider-supplied label, and windows missing from a
 * non-empty list stay hidden. Keeping the projection identical is what lets a
 * reorder show up immediately instead of waiting for the next provider run.
 */
export function applyVisibleWindowOrder(
  windows: QuotaWindow[],
  visibleWindowIds: string[] | undefined,
): QuotaWindow[] {
  const configured = configuredWindowIds(visibleWindowIds);
  if (configured.length === 0) {
    return windows;
  }

  const selected = new Set<number>();
  const ordered: QuotaWindow[] = [];
  for (const wanted of configured) {
    for (const [index, window] of windows.entries()) {
      if (!selected.has(index) && window.id === wanted) {
        selected.add(index);
        ordered.push(window);
      }
    }
    for (const [index, window] of windows.entries()) {
      if (!selected.has(index) && window.label === wanted) {
        selected.add(index);
        ordered.push(window);
      }
    }
  }
  return ordered;
}

export function projectProviderSnapshot(
  provider: ProviderSnapshot,
  config: RemoteProviderConfig,
): ProviderSnapshot {
  const labelOverrides = config.windowLabelOverrides ?? {};
  return {
    ...provider,
    name: config.name,
    windows: applyVisibleWindowOrder(provider.windows, config.visibleWindowIds).map(
      (window) => ({
        ...window,
        label: labelOverrides[window.id] ?? window.label,
      }),
    ),
  };
}

export function projectSnapshotForConfig(
  snapshot: AppSnapshot | null,
  config: AppConfig,
): AppSnapshot | null {
  if (!snapshot) {
    return snapshot;
  }

  const providersById = new Map(snapshot.providers.map((provider) => [provider.id, provider]));
  return {
    ...snapshot,
    providers: config.providers.flatMap((providerConfig) => {
      if (!providerConfig.enabled) {
        return [];
      }

      const provider = providersById.get(providerConfig.id);
      return provider ? [projectProviderSnapshot(provider, providerConfig)] : [];
    }),
  };
}
