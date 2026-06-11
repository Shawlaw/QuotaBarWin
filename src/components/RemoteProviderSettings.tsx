import { useState } from "react";
import { InstallRemoteProviderDialog } from "./InstallRemoteProviderDialog";
import type { RemoteProviderConfig } from "../types";
import type { RemoteProviderPreview, UpdateInfo } from "../lib/api";

type RemoteProviderSettingsProps = {
  providers: RemoteProviderConfig[];
  onPreview: (url: string, proxyUrl: string | null, autoUpdate: boolean) => Promise<RemoteProviderPreview>;
  onAdd: (url: string, proxyUrl: string | null, autoUpdate: boolean) => Promise<RemoteProviderConfig>;
  onRemove: (id: string) => Promise<void>;
  onRefresh: (id: string) => Promise<UpdateInfo>;
  onCheckUpdates: () => Promise<UpdateInfo[]>;
  onApplyUpdate: (id: string) => Promise<void>;
};

export function RemoteProviderSettings({
  providers,
  onPreview,
  onAdd,
  onRemove,
  onRefresh,
  onCheckUpdates,
  onApplyUpdate
}: RemoteProviderSettingsProps) {
  const [url, setUrl] = useState("");
  const [proxyUrl, setProxyUrl] = useState("");
  const [autoUpdate, setAutoUpdate] = useState(true);
  const [preview, setPreview] = useState<RemoteProviderPreview | null>(null);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [updates, setUpdates] = useState<UpdateInfo[]>([]);

  async function handlePreview() {
    setMessage(null);
    setLoading(true);
    try {
      const result = await onPreview(url.trim(), proxyUrl.trim() || null, autoUpdate);
      setPreview(result);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Failed to preview provider");
    } finally {
      setLoading(false);
    }
  }

  async function handleConfirmInstall() {
    setLoading(true);
    try {
      await onAdd(url.trim(), proxyUrl.trim() || null, autoUpdate);
      setPreview(null);
      setUrl("");
      setProxyUrl("");
      setMessage("Provider installed");
      setUpdates([]);
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Failed to install provider");
    } finally {
      setLoading(false);
    }
  }

  async function handleRemove(id: string) {
    setMessage(null);
    try {
      await onRemove(id);
      setUpdates((current) => current.filter((update) => update.id !== id));
      setMessage("Provider removed");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Failed to remove provider");
    }
  }

  async function handleRefresh(id: string) {
    setMessage(null);
    try {
      const result = await onRefresh(id);
      if (result.available) {
        setUpdates((current) => [
          ...current.filter((update) => update.id !== id),
          result
        ]);
        setMessage("Update available");
      } else {
        setMessage("Provider refreshed");
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Failed to refresh provider");
    }
  }

  async function handleCheckUpdates() {
    setMessage(null);
    try {
      const result = await onCheckUpdates();
      setUpdates(result);
      const availableCount = result.filter((update) => update.available).length;
      if (availableCount > 0) {
        setMessage(`${availableCount} update(s) available`);
      } else {
        setMessage("All providers are up to date");
      }
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Failed to check updates");
    }
  }

  async function handleApplyUpdate(id: string) {
    setMessage(null);
    try {
      await onApplyUpdate(id);
      setUpdates((current) => current.filter((update) => update.id !== id));
      setMessage("Update applied");
    } catch (error) {
      setMessage(error instanceof Error ? error.message : "Failed to apply update");
    }
  }

  return (
    <section className="settings-section" aria-label="Remote Providers" data-testid="remote-providers-section">
      <div className="settings-section-title">
        <h3>Remote Providers</h3>
        <span>{providers.length} installed</span>
      </div>

      <div className="settings-grid remote-provider-add-form">
        <label>
          Manifest URL
          <input
            data-testid="remote-provider-url-input"
            type="text"
            value={url}
            onChange={(event) => setUrl(event.currentTarget.value)}
            placeholder="https://example.com/provider.json"
          />
        </label>
        <label>
          Provider proxy URL (optional)
          <input
            data-testid="remote-provider-proxy-url-input"
            type="text"
            value={proxyUrl}
            onChange={(event) => setProxyUrl(event.currentTarget.value)}
            placeholder="http://proxy:8080"
          />
        </label>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={autoUpdate}
            onChange={(event) => setAutoUpdate(event.currentTarget.checked)}
          />
          Auto-update when available
        </label>
        <button
          type="button"
          className="button-secondary"
          onClick={handlePreview}
          disabled={loading || !url.trim()}
        >
          {loading && preview === null ? "Loading..." : "Preview"}
        </button>
      </div>

      {message ? <div className="settings-message">{message}</div> : null}

      {providers.length === 0 ? (
        <p>No remote providers installed</p>
      ) : (
        <ul className="remote-provider-list">
          {providers.map((provider) => {
            const update = updates.find((u) => u.id === provider.id);
            return (
              <li key={provider.id} className="remote-provider-item">
                <div className="remote-provider-summary">
                  <strong>{provider.name}</strong>
                  <span>{provider.id}</span>
                  <span>{provider.runtime}</span>
                  {provider.autoUpdate ? <span>auto-update</span> : null}
                </div>
                <div className="remote-provider-url">{provider.manifestUrl}</div>
                <div className="remote-provider-actions">
                  <button
                    type="button"
                    className="button-secondary"
                    onClick={() => void handleRefresh(provider.id)}
                  >
                    Refresh
                  </button>
                  <button
                    type="button"
                    className="button-secondary"
                    onClick={() => void handleCheckUpdates()}
                  >
                    Check Updates
                  </button>
                  {update?.available ? (
                    <button
                      type="button"
                      className="button-primary"
                      onClick={() => void handleApplyUpdate(provider.id)}
                    >
                      Apply Update
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="button-danger"
                    onClick={() => void handleRemove(provider.id)}
                  >
                    Remove
                  </button>
                </div>
              </li>
            );
          })}
        </ul>
      )}

      <InstallRemoteProviderDialog
        preview={preview}
        loading={loading}
        onConfirm={() => void handleConfirmInstall()}
        onCancel={() => setPreview(null)}
      />
    </section>
  );
}
