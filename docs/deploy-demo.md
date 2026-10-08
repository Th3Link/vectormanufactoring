# Browser demo on GitHub Pages

A public build of the editor that runs in a plain browser, so anyone can see
the current progress without installing anything.

## What is published

- Whatever `main` contains at the time of the push. The demo is public; do not
  merge anything to `main` that should not be seen.
- `frontend/dist` from `npm run build`: the wasm editor plus the React UI.
  There is no server part.
- By `.github/workflows/pages.yml`: on every push to `main` and on manual
  start (Actions > Pages > Run workflow). Never on pull requests.
- URL: `https://curvyo.github.io/curvyo/` until the custom domain is set,
  then `https://demo.curvyo.org/`. Asset paths are relative (`base: "./"` in
  `frontend/vite.config.ts`), so both work from the same build.

## One-time setup (repo owner)

1. Repo Settings > Pages > Build and deployment > Source: **GitHub Actions**.
   The first deploy fails until this is set. Re-run the Pages workflow
   afterwards.
2. DNS: at the DNS provider of `curvyo.org`, add a `CNAME` record
   `demo` pointing to `curvyo.github.io`.
3. Repo Settings > Pages > Custom domain: `demo.curvyo.org`, then tick
   **Enforce HTTPS** once the certificate is issued (can take some minutes).

The custom domain is a repository setting. For an Actions-deployed site a
`CNAME` file in the build output is ignored, so there is none in the repo.
The org's other Pages site (`curvyo.org`) is not affected: each repository has
its own Pages site and domain.

## Run the same build locally

```text
cd frontend
npm ci
npm run build          # builds the wasm bindings first (needs wasm-bindgen CLI, see scripts/build-wasm.sh)
npx vite preview       # serves frontend/dist
```

## Limits of the browser build

- Needs a browser with WebAssembly and WebGL 2. If it is missing, a message is
  shown in the bar at the top.
- No device access (no machines, no serial/USB).
- There is no native menu. The top bar has New, Open and Save instead. Open
  picks a `.curvyo` file; Save downloads `untitled.curvyo`. Nothing is stored
  on a server and nothing is saved automatically.
- The desktop app (Tauri) is unchanged: the bar is not shown there.
