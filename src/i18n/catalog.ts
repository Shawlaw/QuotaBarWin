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
    refresh: string;
    refreshingShort: string;
    close: string;
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
    language: string;
    languageSystem: string;
    languageEnglish: string;
    languageChinese: string;
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
    setEnvVar: (name: string) => string;
    name: string;
    authToken: string;
    authTokenPlaceholder: string;
    accountId: string;
    optional: string;
    proxyUrl: string;
    providerProxyPlaceholder: string;
    timeout: string;
    remoteEnvVars: string;
    remoteEnvVarsPlaceholder: string;
    windowLabelOverrides: string;
    windowLabelOverridesPlaceholder: string;
    displayedWindows: string;
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
    resetChanges: string;
    save: string;
  };
  networkProxy: {
    label: string;
    noProxy: string;
    systemProxy: string;
    httpProxy: string;
    socks5Proxy: string;
    proxyUrl: string;
    proxyUrlPlaceholder: string;
  };
  remoteProviders: {
    title: string;
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
    applyUpdate: string;
    remove: string;
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
    refresh: "Refresh",
    refreshingShort: "...",
    close: "Close",
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
    language: "Language",
    languageSystem: "System",
    languageEnglish: "English",
    languageChinese: "Simplified Chinese",
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
    setEnvVar: (name) => `Set ${name}`,
    name: "Name",
    authToken: "Auth token",
    authTokenPlaceholder:
      "Paste a token, ${secret:CODEX_ACCESS_TOKEN}, ${env:CODEX_ACCESS_TOKEN}, or ${file:C:\\Secrets\\codex-token.txt}",
    accountId: "ChatGPT account id",
    optional: "Optional",
    proxyUrl: "Proxy URL",
    providerProxyPlaceholder: "Optional, e.g. http://127.0.0.1:7890 or socks5h://127.0.0.1:7890",
    timeout: "Timeout",
    remoteEnvVars: "Environment variables",
    remoteEnvVarsPlaceholder:
      "KEY=value, one per line\nDEEPSEEK_API_KEY=${secret:DEEPSEEK_API_KEY}\nDEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY=200\nDEEPSEEK_BALANCE_WARNING_CNY=20",
    windowLabelOverrides: "Window label overrides",
    windowLabelOverridesPlaceholder: "window-id=Display name\n5h=5h\nweekly=Weekly limit",
    displayedWindows: "Displayed windows",
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
    resetChanges: "Reset changes",
    save: "Save"
  },
  networkProxy: {
    label: "Network proxy",
    noProxy: "No proxy",
    systemProxy: "System proxy",
    httpProxy: "HTTP proxy",
    socks5Proxy: "SOCKS5 proxy",
    proxyUrl: "Proxy URL",
    proxyUrlPlaceholder: "http://host:port or socks5://host:port"
  },
  remoteProviders: {
    title: "Remote Sources",
    installedCount: (count) => `${count} installed`,
    openGuide: "Open Guide",
    registryUrl: "Registry URL",
    registryUrlPlaceholder: "https://... or file:///... or local path to registry.json",
    providerProxyUrl: "Provider proxy URL (optional)",
    providerProxyPlaceholder: "http://proxy:8080",
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
    applyUpdate: "Apply Update",
    remove: "Remove"
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
    refresh: "刷新",
    refreshingShort: "...",
    close: "关闭",
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
    language: "语言",
    languageSystem: "跟随系统",
    languageEnglish: "English",
    languageChinese: "简体中文",
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
    setEnvVar: (name) => `设置 ${name}`,
    name: "名称",
    authToken: "认证令牌",
    authTokenPlaceholder:
      "粘贴令牌、${secret:CODEX_ACCESS_TOKEN}、${env:CODEX_ACCESS_TOKEN} 或 ${file:C:\\Secrets\\codex-token.txt}",
    accountId: "ChatGPT 账号 ID",
    optional: "可选",
    proxyUrl: "代理 URL",
    providerProxyPlaceholder: "可选，例如 http://127.0.0.1:7890 或 socks5h://127.0.0.1:7890",
    timeout: "超时",
    remoteEnvVars: "环境变量",
    remoteEnvVarsPlaceholder:
      "每行一个 KEY=value\nDEEPSEEK_API_KEY=${secret:DEEPSEEK_API_KEY}\nDEEPSEEK_BALANCE_REFERENCE_TOTAL_CNY=200\nDEEPSEEK_BALANCE_WARNING_CNY=20",
    windowLabelOverrides: "窗口标签覆盖",
    windowLabelOverridesPlaceholder: "window-id=显示名称\n5h=5h\nweekly=Weekly limit",
    displayedWindows: "显示的窗口",
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
    resetChanges: "重置更改",
    save: "保存"
  },
  networkProxy: {
    label: "网络代理",
    noProxy: "不使用代理",
    systemProxy: "系统代理",
    httpProxy: "HTTP 代理",
    socks5Proxy: "SOCKS5 代理",
    proxyUrl: "代理 URL",
    proxyUrlPlaceholder: "http://host:port 或 socks5://host:port"
  },
  remoteProviders: {
    title: "远程安装源",
    installedCount: (count) => `已安装 ${count} 个`,
    openGuide: "打开指南",
    registryUrl: "注册表 URL",
    registryUrlPlaceholder: "https://...、file:///... 或 registry.json 本地路径",
    providerProxyUrl: "提供方代理 URL（可选）",
    providerProxyPlaceholder: "http://proxy:8080",
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
    applyUpdate: "应用更新",
    remove: "移除"
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

  return "en";
}

export function createI18n(choice: LanguageChoice = "en", languages?: readonly string[]) {
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
