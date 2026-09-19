# Reflective Graph workbench

Use Node 25.4.0, npm 11.7.0 and Rust 1.85.0 with wasm32-unknown-unknown installed.

```sh
npm ci
npm run dev
```

For a production build use npm run build, then npm run preview. The build script installs the matching locked wasm-bindgen CLI locally when required. Generated packages and output are ignored.

Verification: npm run lint, npm run test:unit, npm run test:parity, and npm run test:browser (after npx playwright install chromium). Browser tests use the production build, which must be rebuilt after Rust or frontend changes.

See [the workbench contract](../docs/reference/workbench.md) for commands, limits, interruption, replay and research boundaries.

## UI reference for future work

The dark palette is adapted from Moltconn's web package at `/Users/saeedshahani/MEGA/Business/moltconn/packages/web`, principally `src/index.css`: page `24 24 24`, surface `35 35 35`, hover `46 46 46`, primary text `255 255 255`, secondary text `196 196 196`, and border `56 56 56`. Reflective Graph retains its own green `#265b43` as the primary action colour in both themes and uses a lighter green only where dark-background text needs more contrast.

For later interface work, Moltconn components may be consulted as visual and interaction references. Adapt the relevant pattern into this workbench rather than creating a runtime import or package dependency, and preserve this project's semantic, accessibility and test boundaries.
