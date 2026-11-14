import type { Flow } from "../config.ts";

import { browseGalleries } from "./browse-galleries.ts";
import { browseMusic } from "./browse-music.ts";
import { browseVideo } from "./browse-video.ts";
import { createGallery } from "./create-gallery.ts";
import { curatePlaylist } from "./curate-playlist.ts";
import { login } from "./login.ts";
import { readAsset } from "./read-asset.ts";
import { uploadAsset } from "./upload-asset.ts";

import type { FlowFn } from "./types.ts";

/** The flow catalog, keyed by the name the scenarios route on. */
export const FLOWS: Record<Flow, FlowFn> = {
  "browse-galleries": browseGalleries,
  "browse-music": browseMusic,
  "browse-video": browseVideo,
  "create-gallery": createGallery,
  "curate-playlist": curatePlaylist,
  login,
  "read-asset": readAsset,
  "upload-asset": uploadAsset,
};
