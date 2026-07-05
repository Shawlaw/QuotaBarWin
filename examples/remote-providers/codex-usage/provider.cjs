const https = require("node:https");
const fs = require("node:fs");
const net = require("node:net");
const os = require("node:os");
const path = require("node:path");
const tls = require("node:tls");

const CODEX_USAGE_URL = "https://chatgpt.com/backend-api/wham/usage";
const CODEX_USAGE_TIMEOUT_MS = 30_000;

async function main() {
  const auth = readCodexAuth();
  const token = process.env.CODEX_ACCESS_TOKEN || auth.accessToken;
  if (!token) {
    throw new Error(
      "Codex access token is required; set CODEX_ACCESS_TOKEN or sign in with Codex so ~/.codex/auth.json exists"
    );
  }
  const accountId = process.env.CODEX_ACCOUNT_ID || auth.accountId || "";

  // Prefer explicit env vars for overrides, then fall back to Codex's own
  // local auth file so users do not have to duplicate short-lived tokens.
  const raw = await fetchCodexUsage(token.trim(), accountId.trim());

  // Raw Codex usage shape used here:
  // {
  //   plan_type, credits,
  //   rate_limit: {
  //     primary_window: { used_percent, reset_at, reset_after_seconds },
  //     secondary_window: { used_percent, reset_at, reset_after_seconds }
  //   }
  // }
  // The two rate-limit windows become provider-snapshot-v1 windows; account and
  // plan details stay in metadata.
  const windows = [];
  const primary = raw?.rate_limit?.primary_window;
  if (primary && typeof primary.used_percent === "number") {
    windows.push(windowFromUsage("5h", "5h", primary));
  }

  const secondary = raw?.rate_limit?.secondary_window;
  if (secondary && typeof secondary.used_percent === "number") {
    windows.push(windowFromUsage("weekly", "Weekly limit", secondary));
  }

  console.log(
    JSON.stringify({
      status: windows.length > 0 ? "ok" : "warning",
      updatedAt: new Date().toISOString(),
      windows,
      metadata: {
        planType: raw.plan_type ?? null,
        credits: raw.credits ?? null
      }
    })
  );
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}

function readCodexAuth() {
  const authPath = process.env.CODEX_AUTH_FILE || path.join(os.homedir(), ".codex", "auth.json");
  try {
    const auth = JSON.parse(fs.readFileSync(authPath, "utf8"));
    return {
      accessToken: stringOrEmpty(auth?.tokens?.access_token),
      accountId: stringOrEmpty(auth?.tokens?.account_id)
    };
  } catch {
    return {
      accessToken: "",
      accountId: ""
    };
  }
}

function fetchCodexUsage(token, accountId) {
  const proxyUrl = firstNonEmpty(
    process.env.QBWIN_PROXY_URL,
    process.env.HTTPS_PROXY,
    process.env.HTTP_PROXY,
    process.env.ALL_PROXY
  );
  return new Promise((resolve, reject) => {
    const headers = {
      Authorization: `Bearer ${token}`,
      "User-Agent": "QuotaBarWin/0.0"
    };
    if (accountId) {
      headers["ChatGPT-Account-Id"] = accountId;
    }

    if (proxyUrl) {
      fetchJsonViaProxy(CODEX_USAGE_URL, headers, proxyUrl, CODEX_USAGE_TIMEOUT_MS).then(resolve, reject);
      return;
    }

    const request = https.get(CODEX_USAGE_URL, { headers }, (response) => {
      let body = "";
      response.on("data", (chunk) => {
        body += chunk;
      });
      response.on("end", () => {
        if (response.statusCode !== 200) {
          reject(new Error(`Codex usage API returned ${response.statusCode}: ${body.slice(0, 160)}`));
          return;
        }
        try {
          resolve(JSON.parse(body));
        } catch (error) {
          reject(new Error(`Failed to parse Codex usage response: ${error instanceof Error ? error.message : String(error)}`));
        }
      });
    });

    request.on("error", (error) => reject(error));
    request.setTimeout(CODEX_USAGE_TIMEOUT_MS, () => {
      request.destroy();
      reject(new Error("Codex usage request timed out"));
    });
  });
}

async function fetchJsonViaProxy(targetUrl, headers, proxyUrl, timeoutMs) {
  const target = new URL(targetUrl);
  const proxy = new URL(proxyUrl);
  let tunnel;

  if (proxy.protocol === "socks5:" || proxy.protocol === "socks5h:") {
    tunnel = await connectSocks5(proxy, target, timeoutMs);
  } else if (proxy.protocol === "http:" || proxy.protocol === "https:") {
    tunnel = await connectHttpProxy(proxy, target, timeoutMs);
  } else {
    throw new Error(`Unsupported Codex proxy protocol: ${proxy.protocol}`);
  }

  const socket = tls.connect({
    socket: tunnel,
    servername: target.hostname
  });

  try {
    const response = await requestOverTlsSocket(socket, target, headers, timeoutMs);
    if (response.statusCode !== 200) {
      throw new Error(`Codex usage API returned ${response.statusCode}: ${response.body.slice(0, 160)}`);
    }
    return JSON.parse(response.body);
  } catch (error) {
    socket.destroy();
    throw error;
  }
}

function connectSocks5(proxy, target, timeoutMs) {
  return new Promise((resolve, reject) => {
    const socket = net.connect({
      host: proxy.hostname,
      port: Number(proxy.port || 1080)
    });
    socket.setTimeout(timeoutMs);
    socket.once("error", reject);
    socket.once("timeout", () => reject(new Error("Codex SOCKS proxy connection timed out")));
    socket.once("connect", async () => {
      const reader = createSocketReader(socket);
      try {
        socket.write(Buffer.from([0x05, 0x01, 0x00]));
        const method = await reader.readExact(2);
        if (method[0] !== 0x05 || method[1] !== 0x00) {
          throw new Error("Codex SOCKS proxy does not allow no-auth connections");
        }

        socket.write(buildSocks5ConnectRequest(target));
        const head = await reader.readExact(4);
        if (head[0] !== 0x05 || head[1] !== 0x00) {
          throw new Error(`Codex SOCKS proxy CONNECT failed with code ${head[1]}`);
        }
        if (head[3] === 0x01) {
          await reader.readExact(6);
        } else if (head[3] === 0x03) {
          const length = await reader.readExact(1);
          await reader.readExact(length[0] + 2);
        } else if (head[3] === 0x04) {
          await reader.readExact(18);
        } else {
          throw new Error("Codex SOCKS proxy returned an unsupported address type");
        }

        reader.stop();
        socket.removeAllListeners("error");
        socket.removeAllListeners("timeout");
        socket.setTimeout(0);
        resolve(socket);
      } catch (error) {
        reader.stop();
        socket.destroy();
        reject(error);
      }
    });
  });
}

function buildSocks5ConnectRequest(target) {
  const host = Buffer.from(target.hostname, "utf8");
  if (host.length > 255) {
    throw new Error("Codex target hostname is too long for SOCKS5");
  }
  const port = Buffer.alloc(2);
  port.writeUInt16BE(Number(target.port || 443));
  return Buffer.concat([
    Buffer.from([0x05, 0x01, 0x00, 0x03, host.length]),
    host,
    port
  ]);
}

function connectHttpProxy(proxy, target, timeoutMs) {
  return new Promise((resolve, reject) => {
    const connectOptions = {
      host: proxy.hostname,
      port: Number(proxy.port || (proxy.protocol === "https:" ? 443 : 80))
    };
    const socket = proxy.protocol === "https:" ? tls.connect(connectOptions) : net.connect(connectOptions);
    socket.setTimeout(timeoutMs);
    socket.once("error", reject);
    socket.once("timeout", () => reject(new Error("Codex HTTP proxy connection timed out")));
    socket.once("connect", async () => {
      const reader = createSocketReader(socket);
      try {
        const targetHost = `${target.hostname}:${target.port || 443}`;
        socket.write(
          [
            `CONNECT ${targetHost} HTTP/1.1`,
            `Host: ${targetHost}`,
            "Proxy-Connection: Keep-Alive",
            "",
            ""
          ].join("\r\n")
        );
        const response = await reader.readUntil("\r\n\r\n");
        if (!/^HTTP\/1\.[01] 2\d\d/i.test(response.toString("utf8"))) {
          throw new Error("Codex HTTP proxy CONNECT failed");
        }
        reader.stop();
        socket.removeAllListeners("error");
        socket.removeAllListeners("timeout");
        socket.setTimeout(0);
        resolve(socket);
      } catch (error) {
        reader.stop();
        socket.destroy();
        reject(error);
      }
    });
  });
}

function requestOverTlsSocket(socket, target, headers, timeoutMs) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    socket.setTimeout(timeoutMs);
    socket.once("error", reject);
    socket.once("timeout", () => reject(new Error("Codex usage request timed out")));
    socket.once("secureConnect", () => {
      const requestHeaders = [
        `GET ${target.pathname}${target.search} HTTP/1.1`,
        `Host: ${target.host}`,
        ...Object.entries(headers).map(([name, value]) => `${name}: ${value}`),
        "Connection: close",
        "",
        ""
      ];
      socket.write(requestHeaders.join("\r\n"));
    });
    socket.on("data", (chunk) => chunks.push(chunk));
    socket.once("end", () => {
      try {
        resolve(parseHttpResponse(Buffer.concat(chunks)));
      } catch (error) {
        reject(error);
      }
    });
  });
}

function parseHttpResponse(raw) {
  const headerEnd = raw.indexOf("\r\n\r\n");
  if (headerEnd < 0) {
    throw new Error("Codex usage API returned an invalid HTTP response");
  }
  const headerText = raw.subarray(0, headerEnd).toString("utf8");
  const bodyBuffer = raw.subarray(headerEnd + 4);
  const headerLines = headerText.split("\r\n");
  const statusCode = Number(headerLines[0].split(/\s+/)[1]);
  const headers = Object.fromEntries(
    headerLines.slice(1).map((line) => {
      const index = line.indexOf(":");
      return index < 0 ? [line.toLowerCase(), ""] : [line.slice(0, index).toLowerCase(), line.slice(index + 1).trim()];
    })
  );
  const body =
    headers["transfer-encoding"]?.toLowerCase() === "chunked"
      ? decodeChunkedBody(bodyBuffer).toString("utf8")
      : bodyBuffer.toString("utf8");
  return { statusCode, body };
}

function decodeChunkedBody(buffer) {
  const chunks = [];
  let offset = 0;
  while (offset < buffer.length) {
    const lineEnd = buffer.indexOf("\r\n", offset);
    if (lineEnd < 0) {
      break;
    }
    const size = Number.parseInt(buffer.subarray(offset, lineEnd).toString("utf8"), 16);
    if (!Number.isFinite(size) || size < 0) {
      throw new Error("Codex usage API returned invalid chunked data");
    }
    if (size === 0) {
      break;
    }
    const start = lineEnd + 2;
    chunks.push(buffer.subarray(start, start + size));
    offset = start + size + 2;
  }
  return Buffer.concat(chunks);
}

function createSocketReader(socket) {
  let buffer = Buffer.alloc(0);
  const waiters = [];

  function onData(chunk) {
    buffer = Buffer.concat([buffer, chunk]);
    flushWaiters();
  }

  function flushWaiters() {
    for (let index = 0; index < waiters.length; index += 1) {
      const waiter = waiters[index];
      const result = waiter.tryRead();
      if (!result) {
        continue;
      }
      waiters.splice(index, 1);
      index -= 1;
      waiter.resolve(result);
    }
  }

  socket.on("data", onData);

  return {
    readExact(length) {
      return new Promise((resolve) => {
        const waiter = {
          tryRead() {
            if (buffer.length < length) {
              return null;
            }
            const result = buffer.subarray(0, length);
            buffer = buffer.subarray(length);
            return result;
          },
          resolve
        };
        const immediate = waiter.tryRead();
        if (immediate) {
          resolve(immediate);
        } else {
          waiters.push(waiter);
        }
      });
    },
    readUntil(marker) {
      const markerBuffer = Buffer.from(marker);
      return new Promise((resolve) => {
        const waiter = {
          tryRead() {
            const markerStart = buffer.indexOf(markerBuffer);
            if (markerStart < 0) {
              return null;
            }
            const markerEnd = markerStart + markerBuffer.length;
            const result = buffer.subarray(0, markerEnd);
            buffer = buffer.subarray(markerEnd);
            return result;
          },
          resolve
        };
        const immediate = waiter.tryRead();
        if (immediate) {
          resolve(immediate);
        } else {
          waiters.push(waiter);
        }
      });
    },
    stop() {
      socket.off("data", onData);
      if (buffer.length > 0) {
        socket.unshift(buffer);
      }
      buffer = Buffer.alloc(0);
      waiters.length = 0;
    }
  };
}

function windowFromUsage(id, label, usage) {
  const usedPercent = numberOrNull(usage.used_percent);
  // Codex reports percentages rather than absolute counters, so used/limit stay
  // null and the snapshot carries usedPercent/remainingPercent for the UI.
  return {
    id,
    label,
    used: null,
    limit: null,
    unit: "percent",
    usedPercent,
    remainingPercent: usedPercent === null ? null : Math.max(0, 100 - usedPercent),
    resetAt: resetIsoFromWindow(usage),
    resetText: null,
    confidence: "exact"
  };
}

function resetIsoFromWindow(usage) {
  if (typeof usage.reset_at === "number" && usage.reset_at > 0) {
    return new Date(usage.reset_at * 1000).toISOString();
  }
  if (typeof usage.reset_after_seconds === "number" && usage.reset_after_seconds > 0) {
    return new Date(Date.now() + usage.reset_after_seconds * 1000).toISOString();
  }
  return null;
}

function numberOrNull(value) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function stringOrEmpty(value) {
  return typeof value === "string" ? value.trim() : "";
}

function firstNonEmpty(...values) {
  for (const value of values) {
    if (typeof value === "string" && value.trim()) {
      return value.trim();
    }
  }
  return "";
}

module.exports = {
  firstNonEmpty,
  readCodexAuth
};
