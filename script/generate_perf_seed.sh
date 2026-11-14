#!/usr/bin/env bash
# Emit the volume library every `test-seed` boot applies.
#
# The three files under `test/seed/sql` are committed, so no suite needs this
# script to boot a server; it exists so the volumes can be re-derived from one
# place. Re-running it produces byte-identical files.
#
# The dataset is deliberately smaller than the first proposal in
# `.opencode/plan/performance-testing-plan.md`: the committed SQL and the added
# time per container start are budgeted (§ 9.4), and the larger preset missed
# both. The volumes below are the measured compromise; change a constant here,
# re-run the script, and everything that derives an identifier moves with it.
#
# Identifiers are arithmetic, never random:
#
#   image file/media   1_000_000 / 2_000_000
#   audio file/media   1_100_000 / 2_100_000
#   video file/media   1_200_000 / 2_200_000
#   gallery/item       3_000_000 / 3_100_000
#   album/medium       4_000_000 / 4_100_000
#   recording/track    4_200_000 / 4_300_000
#   music group        4_900_000
#   movie/series       5_000_000 / 6_000_000
#   season/episode     6_100_000 / 6_200_000
#
# The `file` rows carry the size and modification instant of the hardlink base
# the Rust seeder writes under `media/perf/.base/`. Both sides name the same
# constants: `PERF_IMAGE_BYTES`, `PERF_VIDEO_BYTES`, `PERF_AUDIO_BYTES`,
# `PERF_STREAM_BYTES` and `PERF_MODIFIED_AT` in
# `src/lib/infrastructure/seed.rs`. A mismatch makes the startup scan re-probe
# every bulk file, which the seed unit test catches.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
readonly OUTPUT_DIR="${REPO_ROOT}/test/seed/sql"

# Volumes. Every one is a lever: shrink here, re-run, commit.
readonly GALLERIES=1000
readonly GALLERY_ITEMS_PER_GALLERY=5
readonly SPARE_IMAGES=2048
readonly ALBUMS=500
readonly TRACKS_PER_ALBUM=10
readonly PLAYLISTS=100
readonly PLAYLIST_TRACKS=5
readonly MOVIES=1000
readonly SERIES=200
readonly SEASONS_PER_SERIES=3
readonly EPISODES_PER_SEASON=10

# Identifier bases (shared verbatim with src/lib/infrastructure/seed.rs).
readonly IMAGE_FILE_BASE=1000000
readonly AUDIO_FILE_BASE=1100000
readonly VIDEO_FILE_BASE=1200000
readonly IMAGE_MEDIA_BASE=2000000
readonly AUDIO_MEDIA_BASE=2100000
readonly VIDEO_MEDIA_BASE=2200000
readonly GALLERY_BASE=3000000
readonly GALLERY_ITEM_BASE=3100000
readonly ALBUM_BASE=4000000
readonly MEDIUM_BASE=4100000
readonly RECORDING_BASE=4200000
readonly TRACK_BASE=4300000
readonly PLAYLIST_BASE=4400000
readonly PLAYLIST_TRACK_BASE=4500000
readonly MUSIC_GROUP_ID=4900000
readonly MOVIE_BASE=5000000
readonly SERIES_BASE=6000000
readonly SEASON_BASE=6100000
readonly EPISODE_BASE=6200000

# Resource identifier bases; kept clear of the small singleton ids 1-7.
readonly GALLERY_RES_BASE=10000000
readonly GALLERY_ITEM_RES_BASE=10100000
readonly IMAGE_RES_BASE=10200000
readonly IMAGE_FILE_RES_BASE=10300000
readonly ALBUM_RES_BASE=10400000
readonly MEDIUM_RES_BASE=10500000
readonly RECORDING_RES_BASE=10600000
readonly AUDIO_RES_BASE=10700000
readonly AUDIO_FILE_RES_BASE=10800000
readonly MUSIC_GROUP_RES_BASE=10900000
readonly PLAYLIST_RES_BASE=10950000
readonly MOVIE_RES_BASE=11000000
readonly MOVIE_VIDEO_RES_BASE=11100000
readonly MOVIE_FILE_RES_BASE=11200000
readonly SERIES_RES_BASE=11300000
readonly SEASON_RES_BASE=11400000
readonly EPISODE_RES_BASE=11500000
readonly EPISODE_VIDEO_RES_BASE=11600000
readonly EPISODE_FILE_RES_BASE=11700000

# Base sizes and the shared modification instant, mirrored in seed.rs.
readonly IMAGE_BYTES=1024
readonly VIDEO_BYTES=2048
readonly AUDIO_BYTES=4096
readonly STREAM_BYTES=4194304
readonly MODIFIED_AT="2020-01-01 00:00:00"
readonly UPLOADED_AT="2020-01-01"
readonly OWNER_ID=9001

readonly ITEM_IMAGES=$((GALLERIES * GALLERY_ITEMS_PER_GALLERY))
readonly TOTAL_IMAGES=$((ITEM_IMAGES + SPARE_IMAGES))
readonly TOTAL_TRACKS=$((ALBUMS * TRACKS_PER_ALBUM))
readonly TOTAL_SEASONS=$((SERIES * SEASONS_PER_SERIES))
readonly TOTAL_EPISODES=$((TOTAL_SEASONS * EPISODES_PER_SEASON))
readonly TOTAL_VIDEOS=$((MOVIES + TOTAL_EPISODES))

readonly COMMON_ARGS=(
	-v galleries="${GALLERIES}"
	-v items_per_gallery="${GALLERY_ITEMS_PER_GALLERY}"
	-v spare_images="${SPARE_IMAGES}"
	-v albums="${ALBUMS}"
	-v tracks_per_album="${TRACKS_PER_ALBUM}"
	-v playlists="${PLAYLISTS}"
	-v playlist_tracks="${PLAYLIST_TRACKS}"
	-v movies="${MOVIES}"
	-v series="${SERIES}"
	-v seasons_per_series="${SEASONS_PER_SERIES}"
	-v episodes_per_season="${EPISODES_PER_SEASON}"
	-v image_file_base="${IMAGE_FILE_BASE}"
	-v audio_file_base="${AUDIO_FILE_BASE}"
	-v video_file_base="${VIDEO_FILE_BASE}"
	-v image_media_base="${IMAGE_MEDIA_BASE}"
	-v audio_media_base="${AUDIO_MEDIA_BASE}"
	-v video_media_base="${VIDEO_MEDIA_BASE}"
	-v gallery_base="${GALLERY_BASE}"
	-v gallery_item_base="${GALLERY_ITEM_BASE}"
	-v album_base="${ALBUM_BASE}"
	-v medium_base="${MEDIUM_BASE}"
	-v recording_base="${RECORDING_BASE}"
	-v track_base="${TRACK_BASE}"
	-v playlist_base="${PLAYLIST_BASE}"
	-v playlist_track_base="${PLAYLIST_TRACK_BASE}"
	-v music_group_id="${MUSIC_GROUP_ID}"
	-v movie_base="${MOVIE_BASE}"
	-v series_base="${SERIES_BASE}"
	-v season_base="${SEASON_BASE}"
	-v episode_base="${EPISODE_BASE}"
	-v gallery_res_base="${GALLERY_RES_BASE}"
	-v gallery_item_res_base="${GALLERY_ITEM_RES_BASE}"
	-v image_res_base="${IMAGE_RES_BASE}"
	-v image_file_res_base="${IMAGE_FILE_RES_BASE}"
	-v album_res_base="${ALBUM_RES_BASE}"
	-v medium_res_base="${MEDIUM_RES_BASE}"
	-v recording_res_base="${RECORDING_RES_BASE}"
	-v audio_res_base="${AUDIO_RES_BASE}"
	-v audio_file_res_base="${AUDIO_FILE_RES_BASE}"
	-v music_group_res_base="${MUSIC_GROUP_RES_BASE}"
	-v playlist_res_base="${PLAYLIST_RES_BASE}"
	-v movie_res_base="${MOVIE_RES_BASE}"
	-v movie_video_res_base="${MOVIE_VIDEO_RES_BASE}"
	-v movie_file_res_base="${MOVIE_FILE_RES_BASE}"
	-v series_res_base="${SERIES_RES_BASE}"
	-v season_res_base="${SEASON_RES_BASE}"
	-v episode_res_base="${EPISODE_RES_BASE}"
	-v episode_video_res_base="${EPISODE_VIDEO_RES_BASE}"
	-v episode_file_res_base="${EPISODE_FILE_RES_BASE}"
	-v image_bytes="${IMAGE_BYTES}"
	-v video_bytes="${VIDEO_BYTES}"
	-v audio_bytes="${AUDIO_BYTES}"
	-v stream_bytes="${STREAM_BYTES}"
	-v modified_at="${MODIFIED_AT}"
	-v uploaded_at="${UPLOADED_AT}"
	-v owner="${OWNER_ID}"
)

generate_galleries() {
	awk "${COMMON_ARGS[@]}" '
	function open_insert(table, columns) {
		printf "INSERT INTO %s (\n    %s\n)\nVALUES\n", table, columns
	}
	function close_row(last) {
		printf "%s\n", (last ? ";" : ",")
	}
	BEGIN {
		items = galleries * items_per_gallery
		total_images = items + spare_images

		printf "-- Volume galleries for the black-box suites. Generated by\n"
		printf "-- script/generate_perf_seed.sh; local reference preset, owned by\n"
		printf "-- alice (user %d). Every file shares one hardlink base the Rust\n", owner
		printf "-- seeder writes under media/perf/.base/.\n\n"

		open_insert("file", "file_id, path, user_id, is_public, mime_type_id, uploaded_at, size_bytes, modified_at, origin, integrity_hash")
		for (i = 0; i < total_images; i++) {
			id = image_file_base + i
			printf "(%d, '\''perf/image/%d.png'\'', %d, 0, '\''image/png'\'', '\''%s'\'', %d, '\''%s'\'', '\''SCANNED'\'', NULL)", id, id, owner, uploaded_at, image_bytes, modified_at
			close_row(i == total_images - 1)
		}

		open_insert("gallery", "gallery_id, name, created_at, last_modified_at, is_public, user_id")
		for (g = 0; g < galleries; g++) {
			printf "(%d, '\''Perf Gallery %d'\'', '\''%s'\'', '\''%s'\'', 1, %d)", gallery_base + g, g, modified_at, modified_at, owner
			close_row(g == galleries - 1)
		}

		open_insert("image", "image_id, name, width_px, height_px, orientation, created_at, file_id")
		for (i = 0; i < total_images; i++) {
			printf "(%d, '\''perf-image-%d.png'\'', 32, 32, '\''SQUARE'\'', '\''%s'\'', %d)", image_media_base + i, i, uploaded_at, image_file_base + i
			close_row(i == total_images - 1)
		}

		open_insert("gallery_item", "gallery_item_id, gallery_id, image_id, video_id, item_index, added_at")
		for (i = 0; i < items; i++) {
			printf "(%d, %d, %d, NULL, %d, '\''%s'\'')", gallery_item_base + i, gallery_base + int(i / items_per_gallery), image_media_base + i, i % items_per_gallery, modified_at
			close_row(i == items - 1)
		}

		open_insert("resource", "resource_id, kind, entity_id, owner_id, auth_parent_id, visibility")
		for (g = 0; g < galleries; g++) {
			printf "(%d, '\''GALLERY'\'', %d, %d, 2, '\''PUBLIC'\''),", gallery_res_base + g, gallery_base + g, owner
			printf "\n"
		}
		for (i = 0; i < items; i++) {
			printf "(%d, '\''GALLERY_ITEM'\'', %d, NULL, %d, '\''PRIVATE'\''),", gallery_item_res_base + i, gallery_item_base + i, gallery_res_base + int(i / items_per_gallery)
			printf "\n"
		}
		for (i = 0; i < total_images; i++) {
			parent = (i < items) ? gallery_item_res_base + i : "NULL"
			printf "(%d, '\''IMAGE'\'', %d, %d, %s, '\''PRIVATE'\''),", image_res_base + i, image_media_base + i, owner, parent
			printf "\n"
		}
		for (i = 0; i < total_images; i++) {
			printf "(%d, '\''FILE'\'', %d, %d, %d, '\''PRIVATE'\'')", image_file_res_base + i, image_file_base + i, owner, image_res_base + i
			close_row(i == total_images - 1)
		}

		printf "\nINSERT INTO grant (\n"
		printf "    principal_kind, principal_id, principal_role, resource_id, role,\n"
		printf "    child_kind, effect\n"
		printf ")\nVALUES\n"
		printf "('\''USER'\'', %d, NULL, %d, '\''DOWNLOADER'\'', '\''ALL'\'', '\''ALLOW'\'');\n", owner, image_file_res_base
	}' >"${OUTPUT_DIR}/0005_library_volume_galleries.sql"
}

generate_music() {
	awk "${COMMON_ARGS[@]}" '
	function open_insert(table, columns) {
		printf "INSERT INTO %s (\n    %s\n)\nVALUES\n", table, columns
	}
	function close_row(last) {
		printf "%s\n", (last ? ";" : ",")
	}
	BEGIN {
		tracks = albums * tracks_per_album

		printf "-- Volume music library for the black-box suites. Generated by\n"
		printf "-- script/generate_perf_seed.sh; local reference preset, owned by\n"
		printf "-- alice (user %d), no covers. Every audio file shares one hardlink\n", owner
		printf "-- base the Rust seeder writes under media/perf/.base/.\n\n"

		open_insert("music_group", "music_group_id, stage_name, cover_image_id")
		printf "(%d, '\''Perf Ensemble'\'', NULL);\n\n", music_group_id

		open_insert("file", "file_id, path, user_id, is_public, mime_type_id, uploaded_at, size_bytes, modified_at, origin, integrity_hash")
		for (t = 0; t < tracks; t++) {
			id = audio_file_base + t
			printf "(%d, '\''perf/audio/%d.flac'\'', %d, 0, '\''audio/flac'\'', '\''%s'\'', %d, '\''%s'\'', '\''SCANNED'\'', NULL)", id, id, owner, uploaded_at, audio_bytes, modified_at
			close_row(t == tracks - 1)
		}

		open_insert("audio", "audio_id, name, duration_ms, codec, sample_rate_hz, bit_depth, block_size, file_id")
		for (t = 0; t < tracks; t++) {
			printf "(%d, '\''perf-track-%d.flac'\'', 500.0, '\''FLAC'\'', 44100.0, 16, NULL, %d)", audio_media_base + t, t, audio_file_base + t
			close_row(t == tracks - 1)
		}

		open_insert("music_album", "music_album_id, name, release_date, production_type, cover_image_id, music_group_id")
		for (a = 0; a < albums; a++) {
			printf "(%d, '\''Perf Album %d'\'', '\''%s'\'', '\''STUDIO'\'', NULL, %d)", album_base + a, a, uploaded_at, music_group_id
			close_row(a == albums - 1)
		}

		open_insert("music_album_group", "music_album_id, music_group_id")
		for (a = 0; a < albums; a++) {
			printf "(%d, %d)", album_base + a, music_group_id
			close_row(a == albums - 1)
		}

		open_insert("music_medium", "music_medium_id, music_album_id, type, medium_index")
		for (a = 0; a < albums; a++) {
			printf "(%d, %d, '\''DIGITAL'\'', 1)", medium_base + a, album_base + a
			close_row(a == albums - 1)
		}

		open_insert("music_recording", "music_recording_id, name, isrc_code, audio_id")
		for (t = 0; t < tracks; t++) {
			printf "(%d, '\''Perf Track %d'\'', NULL, %d)", recording_base + t, t, audio_media_base + t
			close_row(t == tracks - 1)
		}

		open_insert("music_track", "music_track_id, track_index, music_playlist_id, music_medium_id, music_recording_id")
		for (t = 0; t < tracks; t++) {
			printf "(%d, %d, NULL, %d, %d)", track_base + t, t % tracks_per_album, medium_base + int(t / tracks_per_album), recording_base + t
			close_row(t == tracks - 1)
		}

		open_insert("music_playlist", "music_playlist_id, name, created_at, last_modified_at, user_id, is_public, cover_image_id")
		for (p = 0; p < playlists; p++) {
			printf "(%d, '\''Perf Playlist %d'\'', '\''%s'\'', '\''%s'\'', %d, 1, NULL)", playlist_base + p, p, modified_at, modified_at, owner
			close_row(p == playlists - 1)
		}

		open_insert("music_track", "music_track_id, track_index, music_playlist_id, music_medium_id, music_recording_id")
		for (p = 0; p < playlists; p++) {
			for (k = 0; k < playlist_tracks; k++) {
				idx = p * playlist_tracks + k
				printf "(%d, %d, %d, NULL, %d)", playlist_track_base + idx, k, playlist_base + p, recording_base + (idx % tracks)
				close_row(idx == playlists * playlist_tracks - 1)
			}
		}

		open_insert("resource", "resource_id, kind, entity_id, owner_id, auth_parent_id, visibility")
		printf "(%d, '\''MUSIC_GROUP'\'', %d, NULL, 3, '\''PUBLIC'\''),\n", music_group_res_base, music_group_id
		for (p = 0; p < playlists; p++) {
			printf "(%d, '\''MUSIC_PLAYLIST'\'', %d, %d, 2, '\''PUBLIC'\''),\n", playlist_res_base + p, playlist_base + p, owner
		}
		for (a = 0; a < albums; a++) {
			printf "(%d, '\''MUSIC_ALBUM'\'', %d, NULL, 3, '\''PUBLIC'\''),\n", album_res_base + a, album_base + a
		}
		for (a = 0; a < albums; a++) {
			printf "(%d, '\''MUSIC_MEDIUM'\'', %d, NULL, %d, '\''PUBLIC'\''),\n", medium_res_base + a, medium_base + a, album_res_base + a
		}
		for (t = 0; t < tracks; t++) {
			printf "(%d, '\''MUSIC_RECORDING'\'', %d, NULL, %d, '\''PUBLIC'\''),\n", recording_res_base + t, recording_base + t, medium_res_base + int(t / tracks_per_album)
		}
		for (t = 0; t < tracks; t++) {
			printf "(%d, '\''AUDIO'\'', %d, %d, %d, '\''PRIVATE'\''),\n", audio_res_base + t, audio_media_base + t, owner, recording_res_base + t
		}
		for (t = 0; t < tracks; t++) {
			printf "(%d, '\''FILE'\'', %d, %d, %d, '\''PRIVATE'\'')", audio_file_res_base + t, audio_file_base + t, owner, audio_res_base + t
			close_row(t == tracks - 1)
		}

		open_insert("resource_relation", "from_resource_id, to_resource_id, kind, position")
		for (a = 0; a < albums; a++) {
			printf "(%d, %d, '\''GROUP_MEMBER'\'', NULL),", music_group_res_base, album_res_base + a
			printf "\n"
		}
		for (t = 0; t < tracks; t++) {
			printf "(%d, %d, '\''ALBUM_MEMBER'\'', %d)", album_res_base + int(t / tracks_per_album), recording_res_base + t, t % tracks_per_album
			close_row(t == tracks - 1)
		}
	}' >"${OUTPUT_DIR}/0006_library_volume_music.sql"
}

generate_video() {
	awk "${COMMON_ARGS[@]}" '
	function open_insert(table, columns) {
		printf "INSERT INTO %s (\n    %s\n)\nVALUES\n", table, columns
	}
	function close_row(last) {
		printf "%s\n", (last ? ";" : ",")
	}
	BEGIN {
		seasons = series * seasons_per_series
		episodes = seasons * episodes_per_season
		videos = movies + episodes

		printf "-- Volume video library for the black-box suites. Generated by\n"
		printf "-- script/generate_perf_seed.sh; local reference preset, owned by\n"
		printf "-- alice (user %d), no covers. Every video file shares one hardlink\n", owner
		printf "-- base except the first film, which backs the multi-megabyte stream\n"
		printf "-- the read-asset flow measures.\n\n"

		open_insert("file", "file_id, path, user_id, is_public, mime_type_id, uploaded_at, size_bytes, modified_at, origin, integrity_hash")
		for (v = 0; v < videos; v++) {
			id = video_file_base + v
			size = (v == 0) ? stream_bytes : video_bytes
			printf "(%d, '\''perf/video/%d.mp4'\'', %d, 0, '\''video/mp4'\'', '\''%s'\'', %d, '\''%s'\'', '\''SCANNED'\'', NULL)", id, id, owner, uploaded_at, size, modified_at
			close_row(v == videos - 1)
		}

		open_insert("video", "video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, scope, file_id")
		for (v = 0; v < videos; v++) {
			printf "(%d, 1000.0, '\''H264'\'', 24, 1920, 1080, '\''YCbCr'\'', '\''PROGRESSIVE'\'', '\''LIBRARY'\'', %d)", video_media_base + v, video_file_base + v
			close_row(v == videos - 1)
		}

		open_insert("movie", "movie_id, name, plot, country_of_origin, release_date, video_id, cover_image_id")
		for (m = 0; m < movies; m++) {
			printf "(%d, '\''Perf Movie %d'\'', '\''A seeded film.'\'', '\''US'\'', '\''%s'\'', %d, NULL)", movie_base + m, m, uploaded_at, video_media_base + m
			close_row(m == movies - 1)
		}

		open_insert("series", "series_id, name, plot, country_of_origin, first_air_date, cover_image_id")
		for (s = 0; s < series; s++) {
			printf "(%d, '\''Perf Show %d'\'', '\''A seeded show.'\'', '\''US'\'', '\''%s'\'', NULL)", series_base + s, s, uploaded_at
			close_row(s == series - 1)
		}

		open_insert("season", "season_id, series_id, season_index, name, cover_image_id")
		for (s = 0; s < series; s++) {
			for (n = 0; n < seasons_per_series; n++) {
				idx = s * seasons_per_series + n
				printf "(%d, %d, %d, '\''Season %d'\'', NULL)", season_base + idx, series_base + s, n + 1, n + 1
				close_row(idx == seasons - 1)
			}
		}

		open_insert("episode", "episode_id, season_id, episode_index, name, plot, air_date, video_id")
		for (e = 0; e < episodes; e++) {
			printf "(%d, %d, %d, '\''Episode %d'\'', '\''A seeded episode.'\'', '\''%s'\'', %d)", episode_base + e, season_base + int(e / episodes_per_season), e % episodes_per_season, e % episodes_per_season, uploaded_at, video_media_base + movies + e
			close_row(e == episodes - 1)
		}

		open_insert("resource", "resource_id, kind, entity_id, owner_id, auth_parent_id, visibility")
		for (m = 0; m < movies; m++) {
			printf "(%d, '\''MOVIE'\'', %d, NULL, 3, '\''PUBLIC'\''),\n", movie_res_base + m, movie_base + m
		}
		for (s = 0; s < series; s++) {
			printf "(%d, '\''SERIES'\'', %d, NULL, 3, '\''PUBLIC'\''),\n", series_res_base + s, series_base + s
		}
		for (n = 0; n < seasons; n++) {
			printf "(%d, '\''SEASON'\'', %d, NULL, %d, '\''PUBLIC'\''),\n", season_res_base + n, season_base + n, series_res_base + int(n / seasons_per_series)
		}
		for (e = 0; e < episodes; e++) {
			printf "(%d, '\''EPISODE'\'', %d, NULL, %d, '\''PUBLIC'\''),\n", episode_res_base + e, episode_base + e, season_res_base + int(e / episodes_per_season)
		}
		for (m = 0; m < movies; m++) {
			printf "(%d, '\''VIDEO'\'', %d, %d, %d, '\''PRIVATE'\''),\n", movie_video_res_base + m, video_media_base + m, owner, movie_res_base + m
		}
		for (e = 0; e < episodes; e++) {
			printf "(%d, '\''VIDEO'\'', %d, %d, %d, '\''PRIVATE'\''),\n", episode_video_res_base + e, video_media_base + movies + e, owner, episode_res_base + e
		}
		for (m = 0; m < movies; m++) {
			printf "(%d, '\''FILE'\'', %d, %d, %d, '\''PRIVATE'\''),\n", movie_file_res_base + m, video_file_base + m, owner, movie_video_res_base + m
		}
		for (e = 0; e < episodes; e++) {
			printf "(%d, '\''FILE'\'', %d, %d, %d, '\''PRIVATE'\'')", episode_file_res_base + e, video_file_base + movies + e, owner, episode_video_res_base + e
			close_row(e == episodes - 1)
		}

		printf "\nINSERT INTO grant (\n"
		printf "    principal_kind, principal_id, principal_role, resource_id, role,\n"
		printf "    child_kind, effect\n"
		printf ")\nVALUES\n"
		printf "('\''USER'\'', %d, NULL, %d, '\''DOWNLOADER'\'', '\''ALL'\'', '\''ALLOW'\'');\n", owner, movie_file_res_base
	}' >"${OUTPUT_DIR}/0007_library_volume_video.sql"
}

generate_galleries
generate_music
generate_video

printf 'Wrote %s/{0005,0006,0007}_*.sql (%s files, %s media rows)\n' \
	"${OUTPUT_DIR}" \
	"$((TOTAL_IMAGES + TOTAL_TRACKS + TOTAL_VIDEOS))" \
	"$((TOTAL_IMAGES + TOTAL_TRACKS + TOTAL_VIDEOS))"
