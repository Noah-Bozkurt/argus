# Argus for Astro

This package contains no website content or components. It loads immutable releases and provides a browser bridge for an isolated preview website.

Add `argus()` from `@argus/astro` to Astro's integrations. Set `ARGUS_CONTENT_MODE=cms`, `ARGUS_CONTENT_PUBLIC_URL`, and `ARGUS_PROJECT_ID`. Import the pinned release from `virtual:argus-content`; records are under `release.snapshot.models[slug].records`. Register and render your own Astro components by their CMS slug.

Keep `ARGUS_CONTENT_MODE` unset to retain your site's explicit local-content mode during migration. CMS mode fails if a release cannot be loaded. The build resolves `build` once; `ARGUS_RELEASE_ID` can select an existing immutable release for a reproducible rebuild. Serve the generated `argus-release.json` at the site root.

## Preview

Use a separate server-rendered Astro Worker. Its preview route handles POST requests by passing the incoming bearer grant to `loadPreview` from `@argus/astro/runtime`, verifies the returned project, and renders the authorized record using its registered components. Apply submitted field/layout overrides only to this render: never persist them from the preview Worker. Reject unknown components and cap request size.

The GET route serves a `[data-argus-canvas]` element and runs `connectPreview({ operatorOrigin })` from `@argus/astro/preview`. Mark rendered sections with `data-argus-block` containing their block UUID. The bridge validates the parent origin, re-renders on changes, and sends section selection back to the editor. Permit framing only by the configured operator origin with CSP `frame-ancestors`. Never include production analytics or third-party scripts on the preview site. Responses must use `Cache-Control: private, no-store` and `Referrer-Policy: no-referrer`.

Production may use Pages or Workers static assets. Draft preview requires the separate Worker; production is not rebuilt while typing.
