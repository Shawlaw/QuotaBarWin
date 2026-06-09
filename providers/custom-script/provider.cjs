console.log(
  JSON.stringify({
    status: "ok",
    updatedAt: new Date().toISOString(),
    windows: [
      {
        id: "weekly",
        label: "Weekly",
        used: 12,
        limit: 100,
        unit: "requests",
        confidence: "exact"
      }
    ],
    metadata: {
      source: "custom-script"
    }
  })
);
