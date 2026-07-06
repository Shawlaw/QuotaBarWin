# Icon Pipeline

`source.png` is the source image for the app icon. Keep it square, prefer
1024x1024 or larger, and leave transparent-safe padding so the mark still reads
at 16x16.

QuotaBarWin is packaged as a desktop app, so the icon pipeline prunes generated
Android and iOS resources after each run.

Regenerate all Tauri icon outputs from the default source:

```powershell
npm run icons:generate
```

Check that the committed outputs still match the source:

```powershell
npm run icons:check
```

To test another source image without editing this file first:

```powershell
npm run icons:generate -- path\to\source.png
```
