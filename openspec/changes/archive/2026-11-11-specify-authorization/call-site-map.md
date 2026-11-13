# Call-site map

One row per file under `src/lib/application/use_case/` that today calls `Authorization::decide`,
`Authorization::decide_asset`, `Authorization::decide_library_surface` or
`Authorization::decide_library_manage_surface`. The map is the scope definition for tasks 4, 5, 7 and 9. A listing that
drops an item records **filter** instead of a denial variant.

| File                         | Resource kind               | Action                                      | Child kind        | Denial answer                    | Task |
| ---------------------------- | --------------------------- | ------------------------------------------- | ----------------- | -------------------------------- | ---- |
| `add_gallery_item.rs`        | `GALLERY` + `IMAGE`/`VIDEO` | `CreateChild` + `View`                      | `GALLERY_ITEM`    | `NoSuchGallery`/`NotOwnedMedia`  | 9.7  |
| `apply_movie.rs`             | `MOVIE`                     | `ViewMetadata` then `EditMetadata`          | —                 | `NoSuchMovie`                    | 9.5  |
| `apply_music_album.rs`       | `MUSIC_ALBUM`               | `ViewMetadata` then `EditMetadata`          | —                 | `NoSuchAlbum`                    | 9.5  |
| `apply_series.rs`            | `SERIES`                    | `ViewMetadata` then `EditMetadata`          | —                 | `NoSuchSeries`                   | 9.5  |
| `apply_series_episode.rs`    | `SERIES`                    | `ViewMetadata` then `EditMetadata`          | —                 | `NoSuchSeries`                   | 9.5  |
| `cancel_upload.rs`           | `UPLOAD`                    | `ReplaceMedia`                              | —                 | `NoSuchUpload`                   | 9.5  |
| `complete_upload.rs`         | `UPLOAD`                    | `ReplaceMedia`                              | —                 | `NoSuchUpload`                   | 9.5  |
| `create_movie.rs`            | `CATALOGUE`                 | `CreateChild`                               | `MOVIE`           | `Forbidden`                      | 9.6  |
| `create_music_album.rs`      | `CATALOGUE`                 | `CreateChild`                               | `MUSIC_ALBUM`     | `Forbidden`                      | 9.6  |
| `create_music_group.rs`      | `CATALOGUE`                 | `CreateChild`                               | `MUSIC_GROUP`     | `Forbidden`                      | 9.6  |
| `create_series.rs`           | `CATALOGUE`                 | `CreateChild`                               | `SERIES`          | `Forbidden`                      | 9.6  |
| `create_gallery.rs`          | `GALLERY_COLLECTION`        | `CreateChild`                               | `GALLERY`         | `Forbidden`                      | 9.6  |
| `create_playlist.rs`         | `PLAYLIST_COLLECTION`       | `CreateChild`                               | `MUSIC_PLAYLIST`  | `Forbidden`                      | 9.6  |
| `begin_upload.rs`            | `UPLOAD_COLLECTION`         | `CreateChild`                               | `UPLOAD`          | `Forbidden`                      | 9.6  |
| `delete_gallery.rs`          | `GALLERY`                   | `Delete` then `View` per asset              | —                 | `NoSuchGallery`                  | 9.5  |
| `delete_gallery_item.rs`     | `GALLERY`                   | `Delete` then `View` on the media           | —                 | `NoSuchGallery`                  | 9.5  |
| `delete_playlist.rs`         | `MUSIC_PLAYLIST`            | `Delete`                                    | —                 | `NoSuchPlaylist`                 | 9.5  |
| `delete_user.rs`             | `USER`                      | `Delete`                                    | —                 | `NotAdmin`                       | 9.8  |
| `get_album.rs`               | `MUSIC_ALBUM`               | `View`                                      | —                 | `NoSuchAlbum`                    | 9.4  |
| `get_album_medium.rs`        | `MUSIC_ALBUM`               | `View`                                      | —                 | `NoSuchAlbum`                    | 9.4  |
| `get_album_track.rs`         | `MUSIC_RECORDING`           | `View`                                      | —                 | `NoSuchAlbum`                    | 9.4  |
| `get_asset_content.rs`       | `FILE` -> typed asset       | `Download` or `View`                        | —                 | `NoSuchAsset`                    | 9.4  |
| `get_audio.rs`               | `AUDIO` -> `FILE`           | `Stream`                                    | —                 | `NoSuchAudio`                    | 9.4  |
| `get_current_user.rs`        | `USER`                      | `ViewMetadata`                              | —                 | `NoSuchUser`                     | 9.8  |
| `get_file.rs`                | `FILE` -> typed asset       | `Download`                                  | —                 | `NoSuchFile`                     | 9.4  |
| `get_gallery.rs`             | `GALLERY`                   | `View`                                      | —                 | `NoSuchGallery`                  | 9.4  |
| `get_gallery_item.rs`        | `GALLERY_ITEM`              | `View`                                      | —                 | `NoSuchGallery`                  | 9.4  |
| `get_image.rs`               | `IMAGE` -> `FILE`           | `View`                                      | —                 | `NoSuchImage`                    | 9.4  |
| `get_movie.rs`               | `MOVIE`                     | `View`                                      | —                 | `NoSuchMovie`                    | 9.4  |
| `get_music_group.rs`         | `MUSIC_GROUP`               | `View`                                      | —                 | `NoSuchGroup`                    | 9.4  |
| `get_permission.rs`          | `USER`                      | `ManagePermissions` (self exception)        | —                 | `Forbidden`                      | 7.1  |
| `get_playlist.rs`            | `MUSIC_PLAYLIST`            | `View`                                      | —                 | `NoSuchPlaylist`                 | 9.4  |
| `get_playlist_track.rs`      | `MUSIC_RECORDING`           | `View`                                      | —                 | `NoSuchPlaylist`                 | 9.4  |
| `get_series.rs`              | `SERIES`                    | `View`                                      | —                 | `NoSuchSeries`                   | 9.4  |
| `get_series_episode.rs`      | `EPISODE`                   | `View`                                      | —                 | `NoSuchSeries`                   | 9.4  |
| `get_series_season.rs`       | `SEASON`                    | `View`                                      | —                 | `NoSuchSeries`                   | 9.4  |
| `get_upload.rs`              | `UPLOAD`                    | `ViewMetadata`                              | —                 | `NoSuchUpload`                   | 9.4  |
| `get_user.rs`                | `USER`                      | `ViewMetadata`                              | —                 | `NotAdmin`                       | 9.8  |
| `get_video.rs`               | `VIDEO` -> `FILE`           | `Stream`                                    | —                 | `NoSuchVideo`                    | 9.4  |
| `identify_music.rs`          | `CATALOGUE`                 | `ViewMetadata`                              | —                 | `NotPermitted`                   | 9.4  |
| `identify_video.rs`          | `CATALOGUE`                 | `ViewMetadata`                              | —                 | `NotPermitted`                   | 9.4  |
| `list_album_tracks.rs`       | `MUSIC_ALBUM`               | `View`                                      | —                 | `NoSuchAlbum`                    | 9.4  |
| `list_albums.rs`             | `MUSIC_ALBUM`               | `Discover`                                  | —                 | filter                           | 9.4  |
| `list_galleries.rs`          | `GALLERY`                   | `Discover`                                  | —                 | filter                           | 9.4  |
| `list_movies.rs`             | `MOVIE`                     | `Discover`                                  | —                 | filter                           | 9.4  |
| `list_music_groups.rs`       | `MUSIC_GROUP`               | `Discover`                                  | —                 | filter                           | 9.4  |
| `list_playlists.rs`          | `MUSIC_PLAYLIST`            | `Discover`                                  | —                 | filter                           | 9.4  |
| `list_series.rs`             | `SERIES`                    | `Discover`                                  | —                 | filter                           | 9.4  |
| `list_series_episodes.rs`    | `EPISODE`                   | `Discover`                                  | —                 | `NoSuchSeries`                   | 9.4  |
| `list_users.rs`              | `USER`                      | `Discover`                                  | —                 | `NotAdmin`                       | 9.8  |
| `patch_album_track.rs`       | `MUSIC_RECORDING`           | `View` then `EditMetadata`                  | —                 | `NoSuchAlbum`                    | 9.5  |
| `patch_credential.rs`        | `USER`                      | `ReplaceCredential`                         | —                 | `Forbidden`                      | 9.8  |
| `put_album_medium.rs`        | `MUSIC_ALBUM`               | `View` then `CreateChild`                   | `MUSIC_MEDIUM`    | `NoSuchAlbum`                    | 9.6  |
| `put_album_track.rs`         | `MUSIC_MEDIUM`              | `View` then `CreateChild`                   | `MUSIC_RECORDING` | `NoSuchAlbum`                    | 9.6  |
| `put_series_episode.rs`      | `SEASON`                    | `ViewMetadata` then `CreateChild`           | `EPISODE`         | `Forbidden`                      | 9.6  |
| `put_series_season.rs`       | `SERIES`                    | `ViewMetadata` then `CreateChild`           | `SEASON`          | `Forbidden`                      | 9.6  |
| `register_user.rs`           | `USER_COLLECTION`           | `CreateChild`                               | `USER`/`ADMIN`    | `Forbidden`                      | 9.8  |
| `remove_playlist_track.rs`   | `MUSIC_PLAYLIST`            | `Delete`                                    | —                 | `NoSuchTrack`                    | 9.5  |
| `reorder_playlist_tracks.rs` | `MUSIC_PLAYLIST`            | `EditMetadata`                              | —                 | `NoSuchPlaylist`                 | 9.5  |
| `replace_permission.rs`      | `USER`                      | `ManagePermissions`                         | —                 | `Forbidden`                      | 7.1  |
| `resolve_music_metadata.rs`  | `CATALOGUE`                 | `ViewMetadata`                              | —                 | `NotPermitted`                   | 9.4  |
| `resolve_video_metadata.rs`  | `CATALOGUE`                 | `ViewMetadata`                              | —                 | `NotPermitted`                   | 9.4  |
| `update_movie.rs`            | `MOVIE`                     | `View` then `EditMetadata`                  | —                 | `NoSuchMovie`                    | 9.5  |
| `update_music_album.rs`      | `MUSIC_ALBUM`               | `View` then `EditMetadata`                  | —                 | `NoSuchAlbum`                    | 9.5  |
| `update_music_group.rs`      | `MUSIC_GROUP`               | `View` then `EditMetadata`                  | —                 | `NoSuchGroup`                    | 9.5  |
| `update_playlist.rs`         | `MUSIC_PLAYLIST`            | `View` then `EditMetadata` (+`CreateChild`) | —                 | `NoSuchPlaylist`                 | 9.5  |
| `update_series.rs`           | `SERIES`                    | `View` then `EditMetadata`                  | —                 | `NoSuchSeries`                   | 9.5  |
| `update_series_episode.rs`   | `EPISODE`                   | `View` then `EditMetadata`                  | —                 | `NoSuchSeries`                   | 9.5  |
| `update_series_season.rs`    | `SEASON`                    | `View` then `EditMetadata`                  | —                 | `NoSuchSeries`                   | 9.5  |
| `update_user.rs`             | `USER`                      | `EditMetadata` (+`ChangeRole`)              | —                 | `NotAdmin`/`RoleChangeForbidden` | 9.8  |
| `write_upload_chunk.rs`      | `UPLOAD`                    | `ReplaceMedia`                              | —                 | `NoSuchUpload`                   | 9.5  |

`create_gallery`, `create_playlist`, `begin_upload` and the content-creation use cases gain a decision they do not have
today; every other row replaces an existing call. The new permission routes (`7.1`–`7.8`) add `get_permission_summary`,
`get_resource_policy`, `replace_resource_policy`, `share_resource` and `revoke_share`.
