# Reflective Graph workbench

Use Node 25.4.0, npm 11.7.0 and Rust 1.85.0 with wasm32-unknown-unknown installed.

```sh
npm ci
npm run dev
```

For a production build use npm run build, then npm run preview. The build script installs the matching locked wasm-bindgen CLI locally when required. Generated packages and output are ignored.

Verification: npm run lint, npm run test:unit, npm run test:parity, and npm run test:browser (after npx playwright install chromium). Browser tests use the production build, which must be rebuilt after Rust or frontend changes.

See [the workbench contract](../docs/workbench.md) for commands, limits, interruption, replay and research boundaries.
