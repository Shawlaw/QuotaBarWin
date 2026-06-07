console.log(
  JSON.stringify({
    id: "fake-command-provider",
    name: "Fake Command Provider",
    status: "ok",
    source: "command",
    updatedAt: "2026-06-08T10:00:00+08:00",
    windows: [
      {
        id: "weekly",
        label: "Weekly",
        used: 123,
        limit: 1000,
        unit: "requests",
        usedPercent: 12.3,
        remainingPercent: 87.7,
        resetAt: null,
        confidence: "exact"
      }
    ]
  })
);
