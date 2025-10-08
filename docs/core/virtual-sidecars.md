# Virtual Sidecar System (VSS)

The Virtual Sidecar System (VSS) is a core component of Spacedrive that manages derivative data associated with your files. This system allows Spacedrive to generate and manage things like thumbnails, OCR text, video transcripts, and other metadata-rich artifacts.

The VSS is built on a key principle: **it does not modify your original files during indexing**. However, it can move files at the user's request. This is achieved through a dual system of "Managed Sidecars" and "Reference Sidecars".

## Key Concepts

- **Managed Sidecars:** These are derivative files that Spacedrive **generates**. Examples include thumbnails, OCR text, and video proxies. These files are stored directly in a managed directory within the Spacedrive library.
- **Reference Sidecars:** These are pre-existing files that are treated as sidecars. A common example is the video component of a Live Photo. These files are **not moved** during indexing. Instead, Spacedrive creates a "reference" to the file in its original location. The user can later choose to "bulk convert" these reference sidecars into managed sidecars, which moves the files into the managed directory.
- **Content-Scoped:** Sidecars are associated with the content of a file, not its path. This means that if you have multiple copies of the same file, they will all share the same set of sidecars, which are generated only once.
- **Portability:** All **managed** sidecars are stored within the `.sdlibrary` directory, making your entire organized ecosystem, including all generated intelligence, completely portable.

## How it Works

The VSS is a combination of a specific filesystem layout, a database schema for tracking, and a set of services for managing the entire lifecycle of a sidecar.

## Managed vs. Reference Sidecars

The VSS handles two types of sidecars differently:

- **Managed Sidecars:** When Spacedrive generates a new derivative file (e.g., a thumbnail), it is stored directly in the `sidecars` directory within the `.sdlibrary`. This is the standard behavior for generated content.

- **Reference Sidecars:** When Spacedrive identifies a pre-existing file that should be treated as a sidecar (e.g., the video part of a Live Photo), it does **not** move the file. Instead, it creates a `sidecar` record in the database with a `source_entry_id` that points to the original file. This allows Spacedrive to track the file as a sidecar without modifying its location. At any time, the user can choose to convert a reference sidecar into a managed sidecar, which will move the file into the `sidecars` directory.

This dual system provides the best of both worlds: it allows Spacedrive to manage its own generated content efficiently, while also respecting the user's original file organization.

### Filesystem Layout

All sidecars are stored in a `sidecars` directory within the `.sdlibrary`. The path to a sidecar is deterministic and is derived from the content UUID of the file it is associated with, the kind of sidecar, and a variant.

A typical path looks like this:

```
.sdlibrary/
  sidecars/
    content/
      {h0}/{h1}/{content_uuid}/
        thumbs/{variant}.webp
        proxies/{profile}.mp4
        embeddings/{model}.json
        ocr/ocr.json
```

- `{h0}/{h1}`: These are the first two byte-pairs of the content UUID, used for sharding the directory structure to ensure good filesystem performance at scale.
- `{content_uuid}`: The unique identifier of the content the sidecar is associated with.
- `thumbs/{variant}.webp`: An example of a thumbnail sidecar.

### Database Schema

The VSS uses two tables in the Spacedrive database:

- `sidecars`: This table tracks all the sidecars that have been generated or are pending generation. It stores information like the content UUID, kind, variant, format, path, size, and status.
- `sidecar_availability`: This table tracks which devices in your library have a copy of a particular sidecar. This is used to avoid regenerating sidecars that already exist on another device.

### Lifecycle of a Sidecar

1.  **Identification/Enqueueing:**
    - For **managed sidecars**, the "Intelligence Queueing Phase" of the indexer determines which sidecars should be generated for a file and enqueues a generation job. This creates a "pending" record in the `sidecars` table.
    - For **reference sidecars**, the indexer identifies a pre-existing file that should be treated as a sidecar and creates a `sidecar` record with a `source_entry_id` pointing to the original file.
2.  **Generation (Managed Sidecars Only):** A background job system (currently in development) picks up the pending generation requests. The job generates the sidecar file and stores it in the deterministic path.
3.  **Recording:** Once a managed sidecar is generated, the job updates the `sidecars` table, marking the sidecar as "ready" and recording its size and other metadata. It also updates the `sidecar_availability` table to indicate that the current device has this sidecar. For reference sidecars, the record is created as "ready" immediately.
4.  **Syncing:** When a device comes online, it can exchange sidecar availability information with its peers. If a device needs a sidecar that it doesn't have locally, it can check the `sidecar_availability` table to see if another device has it. If so, it can transfer the sidecar directly from the peer instead of regenerating it.

## Current Implementation Status

The VSS is partially implemented. Here is a summary of the current status:

- **Implemented:**
  - The core `SidecarManager` service.
  - Filesystem layout and path generation.
  - Database schema and queries for `sidecars` and `sidecar_availability`.
  - Enqueueing of sidecar generation jobs.
  - A `bootstrap_scan` function to synchronize the database with the filesystem.
  - Reference sidecars, which allow tracking files as sidecars without moving them.
- **To-Do:**
  - **Job Dispatch and Execution:** The system for actually executing the generation jobs is not yet implemented. This is the most critical missing piece.
  - **Filesystem Watcher:** A watcher for the `sidecars` directory is needed for real-time updates.
  - **Checksumming:** Checksumming of sidecar files for integrity verification is not yet implemented.

## How to Use the VSS (API)

The `SidecarManager` provides a set of APIs for interacting with the VSS. These are primarily for internal use by other Spacedrive services, but they can also be used for development and debugging.

- `sidecars.presence(content_uuids, kind, variants)`: Checks for the presence of sidecars for a given set of content UUIDs.
- `sidecars.path(content_uuid, kind, variant)`: Gets the local path to a sidecar, or enqueues it for generation if it doesn't exist.
- `sidecars.reconcile()`: Triggers a bootstrap scan to reconcile the database with the filesystem.

## Benefits of the VSS

The Virtual Sidecar System provides a number of benefits:

- **Preserves Original File Organization:** The use of reference sidecars means that Spacedrive does not need to move your pre-existing files (like Live Photos) to treat them as sidecars.
- **Portable Intelligence:** All generated data is stored in the library, making it easy to back up and move your entire Spacedrive setup.
- **Efficient:** Sidecars are generated only once per piece of content and can be shared between devices, saving processing time and storage space.
- **Extensible:** The system is designed to be extensible, allowing new kinds of sidecars to be added in the future.
- **Foundation for AI:** The VSS is the foundation for Spacedrive's AI capabilities, providing the structured data needed for features like semantic search and intelligent organization.
