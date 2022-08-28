# 0.1.0_beta

After __ months of development we are extremely excited to be releasing the first version of Spacedrive as a public beta.

This is an MVP, and by no means feature complete. 

It is desktop only, but mobile has already begun development.

*Features:*
- Support for Windows, Linux and macOS.
- Create [Libraries](../architecture/libraries.md) and switch between them.
- Connect multiple [Nodes](../architecture/nodes.md) via LAN and synchronize Library data in realtime.
- Add [Locations](../architecture/locations.md) to be indexed and watched for files.
  - Indexer keeps watch for changes and performs light rescans.
- Browse Locations via the [Explorer](../architecture/explorer.md) and view file preview and metadata.
  - Multi-select and Context menu.
  - Viewer options: row/grid item size, gap adjustment, show/hide info.
- Identify unique files to discover duplicates.
- Generate [Preview Media](../architecture/preview-media.md) for image, video and text.
- Create [Tags](../architecture/tags.md) and assign them to files, browse Tags in the Explorer.
- Create [Spaces](../architecture/spaces.md) to virtually organize and present files.
- Create [Albums](../architecture/albums.md) and add images.
- [Search](../architecture/search.md) files in Library via ⌘K or CTRL+K.
- Pause and resume [Jobs](../architecture/jobs.md) with recovery on crash via Job Manager widget.
- Automatic, configurable, local database backups.