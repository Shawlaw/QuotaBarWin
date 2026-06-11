import type { RemoteProviderPreview } from "../lib/api";

type InstallRemoteProviderDialogProps = {
  preview: RemoteProviderPreview | null;
  loading: boolean;
  onConfirm: () => void;
  onCancel: () => void;
};

export function InstallRemoteProviderDialog({
  preview,
  loading,
  onConfirm,
  onCancel
}: InstallRemoteProviderDialogProps) {
  if (!preview) {
    return null;
  }

  return (
    <div className="dialog-overlay" role="presentation" data-testid="install-remote-provider-dialog">
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="install-remote-provider-title">
        <h3 id="install-remote-provider-title">Install Remote Provider</h3>
        <p>
          You are about to install a remote provider. Make sure you trust the source before proceeding.
        </p>
        <dl className="remote-provider-preview-list">
          <div>
            <dt>Name</dt>
            <dd>{preview.name}</dd>
          </div>
          <div>
            <dt>ID</dt>
            <dd>{preview.id}</dd>
          </div>
          {preview.description ? (
            <div>
              <dt>Description</dt>
              <dd>{preview.description}</dd>
            </div>
          ) : null}
          <div>
            <dt>Runtime</dt>
            <dd>{preview.runtime}</dd>
          </div>
          <div>
            <dt>Source URL</dt>
            <dd>{preview.sourceUrl}</dd>
          </div>
          {preview.requiredEnvVars.length > 0 ? (
            <div>
              <dt>Required Environment Variables</dt>
              <dd>{preview.requiredEnvVars.join(", ")}</dd>
            </div>
          ) : null}
          {preview.checksum ? (
            <div>
              <dt>Source Checksum</dt>
              <dd>{preview.checksum}</dd>
            </div>
          ) : null}
        </dl>
        <div className="dialog-actions">
          <button
            type="button"
            className="button-primary"
            onClick={onConfirm}
            disabled={loading}
            data-testid="install-remote-provider-confirm"
          >
            {loading ? "Installing..." : "Install"}
          </button>
          <button
            type="button"
            className="button-secondary"
            onClick={onCancel}
            disabled={loading}
            data-testid="install-remote-provider-cancel"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
