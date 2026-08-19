# Third-Party Notices

Argus is proprietary software, but it depends on third-party software that is distributed under separate licenses.

The Argus proprietary license applies only to original Argus materials. It does **not** change, replace, or restrict license rights and obligations attached to third-party dependencies, libraries, frameworks, tools, container images, or assets.

## Dependency sources of truth

The dependency set used by a particular revision is recorded primarily in:

- `Cargo.toml` and `Cargo.lock` for Rust dependencies;
- `package.json` files and `pnpm-lock.yaml` for JavaScript/TypeScript dependencies;
- container/deployment definitions where an external image or runtime is referenced.

Those components remain governed by the license terms supplied by their respective authors and distributors.

## Distribution

Before distributing an Argus build outside the organization or authorization scope in which it was created, third-party license obligations must be reviewed for that exact build. Depending on the dependency, those obligations may include retaining copyright notices, license texts, attribution, source offers, or other notices.

This file is a licensing boundary notice, not an exhaustive reproduction of every third-party license. Lockfiles and manifests should be used to generate an exact dependency/license inventory for a release when distribution begins.
