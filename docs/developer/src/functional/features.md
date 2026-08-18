# Feature catalogue

The v1 feature set as delivered. Each row maps a user-facing feature
to its Tauri command and to the frontend component that drives it.

## Library management

| Feature                        | Command                          | Frontend component            |
| ------------------------------ | -------------------------------- | ----------------------------- |
| Add a library folder           | `add_library_folder`             | `TopBar`                      |
| Remove a library folder        | `remove_library_folder`          | `Sidebar` (right-click menu)  |
| List library folders           | `list_library_folders`           | `Sidebar`, `App` bootstrap    |
| Rescan a single folder         | `rescan_folder`                  | `Sidebar` (right-click menu)  |
| Rescan every folder            | `rescan_all`                     | `TopBar`                      |

## Browsing

| Feature                        | Command                          | Frontend component            |
| ------------------------------ | -------------------------------- | ----------------------------- |
| Paged image listing            | `query_images`                   | `ImageGrid`                   |
| Sort by taken/modified/…       | `query_images` (`sort` arg)      | `TopBar`                      |
| Filter by folder / rating / tag| `query_images` (`filter` arg)    | `Sidebar` filters             |
| Full-text search               | `query_images` (`filter.search`) | `TopBar` search box           |

## Editing

| Feature                        | Command                          | Frontend component            |
| ------------------------------ | -------------------------------- | ----------------------------- |
| Fetch a photo's details        | `get_image`                      | `DetailsPanel/SingleDetails`  |
| Auto-save title / comment      | `update_image_metadata`          | `DetailsPanel/SingleDetails`  |
| Set rating                     | `update_image_metadata`          | `StarRating`                  |
| Add / remove tag on one photo  | `update_image_metadata`          | `TagInput`                    |
| Bulk set rating                | `batch_update_metadata`          | `DetailsPanel/MultiDetails`   |
| Bulk add / remove tags         | `batch_update_metadata`          | `DetailsPanel/MultiDetails`   |

## Tag maintenance

| Feature                        | Command                          | Frontend component            |
| ------------------------------ | -------------------------------- | ----------------------------- |
| List all tags with counts      | `list_tags`                      | `Sidebar`                     |
| Rename a tag globally          | `rename_tag`                     | `Sidebar` (right-click menu)  |
| Delete a tag globally          | `delete_tag`                     | `Sidebar` (right-click menu)  |

## Smart collections (skeleton, v1 non-editable)

| Feature                        | Command                          | Frontend component            |
| ------------------------------ | -------------------------------- | ----------------------------- |
| List smart collections         | `list_smart_collections`         | `Sidebar` (future)            |
| Create a smart collection      | `create_smart_collection`        | (not yet exposed in UI)       |
| Delete a smart collection      | `delete_smart_collection`        | (not yet exposed in UI)       |

## Deletion

| Feature                        | Command                          | Frontend component            |
| ------------------------------ | -------------------------------- | ----------------------------- |
| Move photos to Recycle Bin     | `delete_images` (permanent=false)| `DetailsPanel`, `App` (Del key)|
| Permanently delete photos      | `delete_images` (permanent=true) | `DetailsPanel`, `App` (Shift+Del)|

## Diagnostics

| Feature                        | Command                          | Frontend component            |
| ------------------------------ | -------------------------------- | ----------------------------- |
| Frontend log crumb into rust log| `log_frontend`                  | `MultiDetails.applyTags`      |

## Thumbnails and image display

| Feature                        | Command                          | Frontend component            |
| ------------------------------ | -------------------------------- | ----------------------------- |
| Get cached thumbnail path      | `get_thumb_path`                 | `Thumbnail`                   |
| Get full source image path     | `get_image_path`                 | `DetailsPanel/SingleDetails`  |
