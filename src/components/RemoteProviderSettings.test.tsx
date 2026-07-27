import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { ComponentProps, ReactElement } from "react";
import { RemoteProviderSettings } from "./RemoteProviderSettings";
import { DEFAULT_REMOTE_PROVIDER_REGISTRY_URL } from "../lib/defaults";
import { I18nProvider } from "../i18n";
import type {
  RemoteProviderCatalogEntry,
  RemoteProviderConfig,
  RegistryMigrationResult,
  RemoteProviderRegistrySettings
} from "../types";

const registrySettings: RemoteProviderRegistrySettings = {
  registryUrl: null,
  providerProxyUrl: null,
  autoUpdate: true
};

const catalog: RemoteProviderCatalogEntry[] = [
  {
    id: "kimi-coding",
    displayName: "Kimi Coding",
    version: "1.0.0",
    description: "Kimi usage",
    providerUrl: "https://example.com/kimi/provider.json",
    checksum: "sha256:manifest",
    installed: false,
    error: null
  }
];

const installedProvider: RemoteProviderConfig = {
  id: "kimi-coding",
  name: "Kimi Coding",
  enabled: true,
  kind: "remote",
  version: "1.0.0",
  manifestUrl: "https://example.com/kimi/provider.json",
  sourceUrl: "https://example.com/kimi/provider.cjs",
  runtime: "node",
  autoUpdate: true,
  updateIntervalSeconds: 3600,
  timeoutSeconds: 30
};

const migratedSource: RegistryMigrationResult = {
  migrated: ["kimi-coding"],
  skipped: [],
  failed: []
};

function renderWithEnglish(ui: ReactElement) {
  return render(ui, {
    wrapper: ({ children }) => (
      <I18nProvider language="en">{children}</I18nProvider>
    )
  });
}

function renderRemoteProviderSettings(
  props: Partial<ComponentProps<typeof RemoteProviderSettings>> = {}
) {
  const mergedProps: ComponentProps<typeof RemoteProviderSettings> = {
    view: "add",
    registrySettings,
    installedProviderIds: [],
    onRegistrySettingsChange: vi.fn(),
    onPreviewRegistry: vi.fn(async () => catalog),
    onInstallManifest: vi.fn(async () => installedProvider),
    onMigrateSource: vi.fn(async () => migratedSource),
    onOpenGuide: vi.fn(async () => undefined),
    onBackToSettings: vi.fn(),
    onBackToAddProvider: vi.fn(),
    onManageSources: vi.fn(),
    ...props
  };

  return {
    ...renderWithEnglish(<RemoteProviderSettings {...mergedProps} />),
    props: mergedProps
  };
}

describe("RemoteProviderSettings", () => {
  test("add_page_loads_catalog_and_opens_guide", async () => {
    const onOpenGuide = vi.fn(async () => undefined);
    const { props } = renderRemoteProviderSettings({ onOpenGuide });

    expect(screen.getByTestId("add-provider-page")).toBeInTheDocument();
    expect(
      screen.getByText("Security notice: only install and use Providers you trust.")
    ).toBeInTheDocument();
    expect(
      screen.getByText(/Provider scripts can read the AI credentials/)
    ).toBeInTheDocument();
    await screen.findByText("Kimi Coding");
    expect(props.onPreviewRegistry).toHaveBeenCalledWith(
      DEFAULT_REMOTE_PROVIDER_REGISTRY_URL,
      null
    );

    fireEvent.click(screen.getByTestId("open-remote-provider-guide"));
    expect(onOpenGuide).toHaveBeenCalledTimes(1);
  });

  test("catalog_install_button_installs_manifest", async () => {
    const onInstallManifest = vi.fn(async () => installedProvider);
    renderRemoteProviderSettings({ onInstallManifest });

    await screen.findByText("Kimi Coding");
    fireEvent.click(screen.getByRole("button", { name: "Install" }));

    await waitFor(() =>
      expect(onInstallManifest).toHaveBeenCalledWith(
        "https://example.com/kimi/provider.json",
        "sha256:manifest",
        null,
        true
      )
    );
    expect(await screen.findByText("Provider installed")).toBeInTheDocument();
  });

  test("catalog_installed_provider_can_add_another_account", async () => {
    const onInstallManifest = vi.fn(async () => ({
      ...installedProvider,
      id: "kimi-coding-2",
      name: "Kimi Coding 2"
    }));
    renderRemoteProviderSettings({
      onInstallManifest,
      onPreviewRegistry: vi.fn(async () => [
        {
          ...catalog[0],
          installed: true,
          installedCount: 1
        }
      ])
    });

    await screen.findByText("Kimi Coding");
    expect(screen.getByText("1 account")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add account" }));

    await waitFor(() =>
      expect(onInstallManifest).toHaveBeenCalledWith(
        "https://example.com/kimi/provider.json",
        "sha256:manifest",
        null,
        true
      )
    );
    expect(await screen.findByText("Account added")).toBeInTheDocument();
    expect(screen.getByText("2 accounts")).toBeInTheDocument();
  });

  test("custom_manifest_install_calls_handler", async () => {
    const onInstallManifest = vi.fn(async () => installedProvider);
    renderRemoteProviderSettings({ onInstallManifest });

    fireEvent.change(screen.getByTestId("remote-provider-manifest-url-input"), {
      target: { value: "D:\\Providers\\provider.json" }
    });
    fireEvent.change(screen.getByTestId("remote-provider-manifest-checksum-input"), {
      target: { value: "sha256:abc" }
    });
    fireEvent.click(screen.getByTestId("install-remote-provider-manifest"));

    await waitFor(() =>
      expect(onInstallManifest).toHaveBeenCalledWith(
        "D:\\Providers\\provider.json",
        "sha256:abc",
        null,
        true
      )
    );
  });

  test("source_page_edits_registry_settings", () => {
    const onRegistrySettingsChange = vi.fn();
    renderRemoteProviderSettings({
      view: "sources",
      registrySettings: {
        registryUrl: null,
        providerProxyUrl: null,
        autoUpdate: true
      },
      onRegistrySettingsChange
    });

    expect(screen.getByTestId("provider-source-page")).toBeInTheDocument();
    expect(screen.getByTestId("remote-provider-url-input")).toHaveValue(
      DEFAULT_REMOTE_PROVIDER_REGISTRY_URL
    );

    fireEvent.change(screen.getByTestId("remote-provider-url-input"), {
      target: { value: "https://example.com/registry.json" }
    });
    expect(onRegistrySettingsChange).toHaveBeenLastCalledWith({
      registryUrl: "https://example.com/registry.json",
      providerProxyUrl: null,
      autoUpdate: true,
      sources: [
        {
          id: "official",
          name: "Project-maintained source",
          url: "https://example.com/registry.json",
          providerProxyUrl: null,
          autoUpdate: true,
          enabled: true
        }
      ]
    });
  });

  test("source_page_adds_multiple_registry_sources", () => {
    const onRegistrySettingsChange = vi.fn();
    renderRemoteProviderSettings({
      view: "sources",
      onRegistrySettingsChange
    });

    fireEvent.click(screen.getByTestId("add-provider-source"));

    const nextSettings = onRegistrySettingsChange.mock.calls.at(-1)?.[0];
    expect(nextSettings.sources).toHaveLength(2);
    expect(nextSettings.sources[0].url).toBe(DEFAULT_REMOTE_PROVIDER_REGISTRY_URL);
    expect(nextSettings.sources[1]).toMatchObject({
      name: "Custom source 2",
      url: "",
      enabled: true
    });
  });

  test("source_page_migrates_installed_providers_only_after_confirmation", async () => {
    const onMigrateSource = vi.fn(async () => migratedSource);
    vi.spyOn(window, "confirm").mockReturnValue(true);
    renderRemoteProviderSettings({
      view: "sources",
      onMigrateSource,
    });

    fireEvent.click(screen.getByTestId("migrate-provider-source-official"));

    await waitFor(() =>
      expect(onMigrateSource).toHaveBeenCalledWith(
        DEFAULT_REMOTE_PROVIDER_REGISTRY_URL,
        null
      )
    );
    expect(await screen.findByText("Migration complete: 1 migrated, 0 skipped, 0 failed.")).toBeInTheDocument();
  });

  test("add_page_loads_all_enabled_sources", async () => {
    const onPreviewRegistry = vi.fn(async (url: string) => [
      {
        ...catalog[0],
        id: url.includes("team") ? "team-provider" : "project-provider",
        displayName: url.includes("team") ? "Team Provider" : "Project Provider",
        providerUrl: `${url}/provider.json`
      }
    ]);

    renderRemoteProviderSettings({
      registrySettings: {
        registryUrl: null,
        providerProxyUrl: null,
        autoUpdate: true,
        sources: [
          {
            id: "official",
            name: "Project-maintained",
            url: "https://example.com/project/registry.json",
            providerProxyUrl: null,
            autoUpdate: true,
            enabled: true
          },
          {
            id: "team",
            name: "Team",
            url: "https://example.com/team/registry.json",
            providerProxyUrl: "http://proxy:8080",
            autoUpdate: false,
            enabled: true
          }
        ]
      },
      onPreviewRegistry
    });

    await screen.findByText("Project Provider");
    await screen.findByText("Team Provider");
    expect(onPreviewRegistry).toHaveBeenCalledWith(
      "https://example.com/project/registry.json",
      null
    );
    expect(onPreviewRegistry).toHaveBeenCalledWith(
      "https://example.com/team/registry.json",
      "http://proxy:8080"
    );
  });

  test("late catalog failure does not overwrite a newer successful load", async () => {
    let rejectInitialLoad: (reason?: unknown) => void = () => undefined;
    let resolveLatestLoad: (entries: RemoteProviderCatalogEntry[]) => void = () => undefined;
    const onPreviewRegistry = vi.fn(
      (_url: string, proxyUrl: string | null) =>
        new Promise<RemoteProviderCatalogEntry[]>((resolve, reject) => {
          if (proxyUrl) {
            resolveLatestLoad = resolve;
          } else {
            rejectInitialLoad = reject;
          }
        })
    );
    const initialSettings: RemoteProviderRegistrySettings = {
      registryUrl: "https://example.com/registry.json",
      providerProxyUrl: null,
      autoUpdate: true
    };
    const { rerender } = renderRemoteProviderSettings({
      registrySettings: initialSettings,
      onPreviewRegistry
    });

    await waitFor(() => expect(onPreviewRegistry).toHaveBeenCalledTimes(1));
    rerender(
      <I18nProvider language="en">
        <RemoteProviderSettings
          view="add"
          registrySettings={{
            ...initialSettings,
            providerProxyUrl: "socks5h://127.0.0.1:10818"
          }}
          installedProviderIds={[]}
          onRegistrySettingsChange={vi.fn()}
          onPreviewRegistry={onPreviewRegistry}
          onInstallManifest={vi.fn(async () => installedProvider)}
          onMigrateSource={vi.fn(async () => migratedSource)}
          onOpenGuide={vi.fn(async () => undefined)}
          onBackToSettings={vi.fn()}
          onBackToAddProvider={vi.fn()}
          onManageSources={vi.fn()}
        />
      </I18nProvider>
    );
    await waitFor(() => expect(onPreviewRegistry).toHaveBeenCalledTimes(2));

    resolveLatestLoad([{ ...catalog[0], displayName: "Fresh Catalog" }]);
    expect(await screen.findByText("Fresh Catalog")).toBeInTheDocument();

    rejectInitialLoad(new Error("Network error: stale request"));
    await waitFor(() => expect(screen.getByText("Fresh Catalog")).toBeInTheDocument());
    expect(screen.queryByText(/stale request/)).not.toBeInTheDocument();
  });
});
