import type { DisplayMode } from "../lib/providerStatus";
import type { ProviderStatus } from "../types";

export type LanguageChoice = "system" | "en" | "zh-CN";
export type ResolvedLanguage = Exclude<LanguageChoice, "system">;

export type I18nCatalog = {
  app: {
    overviewLabel: string;
    providersLabel: string;
    versionTitle: (version: string) => string;
  };
  header: {
    overview: string;
    settings: string;
    refresh: string;
    refreshing: string;
  };
  globalStatus: {
    noProviders: string;
    refreshFailed: (providerName: string) => string;
    needsAttention: (providerName: string, detail?: string) => string;
    belowThreshold: (windowLabel: string, threshold: number) => string;
    dataStale: (providerName: string) => string;
    summary: (providerCount: number, updatedAt: string, refreshIntervalSeconds: number) => string;
    recently: string;
  };
  providerCard: {
    statusLabel: (status: ProviderStatus) => string;
    lastUpdated: (time: string) => string;
    providerSource: (source: string) => string;
    refreshFailedPrefix: string;
    showingCachedDataPrefix: string;
    refresh: string;
    refreshing: string;
    noQuotaWindows: string;
    noResetTime: string;
    progressUnavailable: string;
    showLess: string;
    moreQuotaWindows: (count: number) => string;
    exitCode: string;
    duration: string;
  };
  tray: {
    waitingForData: string;
    lastRefreshedAt: (time: string) => string;
    openMainWindow: string;
    openMainWindowShort: string;
    refresh: string;
    refreshingShort: string;
    close: string;
    resize: string;
    resetSize: string;
    noProviders: string;
    providerStatusLabel: string;
    quotaWindowsLabel: string;
    status: Record<ProviderStatus, string>;
    healthSummary: (providerName: string, status: ProviderStatus, extraCount: number) => string;
  };
  settings: {
    title: string;
    general: string;
    refreshInterval: string;
    refreshIntervalError: string;
    displayMode: string;
    displayRemaining: string;
    displayUsed: string;
    lowQuotaWarning: string;
    lowQuotaWarningError: string;
    logLevel: string;
    logDebug: string;
    logInfo: string;
    logWarn: string;
    logError: string;
    logMaxSize: string;
    logMaxSizeError: string;
    logQuotaData: string;
    logQuotaDataHint: string;
    logQuotaDataConfirmTitle: string;
    logQuotaDataConfirm: string;
    logQuotaDataConfirmAction: string;
    cancel: string;
    language: string;
    languageSystem: string;
    languageEnglish: string;
    languageChinese: string;
    theme: string;
    themeSystem: string;
    themeLight: string;
    themeDark: string;
    launchAtStartup: string;
    providers: string;
    configuredCount: (count: number) => string;
    addProvider: string;
    noProviders: string;
    edit: string;
    collapse: string;
    more: string;
    up: string;
    down: string;
    remove: string;
    removeProviderConfirm: (providerName: string) => string;
    removeManagedSecrets: string;
    removeManagedSecretsHint: string;
    setEnvVar: (name: string) => string;
    name: string;
    authToken: string;
    authTokenPlaceholder: string;
    accountId: string;
    optional: string;
    proxyUrl: string;
    providerProxyPlaceholder: string;
    timeout: string;
    timeoutError: string;
    providerParameters: string;
    providerParametersHint: string;
    parameterRequired: string;
    parameterKind: (kind: string) => string;
    parameterDefault: (value: string) => string;
    parameterPlaceholder: (value: string) => string;
    parameterOptions: (value: string) => string;
    remoteEnvVars: string;
    remoteEnvVarsPlaceholder: string;
    remoteEnvVarsHint: string;
    windowLabelOverrides: string;
    windowLabelOverridesPlaceholder: string;
    displayedWindows: string;
    showInTray: string;
    displayedWindowsPlaceholder: string;
    windowDisplay: string;
    windowDisplayNoSnapshot: string;
    windowDisplayNoRows: string;
    show: string;
    windowId: string;
    defaultLabel: string;
    customLabel: string;
    customLabelFor: (windowId: string) => string;
    order: string;
    showWindow: (windowId: string) => string;
    showAllWindows: string;
    resetWindowNames: string;
    resetWindowOrder: string;
    moveWindowUp: (windowId: string) => string;
    moveWindowDown: (windowId: string) => string;
    advancedWindowText: string;
    configurationStorage: string;
    portableMode: string;
    appDataMode: string;
    configFile: string;
    appData: string;
    portable: string;
    portableMarker: string;
    loadingConfigPath: string;
    loading: string;
    portableModeHint: string;
    openFolder: string;
    resetConfig: string;
    resetConfigConfirm: string;
    noChanges: string;
    saving: string;
    saved: string;
    saveFailed: string;
    unsavedChanges: string;
    unsavedChangesTitle: string;
    unsavedChangesPrompt: string;
    saveAndContinue: string;
    discardChanges: string;
    resetChanges: string;
    save: string;
  };
  appUpdate: {
    title: string;
    currentVersion: (version: string) => string;
    check: string;
    checking: string;
    upToDate: string;
    available: (version: string) => string;
    downloadAndRestart: string;
    downloading: string;
    notes: string;
    unavailable: string;
    failedToCheck: string;
    failedToDownload: string;
    failedToOpenNotes: string;
    autoCheck: string;
    autoCheckHint: string;
    lastChecked: (time: string) => string;
    checkFailed: string;
    openUpdate: string;
    later: string;
  };
  networkProxy: {
    label: string;
    noProxy: string;
    systemProxy: string;
    httpProxy: string;
    socks5Proxy: string;
    proxyUrl: string;
    proxyUrlPlaceholder: string;
    testProxy: string;
    testingProxy: string;
    testProxyHint: string;
    customTestUrl: string;
    testUrl: string;
    testUrlPlaceholder: string;
    testProxySuccess: (statusCode: number | null, elapsedMs: number) => string;
    testProxyNoProxy: string;
    testProxyInvalidTarget: string;
    testProxyInvalidProxy: string;
    testProxyRequestFailed: string;
    testProxyHttpStatus: (statusCode: number | null) => string;
    testProxyUnavailable: string;
  };
  localApi: {
    title: string;
    enabled: string;
    enabledHint: string;
    listenTarget: string;
    loopback: string;
    selectedNetworkInterfaces: string;
    allNetworkInterfaces: string;
    selectedInterfacesHint: string;
    includeLoopback: string;
    includeLoopbackHint: string;
    networkInterface: (name: string, address: string) => string;
    noNetworkInterfaces: string;
    noInterfacesSelected: string;
    port: string;
    portError: string;
    endpoint: string;
    running: string;
    stopped: string;
    unavailable: string;
    authentication: string;
    authenticationRequired: string;
    localAuthenticationNotRequired: string;
    token: string;
    tokenPlaceholder: string;
    copyToken: string;
    replaceToken: string;
    saveToken: string;
    generateToken: string;
    tokenHint: string;
    tokenConfigured: string;
    tokenRequiredBeforeSave: string;
    networkWarning: string;
    tokenSaved: string;
    tokenSaveFailed: string;
    tokenCopied: string;
    tokenCopyFailed: string;
    interfaceLoadFailed: string;
  };
  remoteProviders: {
    title: string;
    addTitle: string;
    manageSourcesTitle: string;
    recommendedProviders: string;
    customInstall: string;
    securityNoticeLead: string;
    securityNoticeBody: string;
    sourceSummary: string;
    sourceSettings: string;
    projectMaintainedSource: string;
    customSource: string;
    sourceName: string;
    sourceUrl: string;
    sourceEnabled: string;
    addSource: string;
    removeSource: string;
    migrateSource: string;
    migratingSource: string;
    migrateSourceConfirm: (sourceName: string) => string;
    migrateSourceResult: (migrated: number, skipped: number, failed: number) => string;
    failedToMigrateSource: string;
    enabledSourcesCount: (count: number) => string;
    sourceLoadError: (sourceName: string, message: string) => string;
    sourceConflict: (providerId: string, sourceNames: string) => string;
    backToSettings: string;
    backToAddProvider: string;
    manageSources: string;
    refreshCatalog: string;
    installProvider: string;
    addAccount: string;
    addingAccount: string;
    installed: string;
    installedAccounts: (count: number) => string;
    installFromManifest: string;
    manifestInstallUrl: string;
    manifestUrlPlaceholder: string;
    manifestChecksum: string;
    manifestChecksumPlaceholder: string;
    noCatalogProviders: string;
    catalogLoadFailed: string;
    providerInstalled: string;
    accountAdded: string;
    installProviderFailed: string;
    installedCount: (count: number) => string;
    openGuide: string;
    registryUrl: string;
    registryUrlPlaceholder: string;
    providerProxyUrl: string;
    providerProxyPlaceholder: string;
    autoUpdateWhenAvailable: string;
    loading: string;
    installRegistry: string;
    installedResult: (count: number) => string;
    skippedResult: (count: number) => string;
    failedResult: (count: number) => string;
    noProvidersInstalledFromRegistry: string;
    failedToInstallRegistry: string;
    providerRemoved: string;
    failedToRemoveProvider: string;
    updateAvailable: string;
    providerRefreshed: string;
    failedToRefreshProvider: string;
    updatesAvailable: (count: number) => string;
    allProvidersUpToDate: string;
    failedToCheckUpdates: string;
    updateApplied: string;
    failedToApplyUpdate: string;
    noRemoteProvidersInstalled: string;
    autoUpdate: string;
    details: string;
    collapse: string;
    refresh: string;
    checkUpdates: string;
    checkingUpdates: string;
    applyUpdate: string;
    applyAllUpdates: (count: number) => string;
    applyingUpdates: string;
    updatesApplied: (count: number) => string;
    updateSomeFailed: (updated: number, failed: number) => string;
    remove: string;
    version: string;
    unknownVersion: string;
    installedAt: string;
    updatedAt: string;
    lastCheckedAt: string;
    runtime: string;
    manifestUrl: string;
  };
  providerSetup: {
    title: (name: string) => string;
    accountName: string;
    required: string;
    configured: string;
    notConfigured: string;
    changeSecret: string;
    clearSecret: string;
    managedSecretHint: (providerId: string, parameterName: string) => string;
    existingCredentialHint: string;
    enterSecret: string;
    replaceSecret: string;
    advancedSettings: string;
    saveAndTest: string;
    autoDetectAndFinish: string;
    autoDetectHint: string;
    saveWithoutTesting: string;
    testing: string;
    testSucceeded: string;
    testFailed: string;
    complete: string;
    back: string;
    statePending: string;
    stateUnverified: string;
    stateReady: string;
    stateNeedsAttention: string;
    setup: string;
    completeSetup: string;
    testConfiguration: string;
    repairConfiguration: string;
    providerPreview: (count: number) => string;
    missingRequired: (name: string) => string;
    invalidNumber: (name: string) => string;
    credentialSource: (source: string) => string;
    credentialSourceType: (source: string) => string;
    errorCategory: (category: string) => string;
    saveFailed: string;
    testFailedKeepSaved: string;
    noUsableProvidersTitle: string;
    noUsableProvidersBody: string;
    addProvider: string;
    openGuide: string;
  };
  format: {
    unknown: string;
    remainingUnknown: string;
    usageUnknown: string;
    displayValue: (percent: number, displayMode: DisplayMode) => string;
    resets: (time: string) => string;
  };
};

export const en: I18nCatalog = {
  app: {
    overviewLabel: "Overview",
    providersLabel: "Providers",
    versionTitle: (version) => `Version ${version}`
  },
  header: {
    overview: "Overview",
    settings: "Settings",
    refresh: "Refresh",
    refreshing: "Refreshing"
  },
  globalStatus: {
    noProviders: "No providers configured. Add a provider in Settings.",
    refreshFailed: (providerName) => `${providerName} refresh failed`,
    needsAttention: (providerName, detail) =>
      `${providerName} needs attention${detail ? ` · ${detail}` : ""}`,
    belowThreshold: (windowLabel, threshold) => `${windowLabel} below ${threshold}%`,
    dataStale: (providerName) => `${providerName} data is stale`,
    summary: (providerCount, updatedAt, refreshIntervalSeconds) =>
      `${providerCount} providers active · Last updated ${updatedAt} · Auto refresh every ${refreshIntervalSeconds}s`,
    recently: "recently"
  },
  providerCard: {
    statusLabel: (status) => `status ${status}`,
    lastUpdated: (time) => `Last updated ${time}`,
    providerSource: (source) => `${source} provider`,
    refreshFailedPrefix: "Refresh failed · ",
    showingCachedDataPrefix: "Showing cached data · ",
    refresh: "Refresh",
    refreshing: "Refreshing",
    noQuotaWindows: "No quota windows reported yet.",
    noResetTime: "No reset time",
    progressUnavailable: "Progress unavailable",
    showLess: "Show less",
    moreQuotaWindows: (count) => `+ ${count} more quota windows`,
    exitCode: "Exit code",
    duration: "Duration"
  },
  tray: {
    waitingForData: "Waiting for data",
    lastRefreshedAt: (time) => `Last refreshed at ${time}`,
    openMainWindow: "Open main window",
    openMainWindowShort: "Open",
    refresh: "Refresh",
    refreshingShort: "...",
    close: "Close",
    resize: "Resize",
    resetSize: "Reset size",
    noProviders: "No providers configured.",
    providerStatusLabel: "Provider status",
    quotaWindowsLabel: "Quota windows",
    status: {
      ok: "OK",
      warning: "Low",
      error: "Error",
      stale: "Stale",
      unknown: "Unknown"
    },
    healthSummary: (providerName, status, extraCount) =>
      `${providerName} ${en.tray.status[status]}${extraCount > 0 ? ` +${extraCount}` : ""}`
  },
  settings: {
    title: "Settings",
    general: "General",
    refreshInterval: "Refresh interval (seconds)",
    refreshIntervalError: "Refresh interval must be greater than 0.",
    displayMode: "Display mode",
    displayRemaining: "Remaining",
    displayUsed: "Used",
    lowQuotaWarning: "Low quota warning",
    lowQuotaWarningError: "Low quota warning must be between 0 and 100.",
    logLevel: "Log level",
    logDebug: "Debug",
    logInfo: "Info",
    logWarn: "Warn",
    logError: "Error",
    logMaxSize: "Local log limit (MB)",
    logMaxSizeError: "Local log limit must be at least 1 MB.",
    logQuotaData: "Log refreshed quota data",
    logQuotaDataHint:
      "When enabled, each enabled provider's refreshed quota values are written to the local log.",
    logQuotaDataConfirmTitle: "Enable quota data logging?",
    logQuotaDataConfirm:
      "This will write refreshed quota data for all enabled providers to quotabarwin.log. The data may reveal usage patterns, quota state, or account type. Do not share plaintext logs with others after enabling it.",
    logQuotaDataConfirmAction: "Enable logging",
    cancel: "Cancel",
    language: "Language",
    languageSystem: "System",
    languageEnglish: "English",
    languageChinese: "Simplified Chinese",
    theme: "Theme",
    themeSystem: "Follow system",
    themeLight: "Light",
    themeDark: "Dark",
    launchAtStartup: "Launch at startup",
    providers: "Providers",
    configuredCount: (count) => `${count} configured`,
    addProvider: "Add Provider",
    noProviders: "No providers yet. Add one to start monitoring quota.",
    edit: "Edit",
    collapse: "Collapse",
    more: "More",
    up: "Up",
    down: "Down",
    remove: "Remove",
    removeProviderConfirm: (providerName) => `Remove provider ${providerName}?`,
    removeManagedSecrets: "Also delete this account's QuotaBarWin-managed local secret files",
    removeManagedSecretsHint: "External files, environment variables, and manually named secret files are never removed.",
    setEnvVar: (name) => `Set ${name}`,
    name: "Name",
    authToken: "Auth token",
    authTokenPlaceholder:
      "Paste a token, ${secret:CODEX_ACCESS_TOKEN}, ${env:CODEX_ACCESS_TOKEN}, or ${file:C:\\Secrets\\codex-token.txt}",
    accountId: "ChatGPT account id",
    optional: "Optional",
    proxyUrl: "Proxy URL",
    providerProxyPlaceholder: "Optional, e.g. http://127.0.0.1:7890 or socks5h://127.0.0.1:7890",
    timeout: "Timeout (seconds)",
    timeoutError: "Timeout must be greater than 0.",
    providerParameters: "Supported parameters",
    providerParametersHint: "Hints come from this provider's manifest.",
    parameterRequired: "Required",
    parameterKind: (kind) => `Type: ${kind}`,
    parameterDefault: (value) => `Default: ${value}`,
    parameterPlaceholder: (value) => `Placeholder: ${value}`,
    parameterOptions: (value) => `Options: ${value}`,
    remoteEnvVars: "Environment variables",
    remoteEnvVarsPlaceholder:
      "KEY=value, one per line\nKIMI_API_KEY=${secret:KIMI_WORK_API_KEY}\nDEEPSEEK_API_KEY=${secret:DEEPSEEK_API_KEY}\nDEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY=200\nDEEPSEEK_BALANCE_WARNING_CNY=20",
    remoteEnvVarsHint:
      "For multiple accounts, keep the left side as the script env var and change the secret name on the right, e.g. KIMI_API_KEY=${secret:KIMI_WORK_API_KEY} reads secrets/KIMI_WORK_API_KEY.txt.",
    windowLabelOverrides: "Window label overrides",
    windowLabelOverridesPlaceholder: "window-id=Display name\n5h=5h\nweekly=Weekly limit",
    displayedWindows: "Displayed windows",
    showInTray: "Show in tray popup",
    displayedWindowsPlaceholder: "Leave empty to show all\n5h\nweekly\nWeekly limit",
    windowDisplay: "Window display",
    windowDisplayNoSnapshot: "No recent snapshot windows yet.",
    windowDisplayNoRows: "Add window ids below to create editable rows.",
    show: "Show",
    windowId: "Window id",
    defaultLabel: "Default label",
    customLabel: "Custom label",
    customLabelFor: (windowId) => `Custom label for ${windowId}`,
    order: "Order",
    showWindow: (windowId) => `Show ${windowId}`,
    showAllWindows: "Show all",
    resetWindowNames: "Reset names",
    resetWindowOrder: "Default order",
    moveWindowUp: (windowId) => `Move ${windowId} up`,
    moveWindowDown: (windowId) => `Move ${windowId} down`,
    advancedWindowText: "Advanced window text",
    configurationStorage: "Configuration storage",
    portableMode: "Portable mode",
    appDataMode: "AppData mode",
    configFile: "Config file",
    appData: "AppData",
    portable: "Portable",
    portableMarker: "Marker file",
    loadingConfigPath: "Loading config path...",
    loading: "Loading...",
    portableModeHint:
      "Portable mode stores config beside the app executable and uses quotabarwin.portable as the marker file.",
    openFolder: "Open config storage folder",
    resetConfig: "Reset config",
    resetConfigConfirm: "Reset QuotaBarWin config to defaults? A backup will be created first.",
    noChanges: "No changes",
    saving: "Saving",
    saved: "Saved",
    saveFailed: "Save failed",
    unsavedChanges: "Unsaved changes",
    unsavedChangesTitle: "Unsaved changes",
    unsavedChangesPrompt:
      "Save your changes before continuing? You can also discard them or stay on this page.",
    saveAndContinue: "Save and continue",
    discardChanges: "Discard changes",
    resetChanges: "Reset changes",
    save: "Save"
  },
  appUpdate: {
    title: "Application update",
    currentVersion: (version) => `Current version: ${version}`,
    check: "Check for updates",
    checking: "Checking...",
    upToDate: "QuotaBarWin is up to date.",
    available: (version) => `QuotaBarWin ${version} is available.`,
    downloadAndRestart: "Download and restart to update",
    downloading: "Downloading update...",
    notes: "Release notes",
    unavailable: "Application updates are not configured in this build.",
    failedToCheck: "Failed to check application updates",
    failedToDownload: "Failed to download application update",
    failedToOpenNotes: "Failed to open release notes",
    autoCheck: "Automatically check for application updates",
    autoCheckHint: "Checks once when you first open the main window or tray popup after 08:00. It never downloads or restarts automatically.",
    lastChecked: (time) => `Last automatic check: ${time}`,
    checkFailed: "The last automatic check failed. You can try again manually.",
    openUpdate: "Update",
    later: "Later"
  },
  networkProxy: {
    label: "Network proxy",
    noProxy: "No proxy",
    systemProxy: "System proxy",
    httpProxy: "HTTP proxy",
    socks5Proxy: "SOCKS5 proxy",
    proxyUrl: "Proxy URL",
    proxyUrlPlaceholder: "http://host:port or socks5://host:port",
    testProxy: "Test proxy",
    testingProxy: "Testing proxy...",
    testProxyHint:
      "Uses the current proxy settings to request the GitHub homepage. Response content is not stored.",
    customTestUrl: "Use a custom test URL",
    testUrl: "Test URL",
    testUrlPlaceholder: "https://example.com/",
    testProxySuccess: (statusCode, elapsedMs) =>
      `Proxy is available (${statusCode ?? "no status"}, ${elapsedMs} ms).`,
    testProxyNoProxy: "Configure a system, HTTP, or SOCKS5 proxy first.",
    testProxyInvalidTarget: "The test URL must be a valid HTTPS address.",
    testProxyInvalidProxy: "The proxy address is invalid.",
    testProxyRequestFailed: "The proxy could not reach the test address.",
    testProxyHttpStatus: (statusCode) =>
      `The test address returned HTTP ${statusCode ?? "an unexpected status"}.`,
    testProxyUnavailable: "Proxy testing is available only in the desktop app."
  },
  localApi: {
    title: "Local integration API",
    enabled: "Enable local integration API",
    enabledHint: "The desktop app serves the current normalized quota snapshot while it is running.",
    listenTarget: "Listen on",
    loopback: "This computer only (127.0.0.1)",
    selectedNetworkInterfaces: "Selected network interfaces",
    allNetworkInterfaces: "All active network interfaces",
    selectedInterfacesHint: "Select one or more active network interfaces with IPv4 or IPv6 addresses.",
    includeLoopback: "Also listen on this computer",
    includeLoopbackHint: "127.0.0.1 remains available without a token.",
    networkInterface: (name, address) => `${name} (${address})`,
    noNetworkInterfaces: "No active IPv4 or IPv6 network interfaces are available.",
    noInterfacesSelected: "Select at least one network interface.",
    port: "Port",
    portError: "Port must be between 1 and 65535.",
    endpoint: "Endpoint",
    running: "Running",
    stopped: "Stopped",
    unavailable: "Unavailable",
    authentication: "Authentication",
    authenticationRequired: "A bearer token is required for network access.",
    localAuthenticationNotRequired: "Loopback access does not require a token.",
    token: "Access token",
    tokenPlaceholder: "Enter at least 32 non-whitespace characters",
    copyToken: "Copy token",
    replaceToken: "Replace token",
    saveToken: "Save token",
    generateToken: "Generate new token",
    tokenHint: "Use Authorization: Bearer <token>. Tokens are stored separately from the main configuration and are never written to logs.",
    tokenConfigured: "An access token is saved and shown in masked form.",
    tokenRequiredBeforeSave: "Save an access token before saving network listener settings.",
    networkWarning: "Network access is exposed over plain HTTP. Keep this on a trusted network, use a strong token, and do not expose the port to the public Internet.",
    tokenSaved: "Access token saved.",
    tokenSaveFailed: "Failed to save the access token.",
    tokenCopied: "Access token copied.",
    tokenCopyFailed: "Unable to copy the access token.",
    interfaceLoadFailed: "Unable to load active network interfaces.",
  },
  remoteProviders: {
    title: "Remote Sources",
    addTitle: "Add Provider",
    manageSourcesTitle: "Provider Source",
    recommendedProviders: "Recommended",
    customInstall: "Custom install",
    securityNoticeLead: "Security notice: only install and use Providers you trust.",
    securityNoticeBody:
      "Provider scripts can read the AI credentials configured for them and make network requests.",
    sourceSummary: "Current source",
    sourceSettings: "Source settings",
    projectMaintainedSource: "Project-maintained source",
    customSource: "Custom source",
    sourceName: "Source name",
    sourceUrl: "Registry URL or local path",
    sourceEnabled: "Enabled",
    addSource: "Add Source",
    removeSource: "Remove Source",
    migrateSource: "Migrate installed Providers",
    migratingSource: "Migrating...",
    migrateSourceConfirm: (sourceName) =>
      `Move matching installed Providers to ${sourceName}? Their account settings stay unchanged, but their manifest and script will be replaced from this source.`,
    migrateSourceResult: (migrated, skipped, failed) =>
      `Migration complete: ${migrated} migrated, ${skipped} skipped, ${failed} failed.`,
    failedToMigrateSource: "Failed to migrate installed Providers",
    enabledSourcesCount: (count) => `${count} source(s) enabled`,
    sourceLoadError: (sourceName, message) => `${sourceName}: ${message}`,
    sourceConflict: (providerId, sourceNames) =>
      `Provider id '${providerId}' appears in multiple sources: ${sourceNames}`,
    backToSettings: "Back to Settings",
    backToAddProvider: "Back to Add Provider",
    manageSources: "Manage Source",
    refreshCatalog: "Refresh List",
    installProvider: "Install",
    addAccount: "Add account",
    addingAccount: "Adding account",
    installed: "Installed",
    installedAccounts: (count) => `${count} account${count === 1 ? "" : "s"}`,
    installFromManifest: "Install Provider",
    manifestInstallUrl: "Provider manifest URL or local path",
    manifestUrlPlaceholder: "https://.../provider.json or D:\\Providers\\provider.json",
    manifestChecksum: "Manifest checksum (optional)",
    manifestChecksumPlaceholder: "sha256:...",
    noCatalogProviders: "No providers found in this source",
    catalogLoadFailed: "Failed to load provider list",
    providerInstalled: "Provider installed",
    accountAdded: "Account added",
    installProviderFailed: "Failed to install provider",
    installedCount: (count) => `${count} installed`,
    openGuide: "Open Guide",
    registryUrl: "Registry URL",
    registryUrlPlaceholder: "https://... or file:///... or local path to registry.json",
    providerProxyUrl: "Installation source proxy URL (optional)",
    providerProxyPlaceholder: "http://host:port or socks5://host:port",
    autoUpdateWhenAvailable: "Auto-update when available",
    loading: "Loading...",
    installRegistry: "Install Registry",
    installedResult: (count) => `${count} installed`,
    skippedResult: (count) => `${count} skipped`,
    failedResult: (count) => `${count} failed`,
    noProvidersInstalledFromRegistry: "No providers installed from registry",
    failedToInstallRegistry: "Failed to install registry",
    providerRemoved: "Provider removed",
    failedToRemoveProvider: "Failed to remove provider",
    updateAvailable: "Update available",
    providerRefreshed: "Provider refreshed",
    failedToRefreshProvider: "Failed to refresh provider",
    updatesAvailable: (count) => `${count} update(s) available`,
    allProvidersUpToDate: "All providers are up to date",
    failedToCheckUpdates: "Failed to check updates",
    updateApplied: "Update applied",
    failedToApplyUpdate: "Failed to apply update",
    noRemoteProvidersInstalled: "No remote providers installed",
    autoUpdate: "auto-update",
    details: "Details",
    collapse: "Collapse",
    refresh: "Refresh",
    checkUpdates: "Check Updates",
    checkingUpdates: "Checking...",
    applyUpdate: "Apply Update",
    applyAllUpdates: (count) => `Apply all updates (${count})`,
    applyingUpdates: "Applying updates...",
    updatesApplied: (count) => `${count} update(s) applied`,
    updateSomeFailed: (updated, failed) => `${updated} updated, ${failed} failed`,
    remove: "Remove",
    version: "Version",
    unknownVersion: "unknown version",
    installedAt: "Installed",
    updatedAt: "Updated",
    lastCheckedAt: "Last checked",
    runtime: "Runtime",
    manifestUrl: "Manifest"
  },
  providerSetup: {
    title: (name) => `Set up ${name}`,
    accountName: "Account name",
    required: "Required",
    configured: "Configured",
    notConfigured: "Not configured",
    changeSecret: "Change",
    clearSecret: "Clear",
    managedSecretHint: (providerId, parameterName) =>
      `After you save, QuotaBarWin creates an isolated local file for this Provider instance at <config folder>\\secrets\\providers\\${providerId}\\${parameterName}.txt. The main configuration keeps only a reference, and the value is never shown here.`,
    existingCredentialHint: "Leave the field empty to keep the current credential. Enter a new value only when you want to replace it.",
    enterSecret: "Enter API key",
    replaceSecret: "Enter a new credential to replace the current one",
    advancedSettings: "Advanced settings",
    saveAndTest: "Save and test",
    autoDetectAndFinish: "Auto-detect and finish",
    autoDetectHint: "This Provider first tries to use its local sign-in information. You can test it without filling the optional fields.",
    saveWithoutTesting: "Save without testing",
    testing: "Testing configuration...",
    testSucceeded: "Configuration works. This Provider is now enabled.",
    testFailed: "Configuration test failed",
    complete: "Done",
    back: "Back",
    statePending: "Pending setup",
    stateUnverified: "Unverified",
    stateReady: "Ready",
    stateNeedsAttention: "Needs attention",
    setup: "Set up",
    completeSetup: "Complete setup",
    testConfiguration: "Test configuration",
    repairConfiguration: "Repair configuration",
    providerPreview: (count) => `Connected successfully${count ? ` · ${count} quota window(s)` : ""}`,
    missingRequired: (name) => `${name} is required.`,
    invalidNumber: (name) => `${name} must be a number.`,
    credentialSource: (source) => `Credential source: ${source}`,
    credentialSourceType: (source) => {
      const labels: Record<string, string> = {
        managedLocalFile: "application-managed local file",
        secretFile: "local secret file",
        environment: "environment variable",
        externalFile: "external file",
        literal: "config value",
        default: "Provider default",
        advancedExpression: "advanced configuration",
        missing: "not configured",
      };
      return labels[source] ?? labels.missing;
    },
    errorCategory: (category) => {
      const labels: Record<string, string> = {
        missingCredential: "Missing account credential",
        authentication: "Credential is invalid or expired",
        network: "Unable to connect to the service",
        proxy: "Proxy connection failed",
        timeout: "Request timed out",
        runtime: "Provider runtime error",
        permission: "Provider permission is insufficient",
        providerOutput: "Provider returned an invalid response",
        unknown: "Unable to determine the cause",
      };
      return labels[category] ?? labels.unknown;
    },
    saveFailed: "Unable to save Provider setup",
    testFailedKeepSaved: "Your configuration was saved safely but is still disabled. Update it and try again.",
    noUsableProvidersTitle: "No quota source is ready yet",
    noUsableProvidersBody: "Add a Provider and set up an account to see quota information here.",
    addProvider: "Add Provider",
    openGuide: "View guide"
  },
  format: {
    unknown: "Unknown",
    remainingUnknown: "Remaining unknown",
    usageUnknown: "Usage unknown",
    displayValue: (percent, displayMode) => `${percent}% ${displayMode}`,
    resets: (time) => `resets ${time}`
  }
};

export const zhCN: I18nCatalog = {
  app: {
    overviewLabel: "概览",
    providersLabel: "提供方",
    versionTitle: (version) => `版本 ${version}`
  },
  header: {
    overview: "概览",
    settings: "设置",
    refresh: "刷新",
    refreshing: "刷新中"
  },
  globalStatus: {
    noProviders: "尚未配置提供方。请在设置中添加提供方。",
    refreshFailed: (providerName) => `${providerName} 刷新失败`,
    needsAttention: (providerName, detail) =>
      `${providerName} 需要注意${detail ? ` · ${detail}` : ""}`,
    belowThreshold: (windowLabel, threshold) => `${windowLabel} 低于 ${threshold}%`,
    dataStale: (providerName) => `${providerName} 数据已过期`,
    summary: (providerCount, updatedAt, refreshIntervalSeconds) =>
      `${providerCount} 个提供方启用 · 上次更新 ${updatedAt} · 每 ${refreshIntervalSeconds}s 自动刷新`,
    recently: "刚刚"
  },
  providerCard: {
    statusLabel: (status) => `状态 ${status}`,
    lastUpdated: (time) => `上次更新 ${time}`,
    providerSource: (source) => `${source} 提供方`,
    refreshFailedPrefix: "刷新失败 · ",
    showingCachedDataPrefix: "正在显示缓存数据 · ",
    refresh: "刷新",
    refreshing: "刷新中",
    noQuotaWindows: "尚未报告额度窗口。",
    noResetTime: "无重置时间",
    progressUnavailable: "进度不可用",
    showLess: "收起",
    moreQuotaWindows: (count) => `+ ${count} 个额度窗口`,
    exitCode: "退出码",
    duration: "耗时"
  },
  tray: {
    waitingForData: "等待数据",
    lastRefreshedAt: (time) => `上次刷新于 ${time}`,
    openMainWindow: "打开主窗口",
    openMainWindowShort: "打开",
    refresh: "刷新",
    refreshingShort: "...",
    close: "关闭",
    resize: "调整大小",
    resetSize: "恢复默认尺寸",
    noProviders: "尚未配置提供方。",
    providerStatusLabel: "提供方状态",
    quotaWindowsLabel: "额度窗口",
    status: {
      ok: "正常",
      warning: "偏低",
      error: "错误",
      stale: "过期",
      unknown: "未知"
    },
    healthSummary: (providerName, status, extraCount) =>
      `${providerName} ${zhCN.tray.status[status]}${extraCount > 0 ? ` +${extraCount}` : ""}`
  },
  settings: {
    title: "设置",
    general: "通用",
    refreshInterval: "刷新间隔（秒）",
    refreshIntervalError: "刷新间隔必须大于 0。",
    displayMode: "显示模式",
    displayRemaining: "剩余",
    displayUsed: "已用",
    lowQuotaWarning: "低额度警告",
    lowQuotaWarningError: "低额度警告必须介于 0 到 100 之间。",
    logLevel: "日志级别",
    logDebug: "调试",
    logInfo: "信息",
    logWarn: "警告",
    logError: "错误",
    logMaxSize: "本地日志保留上限（MB）",
    logMaxSizeError: "本地日志保留上限至少为 1 MB。",
    logQuotaData: "记录刷新后的额度数据",
    logQuotaDataHint:
      "开启后，每个已启用 Provider 刷新得到的额度数值都会写入本地日志。",
    logQuotaDataConfirmTitle: "开启额度数据日志？",
    logQuotaDataConfirm:
      "开启后，所有已启用 Provider 的刷新额度数据都会写入 quotabarwin.log。这些数据可能暴露使用节奏、额度状态或账号类型。开启后请不要将明文日志发给他人。",
    logQuotaDataConfirmAction: "开启日志",
    cancel: "取消",
    language: "语言",
    languageSystem: "跟随系统",
    languageEnglish: "English",
    languageChinese: "简体中文",
    theme: "主题",
    themeSystem: "跟随系统",
    themeLight: "浅色",
    themeDark: "深色",
    launchAtStartup: "开机启动",
    providers: "提供方",
    configuredCount: (count) => `已配置 ${count} 个`,
    addProvider: "添加提供方",
    noProviders: "尚无提供方。添加一个以开始监控额度。",
    edit: "编辑",
    collapse: "收起",
    more: "更多",
    up: "上移",
    down: "下移",
    remove: "移除",
    removeProviderConfirm: (providerName) => `移除提供方 ${providerName}？`,
    removeManagedSecrets: "同时删除此账号由 QuotaBarWin 管理的本地密钥文件",
    removeManagedSecretsHint: "不会删除外部文件、环境变量或手工命名的 secret 文件。",
    setEnvVar: (name) => `设置 ${name}`,
    name: "名称",
    authToken: "认证令牌",
    authTokenPlaceholder:
      "粘贴令牌、${secret:CODEX_ACCESS_TOKEN}、${env:CODEX_ACCESS_TOKEN} 或 ${file:C:\\Secrets\\codex-token.txt}",
    accountId: "ChatGPT 账号 ID",
    optional: "可选",
    proxyUrl: "代理 URL",
    providerProxyPlaceholder: "可选，例如 http://127.0.0.1:7890 或 socks5h://127.0.0.1:7890",
    timeout: "超时（秒）",
    timeoutError: "超时时间必须大于 0。",
    providerParameters: "支持的参数",
    providerParametersHint: "提示来自此 Provider 的 manifest。",
    parameterRequired: "必填",
    parameterKind: (kind) => `类型：${kind}`,
    parameterDefault: (value) => `默认：${value}`,
    parameterPlaceholder: (value) => `占位：${value}`,
    parameterOptions: (value) => `选项：${value}`,
    remoteEnvVars: "环境变量",
    remoteEnvVarsPlaceholder:
      "每行一个 KEY=value\nKIMI_API_KEY=${secret:KIMI_WORK_API_KEY}\nDEEPSEEK_API_KEY=${secret:DEEPSEEK_API_KEY}\nDEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY=200\nDEEPSEEK_BALANCE_WARNING_CNY=20",
    remoteEnvVarsHint:
      "多账号时，左边保持脚本需要的环境变量名，右边换成这个账号的 secret 名；例如 KIMI_API_KEY=${secret:KIMI_WORK_API_KEY} 会读取 secrets/KIMI_WORK_API_KEY.txt。",
    windowLabelOverrides: "窗口标签覆盖",
    windowLabelOverridesPlaceholder: "window-id=显示名称\n5h=5h\nweekly=Weekly limit",
    displayedWindows: "显示的窗口",
    showInTray: "在托盘小窗中显示",
    displayedWindowsPlaceholder: "留空显示全部\n5h\nweekly\nWeekly limit",
    windowDisplay: "窗口显示",
    windowDisplayNoSnapshot: "暂无最近快照窗口。",
    windowDisplayNoRows: "可在下方添加窗口 ID 生成可编辑行。",
    show: "显示",
    windowId: "窗口 ID",
    defaultLabel: "默认名称",
    customLabel: "自定义名称",
    customLabelFor: (windowId) => `${windowId} 的自定义名称`,
    order: "顺序",
    showWindow: (windowId) => `显示 ${windowId}`,
    showAllWindows: "显示全部",
    resetWindowNames: "重置名称",
    resetWindowOrder: "默认顺序",
    moveWindowUp: (windowId) => `上移 ${windowId}`,
    moveWindowDown: (windowId) => `下移 ${windowId}`,
    advancedWindowText: "高级窗口文本",
    configurationStorage: "配置存储",
    portableMode: "便携模式",
    appDataMode: "AppData 模式",
    configFile: "配置文件",
    appData: "AppData",
    portable: "便携",
    portableMarker: "标记文件",
    loadingConfigPath: "正在加载配置路径...",
    loading: "加载中...",
    portableModeHint:
      "便携模式会把配置存放在应用可执行文件旁，并使用 quotabarwin.portable 作为标记文件。",
    openFolder: "打开配置存储文件夹",
    resetConfig: "重置配置",
    resetConfigConfirm: "将 QuotaBarWin 配置重置为默认值？会先创建备份。",
    noChanges: "无更改",
    saving: "保存中",
    saved: "已保存",
    saveFailed: "保存失败",
    unsavedChanges: "未保存的更改",
    unsavedChangesTitle: "未保存的更改",
    unsavedChangesPrompt:
      "继续前要保存更改吗？你也可以放弃更改，或留在当前页面。",
    saveAndContinue: "保存并继续",
    discardChanges: "放弃更改",
    resetChanges: "重置更改",
    save: "保存"
  },
  appUpdate: {
    title: "应用更新",
    currentVersion: (version) => `当前版本：${version}`,
    check: "检查应用更新",
    checking: "正在检查...",
    upToDate: "QuotaBarWin 已是最新版本。",
    available: (version) => `发现 QuotaBarWin ${version}。`,
    downloadAndRestart: "下载并重启更新",
    downloading: "正在下载更新...",
    notes: "查看发布说明",
    unavailable: "此构建尚未配置应用更新。",
    failedToCheck: "检查应用更新失败",
    failedToDownload: "下载应用更新失败",
    failedToOpenNotes: "无法打开发行说明",
    autoCheck: "自动检查应用更新",
    autoCheckHint: "每天 08:00 后首次打开主窗口或托盘小窗时检查；不会自动下载或重启。",
    lastChecked: (time) => `上次自动检查：${time}`,
    checkFailed: "最近一次自动检查失败，可手动重试。",
    openUpdate: "前往更新",
    later: "稍后"
  },
  networkProxy: {
    label: "网络代理",
    noProxy: "不使用代理",
    systemProxy: "系统代理",
    httpProxy: "HTTP 代理",
    socks5Proxy: "SOCKS5 代理",
    proxyUrl: "代理 URL",
    proxyUrlPlaceholder: "http://host:port 或 socks5://host:port",
    testProxy: "检测代理",
    testingProxy: "正在检测代理...",
    testProxyHint: "使用当前代理设置请求 GitHub 首页，不保存响应内容。",
    customTestUrl: "使用自定义检测地址",
    testUrl: "检测地址",
    testUrlPlaceholder: "https://example.com/",
    testProxySuccess: (statusCode, elapsedMs) =>
      `代理可用（${statusCode ?? "无状态码"}，${elapsedMs} ms）。`,
    testProxyNoProxy: "请先配置系统代理、HTTP 代理或 SOCKS5 代理。",
    testProxyInvalidTarget: "检测地址必须是有效的 HTTPS 地址。",
    testProxyInvalidProxy: "代理地址无效。",
    testProxyRequestFailed: "代理无法访问检测地址。",
    testProxyHttpStatus: (statusCode) =>
      `检测地址返回 HTTP ${statusCode ?? "异常状态"}。`,
    testProxyUnavailable: "代理检测仅可在桌面应用中使用。"
  },
  localApi: {
    title: "本地集成 API",
    enabled: "启用本地集成 API",
    enabledHint: "桌面应用运行期间，会对外提供当前已标准化的额度快照。",
    listenTarget: "监听位置",
    loopback: "仅此电脑 (127.0.0.1)",
    selectedNetworkInterfaces: "指定网卡",
    allNetworkInterfaces: "所有活动网卡",
    selectedInterfacesHint: "请选择一个或多个具有 IPv4 或 IPv6 地址的活动网卡。",
    includeLoopback: "同时监听本机",
    includeLoopbackHint: "127.0.0.1 无需 Token 即可访问。",
    networkInterface: (name, address) => `${name}（${address}）`,
    noNetworkInterfaces: "没有可用的活动 IPv4 或 IPv6 网卡。",
    noInterfacesSelected: "请至少选择一个网卡。",
    port: "端口",
    portError: "端口必须介于 1 和 65535 之间。",
    endpoint: "接口地址",
    running: "运行中",
    stopped: "已停止",
    unavailable: "不可用",
    authentication: "鉴权",
    authenticationRequired: "通过网络访问时必须提供 Bearer Token。",
    localAuthenticationNotRequired: "仅本机回环访问无需 Token。",
    token: "访问 Token",
    tokenPlaceholder: "至少输入 32 个非空白字符",
    copyToken: "复制 Token",
    replaceToken: "替换 Token",
    saveToken: "保存 Token",
    generateToken: "生成新 Token",
    tokenHint: "请求头使用 Authorization: Bearer <token>。Token 与主配置分开保存，且不会写入日志。",
    tokenConfigured: "已保存访问 Token（已掩码显示）。",
    tokenRequiredBeforeSave: "请先保存访问 Token，再保存网络监听设置。",
    networkWarning: "网络访问使用明文 HTTP。请只在受信任网络使用强 Token，且不要将端口暴露到公网。",
    tokenSaved: "访问 Token 已保存。",
    tokenSaveFailed: "保存访问 Token 失败。",
    tokenCopied: "访问 Token 已复制。",
    tokenCopyFailed: "无法复制访问 Token。",
    interfaceLoadFailed: "无法读取活动网卡。",
  },
  remoteProviders: {
    title: "远程安装源",
    addTitle: "添加提供方",
    manageSourcesTitle: "提供方来源",
    recommendedProviders: "推荐",
    customInstall: "自定义安装",
    securityNoticeLead: "安全提示：只安装使用可信任的 Provider。",
    securityNoticeBody:
      "Provider 脚本可以直接读取你配置的各类 AI 鉴权信息，并发起网络通讯。",
    sourceSummary: "当前来源",
    sourceSettings: "来源设置",
    projectMaintainedSource: "项目维护来源",
    customSource: "自定义来源",
    sourceName: "来源名称",
    sourceUrl: "Registry URL 或本地路径",
    sourceEnabled: "已启用",
    addSource: "添加来源",
    removeSource: "移除来源",
    migrateSource: "迁移已安装 Provider",
    migratingSource: "正在迁移...",
    migrateSourceConfirm: (sourceName) =>
      `将匹配的已安装 Provider 迁移到“${sourceName}”？会保留账号设置，但将从此来源替换 manifest 与脚本。`,
    migrateSourceResult: (migrated, skipped, failed) =>
      `迁移完成：已迁移 ${migrated} 个，跳过 ${skipped} 个，失败 ${failed} 个。`,
    failedToMigrateSource: "迁移已安装 Provider 失败",
    enabledSourcesCount: (count) => `已启用 ${count} 个来源`,
    sourceLoadError: (sourceName, message) => `${sourceName}：${message}`,
    sourceConflict: (providerId, sourceNames) =>
      `Provider id '${providerId}' 同时出现在多个来源：${sourceNames}`,
    backToSettings: "返回设置",
    backToAddProvider: "返回添加提供方",
    manageSources: "管理来源",
    refreshCatalog: "刷新列表",
    installProvider: "安装",
    addAccount: "添加账号",
    addingAccount: "正在添加账号",
    installed: "已安装",
    installedAccounts: (count) => `${count} 个账号`,
    installFromManifest: "安装提供方",
    manifestInstallUrl: "Provider manifest URL 或本地路径",
    manifestUrlPlaceholder: "https://.../provider.json 或 D:\\Providers\\provider.json",
    manifestChecksum: "Manifest checksum（可选）",
    manifestChecksumPlaceholder: "sha256:...",
    noCatalogProviders: "此来源没有找到提供方",
    catalogLoadFailed: "加载提供方列表失败",
    providerInstalled: "提供方已安装",
    accountAdded: "账号已添加",
    installProviderFailed: "安装提供方失败",
    installedCount: (count) => `已安装 ${count} 个`,
    openGuide: "打开指南",
    registryUrl: "注册表 URL",
    registryUrlPlaceholder: "https://...、file:///... 或 registry.json 本地路径",
    providerProxyUrl: "安装源代理 URL（可选）",
    providerProxyPlaceholder: "http://host:port 或 socks5://host:port",
    autoUpdateWhenAvailable: "有更新时自动更新",
    loading: "加载中...",
    installRegistry: "安装注册表",
    installedResult: (count) => `已安装 ${count} 个`,
    skippedResult: (count) => `已跳过 ${count} 个`,
    failedResult: (count) => `失败 ${count} 个`,
    noProvidersInstalledFromRegistry: "没有从注册表安装提供方",
    failedToInstallRegistry: "安装注册表失败",
    providerRemoved: "提供方已移除",
    failedToRemoveProvider: "移除提供方失败",
    updateAvailable: "有可用更新",
    providerRefreshed: "提供方已刷新",
    failedToRefreshProvider: "刷新提供方失败",
    updatesAvailable: (count) => `${count} 个更新可用`,
    allProvidersUpToDate: "所有提供方均为最新",
    failedToCheckUpdates: "检查更新失败",
    updateApplied: "更新已应用",
    failedToApplyUpdate: "应用更新失败",
    noRemoteProvidersInstalled: "未安装远程提供方",
    autoUpdate: "自动更新",
    details: "详情",
    collapse: "收起",
    refresh: "刷新",
    checkUpdates: "检查更新",
    checkingUpdates: "正在检查...",
    applyUpdate: "应用更新",
    applyAllUpdates: (count) => `全部应用更新（${count}）`,
    applyingUpdates: "正在应用更新...",
    updatesApplied: (count) => `已应用 ${count} 个更新`,
    updateSomeFailed: (updated, failed) => `已更新 ${updated} 个，失败 ${failed} 个`,
    remove: "移除",
    version: "版本",
    unknownVersion: "未知版本",
    installedAt: "安装时间",
    updatedAt: "更新时间",
    lastCheckedAt: "上次检查",
    runtime: "运行时",
    manifestUrl: "Manifest"
  },
  providerSetup: {
    title: (name) => `配置 ${name}`,
    accountName: "账号名称",
    required: "必填",
    configured: "已配置",
    notConfigured: "未配置",
    changeSecret: "更换",
    clearSecret: "清除",
    managedSecretHint: (providerId, parameterName) =>
      `填写并保存后，QuotaBarWin 会为当前 Provider 实例自动创建独立文件：<配置目录>\\secrets\\providers\\${providerId}\\${parameterName}.txt。主配置只保存引用，原始值不会在这里显示。`,
    existingCredentialHint: "不更换时请留空；只有要替换当前凭据时，才在下面输入新值。",
    enterSecret: "输入 API Key",
    replaceSecret: "输入新凭据以替换当前值（留空保持不变）",
    advancedSettings: "高级设置",
    saveAndTest: "保存并测试",
    autoDetectAndFinish: "自动检测并完成",
    autoDetectHint: "此 Provider 会优先使用本机已有的登录信息；无需填写可选字段，也可以直接测试。",
    saveWithoutTesting: "保存但暂不测试",
    testing: "正在测试配置...",
    testSucceeded: "配置可用，Provider 已自动启用。",
    testFailed: "配置测试失败",
    complete: "完成",
    back: "返回",
    statePending: "待配置",
    stateUnverified: "未验证",
    stateReady: "可用",
    stateNeedsAttention: "需要处理",
    setup: "配置",
    completeSetup: "完成配置",
    testConfiguration: "测试配置",
    repairConfiguration: "修复配置",
    providerPreview: (count) => `连接成功${count ? ` · ${count} 个额度窗口` : ""}`,
    missingRequired: (name) => `请填写 ${name}。`,
    invalidNumber: (name) => `${name} 必须是数字。`,
    credentialSource: (source) => `凭据来源：${source}`,
    credentialSourceType: (source) => {
      const labels: Record<string, string> = {
        managedLocalFile: "应用托管本地文件",
        secretFile: "本地 secret 文件",
        environment: "环境变量",
        externalFile: "外部文件",
        literal: "配置内明文",
        default: "Provider 默认值",
        advancedExpression: "高级配置",
        missing: "未配置",
      };
      return labels[source] ?? labels.missing;
    },
    errorCategory: (category) => {
      const labels: Record<string, string> = {
        missingCredential: "缺少账号凭据",
        authentication: "凭据无效或已过期",
        network: "无法连接服务",
        proxy: "代理连接失败",
        timeout: "请求超时",
        runtime: "Provider 运行异常",
        permission: "Provider 权限不足",
        providerOutput: "Provider 返回格式异常",
        unknown: "暂时无法确定原因",
      };
      return labels[category] ?? labels.unknown;
    },
    saveFailed: "无法保存 Provider 配置",
    testFailedKeepSaved: "配置已安全保存，但 Provider 仍处于停用状态。修改后请重新测试。",
    noUsableProvidersTitle: "还没有可用的额度来源",
    noUsableProvidersBody: "添加一个 Provider 并配置账号后，即可在这里查看额度。",
    addProvider: "添加 Provider",
    openGuide: "查看使用说明"
  },
  format: {
    unknown: "未知",
    remainingUnknown: "剩余未知",
    usageUnknown: "用量未知",
    displayValue: (percent, displayMode) => `${percent}% ${displayMode === "remaining" ? "剩余" : "已用"}`,
    resets: (time) => `${time} 重置`
  }
};

export const catalogs: Record<ResolvedLanguage, I18nCatalog> = {
  en,
  "zh-CN": zhCN
};

export function resolveLanguage(
  choice: LanguageChoice,
  languages: readonly string[] = getNavigatorLanguages()
): ResolvedLanguage {
  if (choice !== "system") {
    return choice;
  }

  for (const language of languages) {
    const resolved = resolveLanguageTag(language);
    if (resolved) {
      return resolved;
    }
  }

  return "zh-CN";
}

export function createI18n(choice: LanguageChoice = "zh-CN", languages?: readonly string[]) {
  const language = resolveLanguage(choice, languages);

  return {
    choice,
    language,
    t: catalogs[language]
  };
}

function getNavigatorLanguages(): readonly string[] {
  if (typeof navigator === "undefined") {
    return [];
  }

  if (navigator.languages?.length) {
    return navigator.languages;
  }

  return navigator.language ? [navigator.language] : [];
}

function resolveLanguageTag(language: string): ResolvedLanguage | null {
  const normalized = language.toLowerCase();

  if (normalized === "zh-cn" || normalized.startsWith("zh-hans") || normalized.startsWith("zh")) {
    return "zh-CN";
  }

  if (normalized === "en" || normalized.startsWith("en-")) {
    return "en";
  }

  return null;
}
