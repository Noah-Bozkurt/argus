# Third-Party Notices

Argus is proprietary software, but it depends on third-party software that is distributed under separate licenses.

The Argus proprietary license applies only to original Argus materials. It does **not** change, replace, or restrict license rights and obligations attached to third-party dependencies, libraries, frameworks, tools, container images, or assets.

## Dependency sources of truth

The dependency set used by a particular revision is recorded primarily in:

- `Cargo.toml` and `Cargo.lock` for Rust dependencies;
- `package.json` files and `pnpm-lock.yaml` for JavaScript/TypeScript dependencies;
- container/deployment definitions where an external image or runtime is referenced.

Those components remain governed by the license terms supplied by their respective authors and distributors.

## Vendored Lucide icons

The Argus operator interface vendors selected SVG path geometry from [Lucide](https://lucide.dev/) so the UI can use one consistent icon system without adding a runtime dependency.

ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any purpose with or without fee is hereby granted, provided that the above copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

Some Lucide icons are derived from the Feather project and are distributed under the MIT License:

Copyright (c) 2013-present Cole Bemis

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Distribution

Before distributing an Argus build outside the organization or authorization scope in which it was created, third-party license obligations must be reviewed for that exact build. Depending on the dependency, those obligations may include retaining copyright notices, license texts, attribution, source offers, or other notices.

This file is a licensing boundary notice, not an exhaustive reproduction of every third-party license. Lockfiles and manifests should be used to generate an exact dependency/license inventory for a release when distribution begins.
