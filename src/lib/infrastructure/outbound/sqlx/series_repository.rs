use std::collections::HashMap;

use sqlx::{QueryBuilder, Sqlite, Transaction};

use crate::domain::NumericID;
use crate::domain::model::{Episode, EpisodeIndex, Season, SeasonIndex, Series, Video};
use crate::domain::port::{EpisodeFilter, RepositoryError, SeriesFilter, SeriesRepository};

use super::error_mapping::stored_data_invalid;
use super::model::{SeriesRow, parse_asset_scope, parse_scan_type};

use super::query_filter::push_filter;

/// The projection every show read is assembled from: the show, its genres, its
/// seasons, their episodes and the video backing each episode.
const SERIES_PROJECTION: &str = "SELECT
    series.series_id,
    series.name,
    series.plot,
    series.country_of_origin,
    series.first_air_date,
    series.cover_image_id,
    COALESCE((
        SELECT GROUP_CONCAT(genre_id, ',')
        FROM (
            SELECT genre_id
            FROM series_genre
            WHERE series_genre.series_id = series.series_id
            ORDER BY genre_id
        )
    ), '') AS genres,
    season.season_id,
    season.season_index,
    season.name AS season_name,
    season.cover_image_id AS season_cover_image_id,
    episode.episode_index,
    episode.name AS episode_name,
    episode.plot AS episode_plot,
    episode.air_date,
    video.video_id,
    video.duration_ms,
    video.codec,
    video.frame_count,
    video.width,
    video.height,
    video.color_id,
    video.scan_type,
    video.file_id,
    video.scope AS video_scope
FROM series
LEFT JOIN season ON season.series_id = series.series_id
LEFT JOIN episode ON episode.season_id = season.season_id
LEFT JOIN video ON video.video_id = episode.video_id";

/// Repository querying shows, their seasons and their episodes, backed by a
/// `SQLite` transaction.
pub(crate) struct SqlxSeriesRepository<'transaction> {
    /// Transaction the repository reads from.
    transaction: &'transaction mut Transaction<'static, Sqlite>,
}

/// A season being hydrated: its surrogate identifier, domain identity, name,
/// artwork and the episodes read so far.
type HydratedSeason = (
    NumericID,
    SeasonIndex,
    Option<String>,
    Option<NumericID>,
    Vec<Episode>,
);

impl<'transaction> SqlxSeriesRepository<'transaction> {
    /// Create a new repository bound to `transaction`.
    #[must_use]
    pub(crate) fn new(transaction: &'transaction mut Transaction<'static, Sqlite>) -> Self {
        Self { transaction }
    }

    /// Remove every genre link of `series_id` and link `genre_ids` instead.
    ///
    /// The genre vocabulary is open, so a genre the catalogue does not know yet
    /// is inserted before it is linked.
    async fn replace_genres(
        &mut self,
        series_id: NumericID,
        genre_ids: &[String],
    ) -> Result<(), RepositoryError> {
        sqlx::query("DELETE FROM series_genre WHERE series_id = ?")
            .bind(series_id)
            .execute(&mut **self.transaction)
            .await?;
        for genre_id in genre_ids {
            // The genre table uppercases its identifier through a trigger, so the
            // link is written in the stored vocabulary rather than the caller's.
            let stored_genre = genre_id.to_ascii_uppercase();
            sqlx::query("INSERT OR IGNORE INTO genre (genre_id) VALUES (?)")
                .bind(&stored_genre)
                .execute(&mut **self.transaction)
                .await?;
            sqlx::query("INSERT INTO series_genre (series_id, genre_id) VALUES (?1, ?2)")
                .bind(series_id)
                .bind(&stored_genre)
                .execute(&mut **self.transaction)
                .await?;
        }
        Ok(())
    }

    /// Read the episode at `episode_index` of the season at `season_index` of
    /// `series_id`.
    async fn stored_episode(
        &mut self,
        series_id: NumericID,
        season_index: u32,
        episode_index: u32,
    ) -> Result<Option<Episode>, RepositoryError> {
        let stored = self.stored_season(series_id, season_index).await?;
        Ok(stored.and_then(|found| {
            found
                .episodes()
                .iter()
                .find(|episode| episode.index().value() == episode_index)
                .cloned()
        }))
    }

    /// Read the season at `season_index` of `series_id` with its episodes.
    async fn stored_season(
        &mut self,
        series_id: NumericID,
        season_index: u32,
    ) -> Result<Option<Season>, RepositoryError> {
        let found = self
            .search(&SeriesFilter {
                id: Some(series_id),
                ..SeriesFilter::default()
            })
            .await?
            .into_iter()
            .next();
        Ok(found.and_then(|show| {
            show.seasons()
                .iter()
                .find(|season| season.index().value() == season_index)
                .cloned()
        }))
    }
}

impl SeriesRepository for SqlxSeriesRepository<'_> {
    async fn create(&mut self, series: Series) -> Result<Series, RepositoryError> {
        let series_id = sqlx::query_scalar::<_, NumericID>(
            "INSERT INTO series (
                name, plot, country_of_origin, first_air_date, cover_image_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5)
             RETURNING series_id",
        )
        .bind(series.name())
        .bind(series.plot())
        .bind(series.country_of_origin())
        .bind(series.first_air_date())
        .bind(series.cover_image_id())
        .fetch_one(&mut **self.transaction)
        .await?;

        self.replace_genres(series_id, series.genre_ids()).await?;

        self.search(&SeriesFilter {
            id: Some(series_id),
            ..SeriesFilter::default()
        })
        .await?
        .into_iter()
        .next()
        .ok_or(RepositoryError::OperationFailed)
    }

    async fn delete(&mut self, filter: &SeriesFilter) -> Result<bool, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM series");
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "series_id = ", id);
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "name = ", name);
        }

        let result = builder.build().execute(&mut **self.transaction).await?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_episode(&mut self, filter: &EpisodeFilter) -> Result<bool, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new("DELETE FROM episode");
        let mut first = true;

        if let Some(index) = filter.index {
            push_filter(
                &mut first,
                &mut builder,
                "episode_index = ",
                i64::from(index.value()),
            );
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "name = ", name);
        }
        if let Some(video_id) = filter.video_id {
            push_filter(&mut first, &mut builder, "video_id = ", video_id);
        }

        let result = builder.build().execute(&mut **self.transaction).await?;
        Ok(result.rows_affected() > 0)
    }

    async fn save(&mut self, series: Series) -> Result<Series, RepositoryError> {
        sqlx::query(
            "INSERT INTO series (
                series_id, name, plot, country_of_origin, first_air_date, cover_image_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (series_id) DO UPDATE SET
                name = excluded.name,
                plot = excluded.plot,
                country_of_origin = excluded.country_of_origin,
                first_air_date = excluded.first_air_date,
                cover_image_id = excluded.cover_image_id",
        )
        .bind(series.id())
        .bind(series.name())
        .bind(series.plot())
        .bind(series.country_of_origin())
        .bind(series.first_air_date())
        .bind(series.cover_image_id())
        .execute(&mut **self.transaction)
        .await?;

        self.replace_genres(series.id(), series.genre_ids()).await?;

        // `save` writes the whole aggregate: the stored season and episode
        // collections are replaced by the ones the caller holds.
        sqlx::query("DELETE FROM season WHERE series_id = ?")
            .bind(series.id())
            .execute(&mut **self.transaction)
            .await?;

        for season in series.seasons() {
            sqlx::query(
                "INSERT INTO season (series_id, season_index, name, cover_image_id)
                 VALUES (?1, ?2, ?3, ?4)",
            )
            .bind(series.id())
            .bind(i64::from(season.index().value()))
            .bind(season.name())
            .bind(season.cover_image_id())
            .execute(&mut **self.transaction)
            .await?;

            let season_id = sqlx::query_scalar::<_, NumericID>(
                "SELECT season_id FROM season WHERE series_id = ?1 AND season_index = ?2",
            )
            .bind(series.id())
            .bind(i64::from(season.index().value()))
            .fetch_one(&mut **self.transaction)
            .await?;

            for episode in season.episodes() {
                sqlx::query(
                    "INSERT INTO episode (
                        season_id, episode_index, name, plot, air_date, video_id
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .bind(season_id)
                .bind(i64::from(episode.index().value()))
                .bind(episode.name())
                .bind(episode.plot())
                .bind(episode.air_date())
                .bind(episode.video_id())
                .execute(&mut **self.transaction)
                .await?;
            }
        }

        self.search(&SeriesFilter {
            id: Some(series.id()),
            ..SeriesFilter::default()
        })
        .await?
        .into_iter()
        .next()
        .ok_or(RepositoryError::OperationFailed)
    }
    async fn save_episode(
        &mut self,
        series_id: NumericID,
        season_index: u32,
        episode: Episode,
    ) -> Result<Option<Episode>, RepositoryError> {
        let Some(season_id) = sqlx::query_scalar::<_, NumericID>(
            "SELECT season_id FROM season WHERE series_id = ?1 AND season_index = ?2",
        )
        .bind(series_id)
        .bind(i64::from(season_index))
        .fetch_optional(&mut **self.transaction)
        .await?
        else {
            return Ok(None);
        };

        sqlx::query(
            "INSERT INTO episode (
                season_id, episode_index, name, plot, air_date, video_id
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (season_id, episode_index) DO UPDATE SET
                name = excluded.name,
                plot = excluded.plot,
                air_date = excluded.air_date,
                video_id = excluded.video_id",
        )
        .bind(season_id)
        .bind(i64::from(episode.index().value()))
        .bind(episode.name())
        .bind(episode.plot())
        .bind(episode.air_date())
        .bind(episode.video().id())
        .execute(&mut **self.transaction)
        .await?;

        self.stored_episode(series_id, season_index, episode.index().value())
            .await
    }
    async fn save_season(
        &mut self,
        series_id: NumericID,
        season: Season,
    ) -> Result<Option<Season>, RepositoryError> {
        let series_exists =
            sqlx::query_scalar::<_, i64>("SELECT 1 FROM series WHERE series_id = ?")
                .bind(series_id)
                .fetch_optional(&mut **self.transaction)
                .await?
                .is_some();
        if !series_exists {
            return Ok(None);
        }

        let season_index = i64::from(season.index().value());
        sqlx::query(
            "INSERT INTO season (series_id, season_index, name, cover_image_id)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (series_id, season_index) DO UPDATE SET
                name = excluded.name,
                cover_image_id = excluded.cover_image_id",
        )
        .bind(series_id)
        .bind(season_index)
        .bind(season.name())
        .bind(season.cover_image_id())
        .execute(&mut **self.transaction)
        .await?;

        self.stored_season(series_id, season.index().value()).await
    }

    async fn search(&mut self, filter: &SeriesFilter) -> Result<Vec<Series>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(SERIES_PROJECTION);
        let mut first = true;

        if let Some(id) = filter.id {
            push_filter(&mut first, &mut builder, "series.series_id = ", id);
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "series.name = ", name);
        }

        // The show, its seasons and their episodes make one aggregate read, so
        // the order fixes both the season collection order and each season's
        // episode order. A stable show identifier also makes the caller's
        // offset/limit pagination deterministic (`ListSeries` pages in memory).
        builder.push(" ORDER BY series.series_id, season.season_index, episode.episode_index");

        let rows = builder
            .build_query_as::<SeriesRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        // The rows repeat a show once per episode and arrive ordered by show,
        // then season index, then episode index, so one pass groups them
        // without re-sorting. The season surrogate identifier keys the group;
        // the domain identity is the validated `SeasonIndex`.
        let mut seasons: HashMap<NumericID, Vec<HydratedSeason>> = HashMap::new();
        let mut series_ids: Vec<NumericID> = Vec::new();

        for row in &rows {
            if series_ids.last() != Some(&row.series_id) {
                series_ids.push(row.series_id);
            }
            let Some(season_id) = row.season_id else {
                continue;
            };
            let season_index_raw = row
                .season_index
                .ok_or_else(|| stored_data_invalid("missing_season_index"))?;
            let season_index = u32::try_from(season_index_raw)
                .map_err(|_| stored_data_invalid("season_index_out_of_range"))?;
            let series_seasons = seasons.entry(row.series_id).or_default();
            if series_seasons.last().map(|season| season.0) != Some(season_id) {
                series_seasons.push((
                    season_id,
                    SeasonIndex::new(season_index),
                    row.season_name.clone(),
                    row.season_cover_image_id,
                    Vec::new(),
                ));
            }
            let Some(episode) = episode_from_row(row)? else {
                continue;
            };
            if let Some(season) = series_seasons.last_mut() {
                season.4.push(episode);
            }
        }

        let mut result = Vec::with_capacity(series_ids.len());
        for series_id in series_ids {
            let row = rows
                .iter()
                .find(|row| row.series_id == series_id)
                .ok_or_else(|| stored_data_invalid("missing_series_row"))?;
            let series_seasons = seasons
                .remove(&series_id)
                .unwrap_or_default()
                .into_iter()
                .map(|(_, index, name, cover_image_id, episodes)| {
                    Season::new(index, name, cover_image_id, episodes)
                })
                .collect();
            result.push(
                Series::try_new(
                    series_id,
                    row.name.clone(),
                    row.plot.clone(),
                    row.country_of_origin.clone(),
                    row.first_air_date,
                    row.genres
                        .split(',')
                        .filter(|genre| !genre.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    row.cover_image_id,
                    series_seasons,
                )
                .map_err(|_| stored_data_invalid("invalid_series"))?,
            );
        }
        Ok(result)
    }

    async fn search_episodes(
        &mut self,
        filter: &EpisodeFilter,
    ) -> Result<Vec<Episode>, RepositoryError> {
        let mut builder = QueryBuilder::<Sqlite>::new(SERIES_PROJECTION);
        let mut first = true;

        if let Some(index) = filter.index {
            push_filter(
                &mut first,
                &mut builder,
                "episode.episode_index = ",
                i64::from(index.value()),
            );
        }
        if let Some(name) = filter.name.as_deref() {
            push_filter(&mut first, &mut builder, "episode.name = ", name);
        }
        if let Some(video_id) = filter.video_id {
            push_filter(&mut first, &mut builder, "episode.video_id = ", video_id);
        }

        builder.push(" ORDER BY series.series_id, season.season_index, episode.episode_index");

        let rows = builder
            .build_query_as::<SeriesRow>()
            .fetch_all(&mut **self.transaction)
            .await?;

        rows.iter()
            .filter_map(|row| episode_from_row(row).transpose())
            .collect()
    }
}

/// Build the episode of `row`, or `None` when the row carries no episode.
fn episode_from_row(row: &SeriesRow) -> Result<Option<Episode>, RepositoryError> {
    let Some(episode_index_raw) = row.episode_index else {
        return Ok(None);
    };
    let episode_index = u32::try_from(episode_index_raw)
        .map_err(|_| stored_data_invalid("episode_index_out_of_range"))?;
    let scan_type_raw = row
        .scan_type
        .clone()
        .ok_or_else(|| stored_data_invalid("missing_scan_type"))?;
    let scan_type =
        parse_scan_type(&scan_type_raw).ok_or_else(|| stored_data_invalid("unknown_scan_type"))?;
    let scope = row
        .video_scope
        .as_deref()
        .map(|value| parse_asset_scope(value).ok_or_else(|| stored_data_invalid("unknown_scope")))
        .transpose()?;
    let video = Video::try_new(
        row.video_id
            .ok_or_else(|| stored_data_invalid("missing_video_id"))?,
        row.duration_ms
            .ok_or_else(|| stored_data_invalid("missing_duration_ms"))?,
        row.codec
            .clone()
            .ok_or_else(|| stored_data_invalid("missing_codec"))?,
        row.frame_count
            .ok_or_else(|| stored_data_invalid("missing_frame_count"))?,
        row.width
            .ok_or_else(|| stored_data_invalid("missing_width"))?,
        row.height
            .ok_or_else(|| stored_data_invalid("missing_height"))?,
        row.color_id
            .clone()
            .ok_or_else(|| stored_data_invalid("missing_color_id"))?,
        scan_type,
        row.file_id
            .ok_or_else(|| stored_data_invalid("missing_file_id"))?,
        scope,
    )
    .map_err(|_| stored_data_invalid("invalid_video"))?;
    Episode::try_new(
        EpisodeIndex::new(episode_index),
        row.episode_name
            .clone()
            .ok_or_else(|| stored_data_invalid("missing_episode_name"))?,
        row.episode_plot
            .clone()
            .ok_or_else(|| stored_data_invalid("missing_episode_plot"))?,
        row.air_date,
        video,
    )
    .map(Some)
    .map_err(|_| stored_data_invalid("invalid_episode"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::ScanType;

    use crate::infrastructure::outbound::sqlx::test_helpers::begin_transaction;
    use crate::test_helpers::*;
    use chrono::NaiveDate;
    use std::error::Error;
    use tracing_test::traced_test;

    /// Assert the captured output is exactly one `output_validation_failed`
    /// event with `reason`, carrying neither a stored value nor an `sqlx`
    /// detail.
    fn assert_output_validation_failed(lines: &[&str], reason: &str) -> Result<(), String> {
        let ours: Vec<&str> = lines
            .iter()
            .copied()
            .filter(|line| line.contains("security:"))
            .collect();
        let warn_count = ours.iter().filter(|line| line.contains("WARN")).count();
        if warn_count != 1 {
            return Err(format!(
                "rejected stored data must emit exactly one warn event (found {warn_count})"
            ));
        }
        if !ours
            .iter()
            .any(|line| line.contains("event=\"output_validation_failed\""))
        {
            return Err("the event must be output_validation_failed".to_owned());
        }
        if !ours
            .iter()
            .any(|line| line.contains(&format!("reason=\"{reason}\"")))
        {
            return Err(format!("the event must carry the stable reason `{reason}`"));
        }
        if ours.iter().any(|line| line.contains("sqlx")) {
            return Err("the event must not carry the sqlx detail".to_owned());
        }
        Ok(())
    }

    /// Open a migrated in-memory database and begin one transaction.
    /// Seed a credential and a standard user so foreign keys resolve.
    async fn seed_user(transaction: &mut Transaction<'static, Sqlite>) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO credential (credential_id, password_hash) VALUES (1, 'hash')")
            .execute(&mut **transaction)
            .await?;
        sqlx::query(
            "INSERT INTO user (user_id, role, username, email, credential_id)
             VALUES (1, 'STANDARD', 'tester', NULL, 1)",
        )
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a video file owned by the seeded user.
    async fn seed_video_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, 1, 0, 'video/mp4', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?3)",
        )
        .bind(file_id)
        .bind(format!("{file_id}.mp4"))
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a progressive video of `width` pixels on `file_id`.
    async fn seed_video(
        transaction: &mut Transaction<'static, Sqlite>,
        video_id: i64,
        file_id: i64,
        width: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO video (
                video_id, duration_ms, codec, frame_count, width, height, color_id, scan_type, file_id
             )
             VALUES (?1, 1000.0, 'H264', 30, ?2, 10, 'YCbCr', 'PROGRESSIVE', ?3)",
        )
        .bind(video_id)
        .bind(width)
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed an image row of `image_id` on `file_id`, in a square orientation.
    async fn seed_image(
        transaction: &mut Transaction<'static, Sqlite>,
        image_id: i64,
        file_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO image (image_id, name, width_px, height_px, orientation, created_at, file_id)
             VALUES (?1, ?2, 10, 10, 'SQUARE', '2026-10-06', ?3)",
        )
        .bind(image_id)
        .bind(format!("{image_id}.png"))
        .bind(file_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed an image file of `file_id`.
    async fn seed_image_file(
        transaction: &mut Transaction<'static, Sqlite>,
        file_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (?1, ?2, 1, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?3)",
        )
        .bind(file_id)
        .bind(format!("{file_id}.png"))
        .bind(format!("{file_id:064x}"))
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a show with a lowercase country of origin.
    async fn seed_series(
        transaction: &mut Transaction<'static, Sqlite>,
        series_id: i64,
        name: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO series (
                series_id, name, plot, country_of_origin, first_air_date
             )
             VALUES (?1, ?2, 'A plot', 'us', '1990-04-08')",
        )
        .bind(series_id)
        .bind(name)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed a genre.
    async fn seed_genre(
        transaction: &mut Transaction<'static, Sqlite>,
        genre_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO genre (genre_id) VALUES (?1)")
            .bind(genre_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    /// Attach `genre_id` to `series_id`.
    async fn seed_series_genre(
        transaction: &mut Transaction<'static, Sqlite>,
        series_id: i64,
        genre_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO series_genre (series_id, genre_id) VALUES (?1, ?2)")
            .bind(series_id)
            .bind(genre_id)
            .execute(&mut **transaction)
            .await?;
        Ok(())
    }

    /// Seed a season of `series_id` at `season_index`.
    async fn seed_season(
        transaction: &mut Transaction<'static, Sqlite>,
        season_id: i64,
        series_id: i64,
        season_index: i64,
        name: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO season (season_id, series_id, season_index, name)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(season_id)
        .bind(series_id)
        .bind(season_index)
        .bind(name)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Seed an episode of `season_id` at `episode_index`, backed by `video_id`.
    async fn seed_episode(
        transaction: &mut Transaction<'static, Sqlite>,
        episode_id: i64,
        season_id: i64,
        episode_index: i64,
        name: &str,
        video_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO episode (
                episode_id, season_id, episode_index, name, plot, air_date, video_id
             )
             VALUES (?1, ?2, ?3, ?4, 'Episode plot', '1991-04-08', ?5)",
        )
        .bind(episode_id)
        .bind(season_id)
        .bind(episode_index)
        .bind(name)
        .bind(video_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    /// Re-read the show identified by `id`.
    async fn load_series(
        repository: &mut SqlxSeriesRepository<'_>,
        id: i64,
    ) -> Result<Series, Box<dyn Error>> {
        repository
            .search(&SeriesFilter {
                id: Some(id),
                ..SeriesFilter::default()
            })
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| "the show must still exist".into())
    }

    /// Rebuild `series` with new metadata, keeping its seasons.
    fn remodel(
        series: &Series,
        name: &str,
        plot: &str,
        country_of_origin: Option<&str>,
        first_air_date: Option<NaiveDate>,
        genre_ids: Vec<String>,
    ) -> Result<Series, Box<dyn Error>> {
        Ok(Series::try_new(
            series.id(),
            name.to_owned(),
            plot.to_owned(),
            country_of_origin.map(str::to_owned),
            first_air_date,
            genre_ids,
            series.cover_image_id(),
            series.seasons().to_vec(),
        )?)
    }

    #[tokio::test]
    async fn search_reports_absent_dates_and_country_as_none() -> Result<(), Box<dyn Error>> {
        // Arrange: a show storing neither a country nor a first-air date, and an
        // episode storing no air date.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        sqlx::query(
            "INSERT INTO series (series_id, name, plot) VALUES (1, 'Twin Peaks', 'A plot')",
        )
        .execute(&mut *transaction)
        .await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        sqlx::query(
            "INSERT INTO episode (episode_id, season_id, episode_index, name, plot, video_id)
             VALUES (60, 40, 1, 'Pilot', 'A plot', 20)",
        )
        .execute(&mut *transaction)
        .await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        let show = series.first().ok_or("the show must be found")?;
        assert_eq!(
            show.first_air_date(),
            None,
            "a stored null must read as none"
        );
        assert_eq!(
            show.country_of_origin(),
            None,
            "a stored null must read as none"
        );
        let air_date = show
            .seasons()
            .first()
            .and_then(|season| season.episodes().first())
            .and_then(Episode::air_date);
        assert_eq!(air_date, None, "a stored null must read as none");
        Ok(())
    }

    #[tokio::test]
    async fn insert_rejects_a_video_already_backing_a_film() -> Result<(), Box<dyn Error>> {
        // Arrange: film 1 already backs video 20.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        sqlx::query(
            "INSERT INTO movie (movie_id, name, plot, video_id)
             VALUES (1, 'Some Like It Hot', 'A plot', 20)",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let result = sqlx::query(
            "INSERT INTO episode (episode_id, season_id, episode_index, name, plot, video_id)
             VALUES (60, 40, 1, 'Pilot', 'A plot', 20)",
        )
        .execute(&mut *transaction)
        .await
        .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "a video backing a film must be rejected as a conflict, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn update_rejects_a_video_already_backing_a_film() -> Result<(), Box<dyn Error>> {
        // Arrange: film 1 backs video 20 and episode 60 backs video 21.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 21).await?;
        sqlx::query(
            "INSERT INTO movie (movie_id, name, plot, video_id)
             VALUES (1, 'Some Like It Hot', 'A plot', 20)",
        )
        .execute(&mut *transaction)
        .await?;

        // Act
        let result = sqlx::query("UPDATE episode SET video_id = 20 WHERE episode_id = 60")
            .execute(&mut *transaction)
            .await
            .map_err(RepositoryError::from);

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "moving an episode onto a film's video must be rejected as a conflict, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_hydrates_seasons_episodes_and_their_videos() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video_file(&mut transaction, 12).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1280).await?;
        seed_video(&mut transaction, 22, 12, 720).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_series(&mut transaction, 2, "Northern Exposure").await?;
        seed_genre(&mut transaction, "DRAMA").await?;
        seed_genre(&mut transaction, "MYSTERY").await?;
        seed_series_genre(&mut transaction, 1, "MYSTERY").await?;
        seed_series_genre(&mut transaction, 1, "DRAMA").await?;
        seed_season(&mut transaction, 40, 1, 1, Some("Season One")).await?;
        seed_season(&mut transaction, 41, 1, 2, None).await?;
        // Insert the second episode first so the order can only come from the
        // episode index, not the storage order.
        seed_episode(
            &mut transaction,
            60,
            40,
            2,
            "Zen, or the Skill to Catch a Killer",
            21,
        )
        .await?;
        seed_episode(&mut transaction, 61, 40, 1, "Pilot", 20).await?;
        seed_episode(&mut transaction, 62, 41, 1, "May the Giant Be with You", 22).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        assert_eq!(series.len(), 2, "every show must be returned");
        let first = series.first().ok_or("the first show must be found")?;
        assert_eq!(first.id(), 1);
        assert_eq!(first.name(), "Twin Peaks");
        assert_eq!(first.plot(), "A plot");
        assert_eq!(
            first.country_of_origin(),
            Some("US"),
            "the insert trigger must uppercase the country of origin"
        );
        assert_eq!(
            first.first_air_date(),
            Some(NaiveDate::from_ymd_opt(1990, 4, 8).unwrap_or_default())
        );
        assert_eq!(
            first.genre_ids(),
            ["DRAMA", "MYSTERY"],
            "the genres must be ordered by identifier"
        );
        let seasons: Vec<_> = first.seasons().iter().map(Season::index).collect();
        assert_eq!(
            seasons
                .iter()
                .copied()
                .map(SeasonIndex::value)
                .collect::<Vec<_>>(),
            vec![1, 2],
            "the seasons must be ordered by index"
        );
        let first_season = first
            .seasons()
            .first()
            .ok_or("the first season must be found")?;
        assert_eq!(first_season.name(), Some("Season One"));
        let episode_indexes: Vec<_> = first_season
            .episodes()
            .iter()
            .map(Episode::index)
            .map(EpisodeIndex::value)
            .collect();
        assert_eq!(
            episode_indexes,
            vec![1, 2],
            "the episodes of a season must be ordered by index"
        );
        let pilot = first_season
            .episodes()
            .first()
            .ok_or("the first episode must be found")?;
        assert_eq!(pilot.name(), "Pilot");
        assert_eq!(pilot.plot(), "Episode plot");
        assert_eq!(
            pilot.air_date(),
            Some(NaiveDate::from_ymd_opt(1991, 4, 8).unwrap_or_default())
        );
        assert_eq!(pilot.video().id(), 20);
        assert_eq!(pilot.video().file_id(), 10);
        assert_eq!(pilot.video().codec(), "H264");
        assert_eq!(pilot.video().width(), 1920);
        assert_eq!(pilot.video().height(), 10);
        assert_eq!(pilot.video().scan_type(), ScanType::Progressive);
        assert!((pilot.video().duration_ms() - 1000.0).abs() < f64::EPSILON);
        let second_season = first
            .seasons()
            .get(1)
            .ok_or("the second season must be found")?;
        assert_eq!(second_season.name(), None);
        assert_eq!(second_season.episodes().len(), 1);
        let second = series.get(1).ok_or("the second show must be found")?;
        assert_eq!(second.name(), "Northern Exposure");
        assert!(
            second.seasons().is_empty(),
            "a show without a season must report none"
        );
        assert!(second.genre_ids().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn search_projects_the_show_and_season_covers() -> Result<(), Box<dyn Error>> {
        // Arrange: the show carries artwork, its first season its own, and the
        // second season none.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        sqlx::query(
            "INSERT INTO file (file_id, path, user_id, is_public, mime_type_id, size_bytes, modified_at, origin, integrity_hash)
             VALUES (12, '12.png', 1, 0, 'image/png', 1024, '2026-10-06 00:00:00', 'UPLOADED', ?1)",
        )
        .bind(format!("{:064x}", 12i64))
        .execute(&mut *transaction)
        .await?;
        seed_image(&mut transaction, 12, 12).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        seed_season(&mut transaction, 41, 1, 2, None).await?;
        sqlx::query("UPDATE series SET cover_image_id = 12 WHERE series_id = 1")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE season SET cover_image_id = 12 WHERE season_id = 40")
            .execute(&mut *transaction)
            .await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        let show = series.first().ok_or("the show must be found")?;
        assert_eq!(show.cover_image_id(), Some(12));
        let first_season = show
            .seasons()
            .first()
            .ok_or("the first season must be found")?;
        assert_eq!(first_season.cover_image_id(), Some(12));
        let second_season = show
            .seasons()
            .get(1)
            .ok_or("the second season must be found")?;
        assert_eq!(
            second_season.cover_image_id(),
            None,
            "a season must not inherit its show's cover"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_a_season_that_holds_no_episode() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        let seasons = series
            .first()
            .map(Series::seasons)
            .ok_or("the show must be hydrated")?;
        assert_eq!(seasons.len(), 1);
        let season = seasons.first().ok_or("the season must be hydrated")?;
        assert!(
            season.episodes().is_empty(),
            "a season without an episode holds none"
        );
        Ok(())
    }

    #[tokio::test]
    async fn search_filters_by_name_and_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_series(&mut transaction, 2, "Northern Exposure").await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let by_name = repository
            .search(&SeriesFilter {
                name: Some("Northern Exposure".to_owned()),
                ..SeriesFilter::default()
            })
            .await?;
        let by_id = repository
            .search(&SeriesFilter {
                id: Some(1),
                ..SeriesFilter::default()
            })
            .await?;

        // Assert
        assert_eq!(by_name.len(), 1);
        assert_eq!(by_name.first().map(Series::id), Some(2));
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id.first().map(Series::name), Some("Twin Peaks"));
        Ok(())
    }

    #[tokio::test]
    async fn search_returns_series_ordered_by_identifier() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 3, "Third").await?;
        seed_series(&mut transaction, 1, "First").await?;
        seed_series(&mut transaction, 2, "Second").await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        let ids: Vec<_> = series.iter().map(Series::id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
        Ok(())
    }

    #[tokio::test]
    async fn search_addresses_the_largest_u32_episode_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the largest index the schema and the domain both admit.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 0, None).await?;
        seed_episode(&mut transaction, 60, 40, 4_294_967_295, "The Last", 20).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let series = repository.search(&SeriesFilter::default()).await?;

        // Assert
        let index = series
            .first()
            .and_then(|show| show.seasons().first())
            .and_then(|season| season.episodes().first())
            .map(Episode::index)
            .map(EpisodeIndex::value);
        assert_eq!(index, Some(4_294_967_295));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_an_out_of_range_season_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the range check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_season(&mut transaction, 40, 1, 4_294_967_296, None).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let result = repository.search(&SeriesFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "season_index_out_of_range"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_an_out_of_range_episode_index() -> Result<(), Box<dyn Error>> {
        // Arrange: the range check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 0, None).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_episode(&mut transaction, 60, 40, 4_294_967_296, "Too Far", 20).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let result = repository.search(&SeriesFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "episode_index_out_of_range"));
        Ok(())
    }

    #[tokio::test]
    #[traced_test]
    async fn search_rejects_a_stored_video_the_domain_cannot_hold() -> Result<(), Box<dyn Error>> {
        // Arrange: the width check is probed by relaxing it for this seed.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 0, None).await?;
        sqlx::query("PRAGMA ignore_check_constraints = ON")
            .execute(&mut *transaction)
            .await?;
        seed_video(&mut transaction, 20, 10, 0).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 20).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let result = repository.search(&SeriesFilter::default()).await;

        // Assert
        assert!(matches!(
            result,
            Err(RepositoryError::DataIntegrityViolation)
        ));
        logs_assert(|lines| assert_output_validation_failed(lines, "invalid_video"));
        Ok(())
    }

    #[tokio::test]
    async fn create_stores_the_show_and_genres_with_no_seasons() -> Result<(), Box<dyn Error>> {
        // Arrange: a show with a genre the catalogue does not hold yet.
        let mut transaction = begin_transaction().await?;
        let series = Series::try_new(
            0,
            "Twin Peaks".to_owned(),
            "A plot".to_owned(),
            Some("us".to_owned()),
            Some(NaiveDate::from_ymd_opt(1990, 4, 8).unwrap_or_default()),
            vec!["Mystery".to_owned()],
            None,
            Vec::new(),
        )?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.create(series).await?;

        // Assert
        assert_ne!(stored.id(), 0, "the repository assigns an identity");
        assert_eq!(stored.name(), "Twin Peaks");
        assert_eq!(stored.plot(), "A plot");
        assert_eq!(
            stored.country_of_origin(),
            Some("US"),
            "the insert trigger must uppercase the country of origin"
        );
        assert_eq!(
            stored.first_air_date(),
            Some(NaiveDate::from_ymd_opt(1990, 4, 8).unwrap_or_default())
        );
        assert_eq!(
            stored.genre_ids(),
            ["MYSTERY"],
            "an unknown genre must be stored in the catalogue vocabulary and linked"
        );
        assert!(
            stored.seasons().is_empty(),
            "create must never write a season"
        );
        Ok(())
    }

    #[tokio::test]
    async fn save_replaces_show_metadata_and_genres_and_rewrites_the_aggregate()
    -> Result<(), Box<dyn Error>> {
        // Arrange: show 1 holds season 1 and an episode; the update carries no
        // season at all.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_genre(&mut transaction, "DRAMA").await?;
        seed_series_genre(&mut transaction, 1, "DRAMA").await?;
        seed_season(&mut transaction, 40, 1, 1, Some("Season One")).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 20).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);
        let show = load_series(&mut repository, 1).await?;
        let updated = remodel(
            &show,
            "New Name",
            "New plot",
            Some("GB"),
            Some(NaiveDate::from_ymd_opt(1991, 1, 2).unwrap_or_default()),
            vec!["Comedy".to_owned()],
        )?;

        // Act
        let persisted = repository.save(updated).await?;

        // Assert
        assert_eq!(persisted.name(), "New Name");
        assert_eq!(persisted.plot(), "New plot");
        assert_eq!(persisted.country_of_origin(), Some("GB"));
        assert_eq!(
            persisted.first_air_date(),
            Some(NaiveDate::from_ymd_opt(1991, 1, 2).unwrap_or_default())
        );
        assert_eq!(
            persisted.genre_ids(),
            ["COMEDY"],
            "the genre links must be replaced"
        );
        let season = persisted
            .seasons()
            .first()
            .ok_or("the persisted season must be kept")?;
        assert_eq!(season.name(), Some("Season One"));
        let episode = season
            .episodes()
            .first()
            .ok_or("the persisted episode must be kept")?;
        assert_eq!(episode.name(), "Pilot");
        assert_eq!(
            episode.video().id(),
            20,
            "save must write the episode's aggregate video"
        );
        Ok(())
    }

    #[tokio::test]
    async fn save_inserts_an_absent_show() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let updated = Series::try_new(
            404,
            "New Name".to_owned(),
            "New plot".to_owned(),
            None,
            None,
            Vec::new(),
            None,
            Vec::new(),
        )?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.save(updated).await?;

        // Assert: save upserts, so an unknown identifier is inserted.
        assert_eq!(stored.id(), 404);
        assert_eq!(stored.name(), "New Name");
        Ok(())
    }

    #[tokio::test]
    async fn put_season_creates_the_season() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        let season = Season::new(
            SeasonIndex::new(2),
            Some("Two".to_owned()),
            None,
            Vec::new(),
        );
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.save_season(1, season).await?;

        // Assert
        let persisted = stored.ok_or("the persisted season must be returned")?;
        assert_eq!(persisted.index().value(), 2);
        assert_eq!(persisted.name(), Some("Two"));
        assert!(
            persisted.episodes().is_empty(),
            "a new season must hold no episode"
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_season_replaces_the_season_keeping_its_episodes() -> Result<(), Box<dyn Error>> {
        // Arrange: season 1 already holds an episode.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, Some("Old Name")).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 20).await?;
        let season = Season::new(
            SeasonIndex::new(1),
            Some("New Name".to_owned()),
            None,
            Vec::new(),
        );
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.save_season(1, season).await?;

        // Assert
        let persisted = stored.ok_or("the persisted season must be returned")?;
        assert_eq!(persisted.name(), Some("New Name"));
        let episode = persisted
            .episodes()
            .first()
            .ok_or("the replaced season must keep its episode")?;
        assert_eq!(episode.name(), "Pilot");
        assert_eq!(episode.video().id(), 20);
        Ok(())
    }

    #[tokio::test]
    async fn put_season_returns_none_for_an_unknown_show() -> Result<(), Box<dyn Error>> {
        // Arrange
        let mut transaction = begin_transaction().await?;
        let season = Season::new(SeasonIndex::new(1), None, None, Vec::new());
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.save_season(404, season).await?;

        // Assert
        assert!(stored.is_none(), "an unknown show must read as none");
        Ok(())
    }

    #[tokio::test]
    async fn update_season_changes_name_and_cover_but_keeps_episodes() -> Result<(), Box<dyn Error>>
    {
        // Arrange: season 1 holds an episode and no cover.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_image_file(&mut transaction, 12).await?;
        seed_image(&mut transaction, 12, 12).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, Some("Old Name")).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 20).await?;
        let season = Season::new(
            SeasonIndex::new(1),
            Some("New Name".to_owned()),
            Some(12),
            Vec::new(),
        );
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.save_season(1, season).await?;

        // Assert
        let persisted = stored.ok_or("the persisted season must be returned")?;
        assert_eq!(persisted.name(), Some("New Name"));
        assert_eq!(persisted.cover_image_id(), Some(12));
        let episode = persisted
            .episodes()
            .first()
            .ok_or("the persisted season must keep its episode")?;
        assert_eq!(episode.name(), "Pilot");
        assert_eq!(episode.video().id(), 20);
        Ok(())
    }

    #[tokio::test]
    async fn save_season_creates_an_absent_season() -> Result<(), Box<dyn Error>> {
        // Arrange: the show exists but holds no season at index 1.
        let mut transaction = begin_transaction().await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        let season = Season::new(
            SeasonIndex::new(1),
            Some("One".to_owned()),
            None,
            Vec::new(),
        );
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.save_season(1, season).await?;

        // Assert: save_season upserts, so an absent season is inserted.
        let persisted = stored.ok_or("the persisted season must be returned")?;
        assert_eq!(persisted.index().value(), 1);
        assert_eq!(persisted.name(), Some("One"));
        Ok(())
    }

    #[tokio::test]
    async fn put_episode_creates_then_replaces_by_index_replacing_the_video()
    -> Result<(), Box<dyn Error>> {
        // Arrange: two free videos of the show's season.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1280).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act: create the episode, then replace it with a different video.
        let created = repository
            .save_episode(
                1,
                1,
                Episode::try_new(
                    EpisodeIndex::new(1),
                    "Pilot".to_owned(),
                    "A plot".to_owned(),
                    None,
                    sample_video(20, 10)?,
                )?,
            )
            .await?;
        let replaced = repository
            .save_episode(
                1,
                1,
                Episode::try_new(
                    EpisodeIndex::new(1),
                    "Pilot Two".to_owned(),
                    "Another plot".to_owned(),
                    None,
                    sample_video(21, 11)?,
                )?,
            )
            .await?;

        // Assert
        let inserted = created.ok_or("the inserted episode must be returned")?;
        assert_eq!(inserted.name(), "Pilot");
        assert_eq!(inserted.video().id(), 20);
        let playback = replaced.ok_or("the playback episode must be returned")?;
        assert_eq!(playback.name(), "Pilot Two");
        assert_eq!(
            playback.video().id(),
            21,
            "the replacement must store the newly named video"
        );
        let old_attachment = repository
            .search_episodes(&EpisodeFilter {
                video_id: Some(20),
                ..EpisodeFilter::default()
            })
            .await?
            .into_iter()
            .next();
        assert!(
            old_attachment.is_none(),
            "the previous video must become unattached"
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_episode_returns_none_for_an_unknown_show_or_season() -> Result<(), Box<dyn Error>>
    {
        // Arrange: show 1 exists with no season; show 404 does not exist.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);
        let episode = Episode::try_new(
            EpisodeIndex::new(1),
            "Pilot".to_owned(),
            "A plot".to_owned(),
            None,
            sample_video(20, 10)?,
        )?;

        // Act
        let unknown_show = repository.save_episode(404, 1, episode.clone()).await?;
        let unknown_season = repository.save_episode(1, 1, episode).await?;

        // Assert
        assert!(unknown_show.is_none(), "an unknown show must read as none");
        assert!(
            unknown_season.is_none(),
            "a show without the season must read as none"
        );
        Ok(())
    }

    #[tokio::test]
    async fn put_episode_rejects_a_video_backing_a_film() -> Result<(), Box<dyn Error>> {
        // Arrange: film 1 already backs video 20.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        sqlx::query(
            "INSERT INTO movie (movie_id, name, plot, video_id)
             VALUES (1, 'Some Like It Hot', 'A plot', 20)",
        )
        .execute(&mut *transaction)
        .await?;
        let episode = Episode::try_new(
            EpisodeIndex::new(1),
            "Pilot".to_owned(),
            "A plot".to_owned(),
            None,
            sample_video(20, 10)?,
        )?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let result = repository.save_episode(1, 1, episode).await;

        // Assert
        assert!(
            matches!(result, Err(RepositoryError::Conflict)),
            "the cross-kind trigger must reject a video backing a film, got: {result:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn save_episode_changes_metadata_keeping_the_same_video() -> Result<(), Box<dyn Error>> {
        // Arrange: episode 1 backs video 20; the replacement names the same video.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 20).await?;
        let updated = Episode::try_new(
            EpisodeIndex::new(1),
            "New Name".to_owned(),
            "New plot".to_owned(),
            Some(NaiveDate::from_ymd_opt(1992, 2, 3).unwrap_or_default()),
            sample_video(20, 10)?,
        )?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.save_episode(1, 1, updated).await?;

        // Assert
        let persisted = stored.ok_or("the persisted episode must be returned")?;
        assert_eq!(persisted.name(), "New Name");
        assert_eq!(persisted.plot(), "New plot");
        assert_eq!(
            persisted.air_date(),
            Some(NaiveDate::from_ymd_opt(1992, 2, 3).unwrap_or_default())
        );
        assert_eq!(
            persisted.video().id(),
            20,
            "an unchanged video must stay attached"
        );
        Ok(())
    }

    #[tokio::test]
    async fn save_episode_creates_an_absent_episode() -> Result<(), Box<dyn Error>> {
        // Arrange: the show and season exist but hold no episode.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        let updated = Episode::try_new(
            EpisodeIndex::new(1),
            "Pilot".to_owned(),
            "A plot".to_owned(),
            None,
            sample_video(20, 10)?,
        )?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let stored = repository.save_episode(1, 1, updated).await?;

        // Assert: save_episode upserts, so an absent episode is inserted.
        let persisted = stored.ok_or("the persisted episode must be returned")?;
        assert_eq!(persisted.name(), "Pilot");
        assert_eq!(persisted.video().id(), 20);
        Ok(())
    }

    #[tokio::test]
    async fn search_episodes_filters_by_video() -> Result<(), Box<dyn Error>> {
        // Arrange: video 20 backs an episode, video 21 is unattached.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video_file(&mut transaction, 11).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        seed_video(&mut transaction, 21, 11, 1280).await?;
        seed_series(&mut transaction, 1, "Twin Peaks").await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 20).await?;
        let mut repository = SqlxSeriesRepository::new(&mut transaction);

        // Act
        let attached = repository
            .search_episodes(&EpisodeFilter {
                video_id: Some(20),
                ..EpisodeFilter::default()
            })
            .await?
            .into_iter()
            .next();
        let unattached = repository
            .search_episodes(&EpisodeFilter {
                video_id: Some(21),
                ..EpisodeFilter::default()
            })
            .await?
            .into_iter()
            .next();

        // Assert
        let episode = attached.ok_or("the attached episode must be returned")?;
        assert_eq!(episode.index().value(), 1);
        assert_eq!(episode.name(), "Pilot");
        assert_eq!(episode.video().id(), 20);
        assert!(
            unattached.is_none(),
            "a video the catalogue does not attach must read as none"
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_episode_filters_by_video_removing_only_the_episode()
    -> Result<(), Box<dyn Error>> {
        // Arrange: video 20 backs the episode, video 21 is unattached.
        let mut transaction = begin_transaction().await?;
        seed_user(&mut transaction).await?;
        seed_video_file(&mut transaction, 10).await?;
        seed_video(&mut transaction, 20, 10, 1920).await?;
        sqlx::query(
            "INSERT INTO series (series_id, name, plot) VALUES (1, 'Twin Peaks', 'A plot')",
        )
        .execute(&mut *transaction)
        .await?;
        seed_season(&mut transaction, 40, 1, 1, None).await?;
        seed_episode(&mut transaction, 60, 40, 1, "Pilot", 20).await?;
        // Act
        let (removed, missing) = {
            let mut repository = SqlxSeriesRepository::new(&mut transaction);
            let removed = repository
                .delete_episode(&EpisodeFilter {
                    video_id: Some(20),
                    ..EpisodeFilter::default()
                })
                .await?;
            let missing = repository
                .delete_episode(&EpisodeFilter {
                    video_id: Some(21),
                    ..EpisodeFilter::default()
                })
                .await?;
            (removed, missing)
        };
        let seasons = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM season")
            .fetch_one(&mut *transaction)
            .await?;

        // Assert
        assert!(removed);
        assert!(!missing);
        assert_eq!(seasons, 1, "the season and its show are retained");
        Ok(())
    }
}
