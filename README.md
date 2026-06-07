# QuotaBarWin

WindowsFirst AI usage/quota monitor.

Architecture principle:

Provider is the only public abstraction that supplies quota data to the app.
A provider may internally execute commands, call curl/CLI/scripts, parse JSON,
parse text, or use native logic, but the UI and app runtime only depend on
normalized ProviderSnapshot / AppSnapshot data.

Start from V0, then implement versions incrementally according to docs/specs.