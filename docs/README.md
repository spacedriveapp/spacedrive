# Spacedrive Documentation

The Spacedrive docs site, built with [Fumadocs](https://fumadocs.dev) on Next.js. This directory is a standalone app with its own `bun install`, separate from the repo workspace.

```bash
bun install
bun run dev      # dev server on localhost:3000
bun run build    # production build
```

Or from the repo root: `just dev-docs` / `just build-docs`. Requires Node 20+.

## Layout

- `overview/`, `core/`, `extensions/`, `cli/`, `react/`, `gpui/`: published MDX content, one sidebar tab per folder. Navigation order lives in each folder's `meta.json`.
- `app/`, `lib/`, `components/`: the Next.js app. Pages are served from the site root, so `core/jobs.mdx` renders at `/core/jobs`.
- `public/`: images, logo, favicon.
- `plans/`, `core/design/`, `design/`, `archive/`: internal working docs, not part of the site.
- `WRITING_GUIDE.mdx`, `CODE_COMMENTS.mdx`: contributor guides, not published.

## Writing pages

Frontmatter supports `title`, `description`, and `sidebarTitle` (a shorter name for the sidebar). Callouts, `<Steps>`, `<Tabs>`, `<Cards>`, and `<Accordions>` are registered globally, so no imports are needed in MDX. Every page is also served as raw markdown at `<path>.md` and indexed in `/llms.txt`.
