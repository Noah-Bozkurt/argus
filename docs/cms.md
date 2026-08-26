# Using the Argus CMS

Argus gives every Project its own isolated content workspace. A client is optional: personal, internal and client projects use the same CMS model.

Open a Project in Argus and choose **Content**. The native Content workspace is the normal authoring surface; Payload remains the storage/authentication engine behind it.

## Project boundary and roles

Content, media, forms and application data always belong to one Argus Project. Records cannot be moved between Projects and relationships cannot point across Project boundaries.

Project content roles are:

- `viewer` — read the Project's private CMS workspace, media, forms and submissions;
- `editor` — viewer access plus create/edit schemas, records, drafts, publication, media and forms;
- `manager` — editor access plus destructive lifecycle actions such as permanent record/model/media/submission deletion and membership management.

Organization `owner` and `admin` users inherit access to Projects in their organization. Other workspace users require an explicit Project membership. The Argus Web application forwards the authenticated workspace identity to the Content service; Content re-resolves that user and checks the required Project role before privileged Payload operations are performed.

## Content types

A Project can define up to 50 typed fields per content type. Fields can be reordered, added or removed from the native schema editor.

Supported field types:

- short text;
- long text;
- number;
- boolean;
- date;
- date and time;
- JSON;
- relationship to another content type in the same Project;
- single or multiple media images.

Every content type also has a role:

### Collection

A repeatable set of standalone records such as:

- blog posts;
- projects;
- team members;
- products;
- release notes.

### Component

A reusable visual page block schema such as:

- hero;
- call to action;
- feature grid;
- image/text section;
- latest-posts configuration.

Components do not have standalone records and are not public by themselves. A Page type explicitly allows the Components that authors may place in its layout.

### Page

A page record combines normal fields with an ordered component layout. The native visual editor provides:

- a component palette;
- drag-and-drop block ordering;
- desktop, tablet and mobile canvas widths;
- a live structured preview for each block;
- an inspector for typed block fields;
- duplicate and remove actions.

The visual editor is intentionally schema-driven rather than a free-form HTML/CSS builder. The website remains responsible for how a component slug is rendered. This keeps authored content portable and prevents arbitrary stored markup from becoming application code.

## Stable schemas and versions

A content type's API slug and role are immutable after creation. Existing website integrations can therefore keep using the same endpoint and renderer mapping.

Changing field definitions or the allowed Component set increments `schemaVersion`. Metadata-only changes such as the display name, description, public visibility or lifecycle status do not create a false schema version.

A content type can be archived and restored. Permanent deletion requires `manager` access and is only accepted when the type has no records.

## Draft, publish and record lifecycle

Content records have two independent states:

- editorial state: `draft` or `published`;
- lifecycle state: `active` or `archived`.

**Save draft** keeps the edit private. **Publish** makes the current version available through the public API only when the content type also has **Public when published** enabled.

Payload keeps up to 50 versions per content record. Archived records disappear from public delivery but can be restored. A manager can permanently delete a record; its relationship edges are removed in the same database transaction.

Record values and relationships are saved transactionally so a failed relationship update cannot leave a successfully changed record with only part of its relationship set.

## Media

The Project media library accepts JPEG, PNG, WebP and AVIF images up to 10 MiB. Each image has required alternative text, an optional caption and an explicit public/private flag.

Generated variants include bounded thumbnail, medium and large sizes. Media fields only select assets from the same Project.

A public content response only resolves media that is currently public. A private or deleted asset is not leaked merely because an old record still references its UUID.

## Forms

Forms are also Project-owned. A form has a stable slug, fields, success message and a `draft`, `published` or `archived` lifecycle.

Public endpoints:

```text
GET  /public/projects/:projectId/forms/:formSlug
POST /public/projects/:projectId/forms/:formSlug
```

The GET endpoint returns the renderable public schema. The POST endpoint accepts JSON containing `values`. Submissions are private and can be reviewed inside Argus, exported to CSV or removed by a manager.

## Public content API

A public content type is read with:

```text
GET /public/projects/:projectId/content/:modelSlug
```

Useful query parameters:

```text
?limit=50&page=1
?expand=relationships
```

Only an active Project, active public content type and active published records are returned. Drafts, memberships, internal organization IDs and private Payload metadata are never part of this response.

Relationship expansion is opt-in and limited to one bounded level. Related records are returned only when the target record is independently active/published and its target model is public.

A website needs **no Argus service token** to read public CMS content. Never put `ARGUS_CONTENT_SYNC_TOKEN` or other Argus internal credentials in a website or browser bundle.

## Recommended website integration

A site normally needs two public environment variables:

```text
ARGUS_CONTENT_PUBLIC_URL=https://content.example-argus.tld
ARGUS_PROJECT_ID=00000000-0000-4000-8000-000000000000
```

For an Astro site, a small server/build helper is enough:

```ts
// src/lib/argus-content.ts
const baseURL = import.meta.env.ARGUS_CONTENT_PUBLIC_URL;
const projectId = import.meta.env.ARGUS_PROJECT_ID;

export type ArgusRecord<T> = {
  id: string;
  values: T;
  layout: Array<{
    id: string;
    component: string;
    values: Record<string, unknown>;
  }>;
  published_at: string | null;
  updated_at: string | null;
};

export async function getArgusContent<T>(model: string) {
  const response = await fetch(
    `${baseURL}/public/projects/${projectId}/content/${model}?limit=100`,
  );
  if (!response.ok) {
    throw new Error(`Argus content request failed: ${response.status}`);
  }
  return (await response.json()) as { records: ArgusRecord<T>[] };
}
```

For a statically generated Astro site, this fetch runs at build time. Publishing in Argus therefore becomes visible after the site's next build/deployment. An SSR/on-demand-rendered site can fetch at request time instead. Argus does not currently trigger an arbitrary external site's rebuild when content is published.

## Example: adapting `Noah-Bozkurt/youpspace.com`

The current YoupSpace Astro repository is a useful example because it already separates several kinds of content but stores them in source control:

- `src/data/site.ts` contains site identity, descriptions, navigation, business details and social links;
- `src/data/projects.ts` and `src/data/projects-nl.ts` contain project information;
- `src/content.config.ts` defines an Astro Markdown/MDX blog collection;
- `src/pages/index.astro` imports those local sources and also contains substantial homepage copy directly in the template;
- contact delivery currently points at Formspree.

Argus can replace the editable content parts without making the site's visual components generic.

### Suggested YoupSpace content model

Create these collection types inside the YoupSpace Argus Project:

#### `site_settings`

One record, with fields such as:

```text
name                 text
seo_description_nl   textarea
seo_description_en   textarea
business_email       text
social_links         json
navigation           json
```

Keep truly deployment-specific values such as the canonical production hostname in code/environment when that is more appropriate than editorial content.

#### `projects`

```text
slug             text, required
name             text, required
summary_nl       textarea
summary_en       textarea
status           text
featured         boolean
cover            media
repository_url   text
project_url      text
```

The existing TypeScript project arrays can then be replaced with data returned from the public `projects` model.

#### `blog_posts`

A practical translation of the current Astro collection is:

```text
locale            text, required
slug              text, required
translation_key   text, required
title             text, required
description       textarea, required
publish_date       datetime, required
updated_date       datetime
author             text
category           text
tags               json
project             relationship -> projects
cover               media
featured            boolean
external_video      text
callouts             json
body_markdown        textarea
```

The old `draft` boolean is unnecessary: use Argus draft/publication state. The old `archived` boolean is unnecessary: use the Argus record lifecycle.

Markdown is a reasonable first migration for long-form blog bodies because it keeps the frontend renderer explicit. A richer portable rich-text field can be added later without storing arbitrary executable HTML.

### Suggested visual homepage

Create Component schemas:

- `hero` — eyebrow, heading, body, image, primary label/link and secondary label/link;
- `featured_projects` — heading, intro, limit and optional featured-only toggle;
- `project_spotlight` — project slug, heading, body and image;
- `latest_posts` — heading, limit and optional category;
- `cta` — heading, body, label and URL.

Then create a Page type named `home_page`, allow those components and publish one record. Its visual layout may look like:

```text
Home page
├─ Hero
├─ Featured projects
├─ Project spotlight
├─ Latest posts
└─ CTA
```

For data-driven blocks such as `featured_projects` and `latest_posts`, store display configuration in the block and let the Astro renderer query the corresponding public collection. This is preferable to copying complete project/blog records into page JSON.

The Astro homepage can map Argus component slugs to the site's existing components:

```astro
---
import Hero from '../components/home/Hero.astro';
import FeaturedProjects from '../components/home/FeaturedProjects.astro';
import ProjectSpotlight from '../components/home/ProjectSpotlight.astro';
import LatestPosts from '../components/home/LatestPosts.astro';
import Cta from '../components/home/Cta.astro';
import { getArgusContent } from '../lib/argus-content';

const home = (await getArgusContent('home_page')).records[0];
const projects = (await getArgusContent('projects')).records;
const posts = (await getArgusContent('blog_posts')).records;
---

{
  home.layout.map((block) => {
    switch (block.component) {
      case 'hero':
        return <Hero {...block.values} />;
      case 'featured_projects':
        return <FeaturedProjects config={block.values} projects={projects} />;
      case 'project_spotlight':
        return <ProjectSpotlight config={block.values} projects={projects} />;
      case 'latest_posts':
        return <LatestPosts config={block.values} posts={posts} />;
      case 'cta':
        return <Cta {...block.values} />;
      default:
        return null;
    }
  })
}
```

Argus controls **which blocks and content appear**. The YoupSpace repository controls **what those blocks look like**. This separation is the intended visual-editor architecture.

### Replacing Formspree on YoupSpace

Create a `contact` form in the same Argus Project and publish it. The Astro contact form can POST directly to:

```text
https://<content-host>/public/projects/<youpspace-project-id>/forms/contact
```

Example browser request:

```js
await fetch(contactEndpoint, {
  method: 'POST',
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify({
    values: {
      name: form.name.value,
      email: form.email.value,
      message: form.message.value,
    },
  }),
});
```

The site does not receive a privileged token. Submissions are reviewed in the Project's Content workspace and can be exported from there.

## Migration order for an existing site

For a site such as YoupSpace, migrate incrementally rather than moving every string at once:

1. Create the Argus Project content schemas.
2. Migrate project cards and blog metadata/content first.
3. Add the public Astro fetch helper and render the CMS records with the existing UI components.
4. Move the homepage to a Page + Component layout once those renderers are stable.
5. Move contact handling to an Argus Form if desired.
6. Remove old local data/Markdown only after the Argus-backed build produces the same public site.

This preserves the repository as the source of truth for application code and visual design while making editorial content Project-owned in Argus.
