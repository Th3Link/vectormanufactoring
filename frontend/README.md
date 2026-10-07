# Curvyo frontend

TypeScript + React + Tailwind CSS + shadcn/ui, loaded into the Tauri 2 host
(`curvyo-app`) as its webview content (ADR 0001).

This slice (`specs/0001-project-file-foundation/`) is intentionally thin: an
empty canvas, a status bar, and the `AlertDialog` used to report a failed
project open. No drawing tools, no toolbar, no side panels — none exist yet.

## Development

```sh
npm install
npm run build   # tsc -b && vite build -> dist/, which curvyo-app embeds
```

Running inside the Tauri shell is `cargo run -p curvyo-app` from the
workspace root, which rebuilds this frontend first
(`tauri.conf.json`'s `beforeBuildCommand`/`beforeDevCommand`).
