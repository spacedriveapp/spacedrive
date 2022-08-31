..............COMING SOON..............
# 0.1.0_pre-beta

After __ months of development we are extremely excited to be releasing the first version of Spacedrive as an early public beta.

This is an MVP, and by no means feature complete. Please test out the features listed below and give us feedback via Discord, email or GitHub Issues :D

This release is missing database synchronization between nodes (your devices), for now this renders connecting nodes useless, other than to transfer individual files. But don't worry, its coming very soon!

*Features:*
- Support for Windows, Linux and macOS, iOS and Android.
- Basic onboarding flow, determine use-case and preferences.
- Create [Libraries](../architecture/libraries.md) and switch between them.
- Connect multiple [Nodes](../architecture/nodes.md) via LAN and synchronize Library data in realtime.
- Add [Locations](../architecture/locations.md) to be indexed and watched for files.
  - Indexer keeps watch for changes and performs light re-scans.
- Browse Locations via the [Explorer](../architecture/explorer.md) and view file preview and metadata.
  - Multi-select and Context menu.
  - Viewer options: row/grid item size, gap adjustment, show/hide info.
- Identify unique files to discover duplicates.
- Generate [Preview Media](../architecture/preview-media.md) for image, video and text.
- Create [Tags](../architecture/tags.md) and assign them to files, browse Tags in the Explorer.
- Create [Spaces](../architecture/spaces.md) to virtually organize and present files.
- Create [Albums](../architecture/albums.md) and add images.
- [Search](../architecture/search.md) files in Library via ⌘L or CTRL+L.
- Drag and drop files to other nodes on a keybind, defaults to CTRL+Space, also possible from Explorer context menu.
- Pause and resume [Jobs](../architecture/jobs.md) with recovery on crash via Job Manager widget.