use std::mem::take;

use crate::domain::model::media_query::EpisodePosition;
use crate::domain::model::media_query::MediaQuery;
use crate::domain::model::media_query::VideoKind;

/// Folder and file names that denote an extra rather than the work itself.
pub const EXTRAS: &[&str] = &[
    "behind the scenes",
    "bonus",
    "bonus features",
    "deleted scenes",
    "extra",
    "extras",
    "featurette",
    "featurettes",
    "interview",
    "interviews",
    "sample",
    "samples",
    "scene",
    "scenes",
    "teaser",
    "teasers",
    "trailer",
    "trailers",
];

/// Folder names that group a library rather than name a work.
pub const GENERIC_FOLDERS: &[&str] = &[
    "download",
    "downloads",
    "film",
    "films",
    "library",
    "media",
    "movie",
    "movies",
    "television",
    "tv",
    "tv shows",
    "video",
    "videos",
    "watch",
];

/// Largest episode index a name may carry.
pub const MAX_EPISODE: u32 = 999;

/// Largest season index a name may carry.
pub const MAX_SEASON: u32 = 99;

/// Newest year a release or first-air date may carry.
pub const MAX_YEAR: u32 = 2100;

/// Oldest year a release or first-air date may carry.
pub const MIN_YEAR: u32 = 1878;

/// Tokens a release group uses that never belong to a title.
pub const RELEASE_TOKENS: &[&str] = &[
    "10bit",
    "4k",
    "8bit",
    "bd",
    "bdremux",
    "bdrip",
    "bluray",
    "brrip",
    "cam",
    "ddp",
    "divx",
    "dolby",
    "dovi",
    "dts",
    "dtshd",
    "dvd",
    "dvdr",
    "dvdrip",
    "eac3",
    "eztv",
    "extended",
    "flac",
    "hd",
    "hdcam",
    "hdr",
    "hdr10",
    "hdr10+",
    "hdrip",
    "hevc",
    "imax",
    "internal",
    "limited",
    "multi",
    "multi5",
    "multi6",
    "multisub",
    "nf",
    "nfo",
    "pdtv",
    "proper",
    "r5",
    "rarbg",
    "readnfo",
    "remastered",
    "remux",
    "repack",
    "rerip",
    "scr",
    "screener",
    "sd",
    "subbed",
    "subs",
    "truefrench",
    "truehd",
    "uhd",
    "uncut",
    "unrated",
    "vf",
    "vff",
    "vfq",
    "vostfr",
    "webdl",
    "webrip",
    "x264",
    "x265",
    "xvid",
    "yify",
];

/// File extensions a media file carries.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "3gp", "avi", "divx", "flv", "m2ts", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "mts", "ogm",
    "ogv", "rmvb", "ts", "vob", "webm", "wmv",
];

/// What one media name or path segment yields.
struct ParsedName {
    /// Episode index, when the segment supports one.
    episode: Option<u32>,
    /// Season index, when the segment supports one.
    season: Option<u32>,
    /// Title built from the tokens that survive classification.
    title: String,
    /// Title that keeps the year token, when a numeric title could own it.
    title_with_year: Option<String>,
    /// Release or first-air year, when the segment supports one.
    year: Option<u32>,
}

/// One token of a name, with the lowercased form used to classify it.
struct Token {
    /// Whether the token sat inside a bracket group.
    bracketed: bool,
    /// Lowercased form used for classification.
    lower: String,
    /// Original text, preserved for the title.
    text: String,
}

/// Service turning a media name or root-relative path into ordered search queries.
///
/// The interpretation is pure: it takes the best human-meaningful name the caller
/// has and returns an ordered, de-duplicated list of hypotheses, most supported
/// first. Anything the name does not support is absent rather than invented: no
/// year, no season and no episode is ever fabricated, and a name with nothing
/// usable yields an empty list.
#[non_exhaustive]
pub struct MediaNameInterpreter;

impl MediaNameInterpreter {
    /// Interpret `name` as an ordered list of search queries, most supported first.
    ///
    /// The order is evidence-based: a query backed by a folder precedes one
    /// backed only by the file name, each distinct query appears once, and a
    /// numeric token that could be either part of the title or the year yields
    /// both hypotheses rather than one guess.
    #[must_use]
    pub fn interpret(name: &str) -> Vec<MediaQuery> {
        // A disc folder groups the parts of a multi-disc release.
        let is_disc_folder = |value: &str| -> bool {
            let normalized = normalized_words(value);
            if normalized.starts_with("disc ") || normalized.starts_with("disk ") {
                return true;
            }
            ["cd", "dvd"].iter().any(|prefix| {
                normalized
                    .strip_prefix(*prefix)
                    .is_some_and(|rest| parse_digits(rest, 2).is_some())
            })
        };
        // A generic folder groups a library rather than naming a work.
        let is_generic_folder = |title: &str| -> bool {
            let normalized = normalized_words(title);
            GENERIC_FOLDERS.contains(&normalized.as_str())
        };
        // The nearest parent segment that names the work.
        let nearest_work_folder = |segments: &[&str]| -> Option<ParsedName> {
            for segment in segments.iter().rev() {
                if is_extras(segment)
                    || is_disc_folder(segment)
                    || parse_season_folder(segment).is_some()
                {
                    continue;
                }
                let parsed = parse_name(segment, None);
                if parsed.title.is_empty() || is_generic_folder(&parsed.title) {
                    continue;
                }
                return Some(parsed);
            }
            None
        };

        let mut segments: Vec<&str> = name
            .split(['/', '\\'])
            .filter(|segment| !segment.is_empty() && *segment != ".")
            .collect();
        let Some(file) = segments.pop() else {
            return Vec::new();
        };
        let stem = match file.rsplit_once('.') {
            Some((head, extension))
                if !head.is_empty()
                    && VIDEO_EXTENSIONS
                        .iter()
                        .any(|known| extension.eq_ignore_ascii_case(known)) =>
            {
                head
            }
            _ => file,
        };
        if is_extras(stem) || segments.iter().any(|segment| is_extras(segment)) {
            return Vec::new();
        }

        let folder = nearest_work_folder(&segments);
        let folder_season = segments
            .iter()
            .rev()
            .find_map(|segment| parse_season_folder(segment));
        let (name_part, absolute_episode) = match stem.rsplit_once(" - ") {
            Some((head, tail)) => match parse_digits(tail.trim(), 3) {
                Some(episode) if !head.trim().is_empty() => (head, Some(episode)),
                _ => (stem, None),
            },
            None => (stem, None),
        };
        let parsed = parse_name(name_part, folder_season);
        let season = parsed.season.or(folder_season);
        let episode = parsed.episode;
        let is_series = parsed.season.is_some()
            || episode.is_some()
            || absolute_episode.is_some()
            || folder_season.is_some();

        let mut queries = Vec::new();
        let folder_year = folder.as_ref().and_then(|named| named.year);
        if is_series {
            let position = match (season, episode) {
                (Some(found_season), Some(found_episode)) => {
                    Some(EpisodePosition::new(found_season, found_episode))
                }
                _ => None,
            };
            let folder_title = folder
                .as_ref()
                .map(|named| named.title.as_str())
                .unwrap_or_default();
            let title = if folder_title.is_empty() {
                parsed.title.as_str()
            } else {
                folder_title
            };
            add_query(
                &mut queries,
                VideoKind::Series,
                title,
                folder_year.or(parsed.year),
                position,
            );
            if folder_year.is_some() {
                add_query(
                    &mut queries,
                    VideoKind::Series,
                    title,
                    parsed.year,
                    position,
                );
            }
            add_query(&mut queries, VideoKind::Series, title, None, position);
        } else {
            if let Some(named) = folder.as_ref() {
                add_query(
                    &mut queries,
                    VideoKind::Movie,
                    &named.title,
                    folder_year.or(parsed.year),
                    None,
                );
                if folder_year.is_some() {
                    add_query(
                        &mut queries,
                        VideoKind::Movie,
                        &named.title,
                        parsed.year,
                        None,
                    );
                }
                add_query(&mut queries, VideoKind::Movie, &named.title, None, None);
            }
            add_query(
                &mut queries,
                VideoKind::Movie,
                &parsed.title,
                parsed.year,
                None,
            );
            if let Some(combined) = parsed.title_with_year.as_ref() {
                add_query(&mut queries, VideoKind::Movie, combined, None, None);
            }
            add_query(&mut queries, VideoKind::Movie, &parsed.title, None, None);
        }
        queries
    }
}

/// Append `title` as a query unless it is empty or already present.
fn add_query(
    queries: &mut Vec<MediaQuery>,
    kind: VideoKind,
    title: &str,
    year: Option<u32>,
    episode: Option<EpisodePosition>,
) {
    let Ok(query) = MediaQuery::try_new(kind, title.to_owned(), year, episode) else {
        return;
    };
    if !queries.contains(&query) {
        queries.push(query);
    }
}

/// Return whether `value` names an extra rather than the work itself.
fn is_extras(value: &str) -> bool {
    let normalized = normalized_words(value);
    EXTRAS.iter().any(|phrase| {
        normalized == *phrase
            || normalized
                .strip_prefix(*phrase)
                .is_some_and(|rest| rest.starts_with(' '))
    })
}

/// Join title tokens, merging a run of single-character tokens into one word.
fn join_title(tokens: &[String]) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut acronym = String::new();
    for token in tokens {
        if token.chars().count() == 1 && token.chars().all(char::is_alphanumeric) {
            acronym.push_str(token);
        } else {
            if !acronym.is_empty() {
                words.push(take(&mut acronym));
            }
            words.push(token.clone());
        }
    }
    if !acronym.is_empty() {
        words.push(acronym);
    }
    words.join(" ")
}

/// Fold `value` to lowercase words separated by single spaces.
fn normalized_words(value: &str) -> String {
    let mut normalized = String::new();
    let mut pending_space = false;
    for character in value.to_lowercase().chars() {
        if character.is_alphanumeric() {
            if pending_space && !normalized.is_empty() {
                normalized.push(' ');
            }
            normalized.push(character);
            pending_space = false;
        } else {
            pending_space = true;
        }
    }
    normalized
}

/// Parse up to `max_digits` ASCII digits into a positive number.
fn parse_digits(text: &str, max_digits: usize) -> Option<u32> {
    if text.is_empty() || text.len() > max_digits || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    text.parse::<u32>().ok().filter(|value| *value > 0)
}

/// Parse one name or path segment into the evidence it carries.
fn parse_name(value: &str, season_hint: Option<u32>) -> ParsedName {
    // A two-token release tag, written as two tokens.
    let is_compound_release = |lower: &str, next: Option<&str>| -> bool {
        matches!(
            (lower, next),
            ("web", Some("dl" | "rip"))
                | ("blu", Some("ray"))
                | ("br" | "dvd" | "hd", Some("rip"))
                | ("dvd", Some("scr"))
                | ("directors" | "director's" | "final", Some("cut"))
                | ("dolby", Some("vision"))
        )
    };
    // A release tag that carries a number.
    let is_release_pattern = |lower: &str| -> bool {
        let resolution = lower.strip_suffix('p').or_else(|| lower.strip_suffix('i'));
        if resolution.is_some_and(|digits| {
            matches!(
                digits,
                "360" | "480" | "540" | "576" | "720" | "1080" | "1440" | "2160" | "4320"
            )
        }) {
            return true;
        }
        if let Some(digits) = lower.strip_prefix('x').or_else(|| lower.strip_prefix('h'))
            && matches!(digits, "264" | "265")
        {
            return true;
        }
        [
            "aac", "ac3", "atmos", "av1", "dd", "ddp", "dts", "eac3", "flac", "opus", "truehd",
            "vp9",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    };
    // A release token that never belongs to a title.
    let is_release =
        |lower: &str| -> bool { RELEASE_TOKENS.contains(&lower) || is_release_pattern(lower) };
    // A character that separates two tokens.
    let is_separator = |character: char| -> bool {
        character.is_whitespace()
            || matches!(
                character,
                '.' | '_'
                    | '-'
                    | '+'
                    | '&'
                    | ','
                    | ';'
                    | '!'
                    | '?'
                    | ':'
                    | '/'
                    | '\\'
                    | '|'
                    | '*'
                    | '~'
                    | '#'
                    | '@'
                    | '"'
                    | '='
                    | '^'
                    | '$'
                    | '%'
                    | '\u{2013}'
                    | '\u{2014}'
            )
    };
    // Append the current token to the list and clear it.
    let push_token = |tokens: &mut Vec<Token>, current: &mut String, bracketed: bool| {
        let text = take(current);
        if text.is_empty() {
            return;
        }
        tokens.push(Token {
            bracketed,
            lower: text.to_lowercase(),
            text,
        });
    };
    // Split a segment into tokens, marking the ones inside a bracket group.
    let tokenize = |text: &str| -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut depth: u32 = 0;
        for character in text.chars() {
            match character {
                '[' | '{' | '(' => {
                    push_token(&mut tokens, &mut current, depth > 0);
                    depth = depth.saturating_add(1);
                }
                ']' | '}' | ')' => {
                    push_token(&mut tokens, &mut current, depth > 0);
                    depth = depth.saturating_sub(1);
                }
                _ if is_separator(character) => push_token(&mut tokens, &mut current, depth > 0),
                _ => current.push(character),
            }
        }
        push_token(&mut tokens, &mut current, depth > 0);
        tokens
    };
    // An empty remainder or a continued multi-episode marker.
    let is_episode_tail = |rest: &str| -> bool {
        if rest.is_empty() {
            return true;
        }
        let Some(tail) = rest.strip_prefix('e') else {
            return false;
        };
        take_digits(tail).is_some_and(|(_, remainder)| remainder.is_empty())
    };
    // A `1x02` style season-and-episode marker.
    let parse_cross = |text: &str| -> Option<(u32, u32)> {
        let (season, remainder) = take_digits(text)?;
        let rest = remainder.strip_prefix('x')?;
        let (episode, tail) = take_digits(rest)?;
        if !tail.is_empty()
            || season == 0
            || episode == 0
            || season > MAX_SEASON
            || episode > MAX_EPISODE
        {
            return None;
        }
        Some((season, episode))
    };
    // An `E02` style episode marker.
    let parse_episode_word = |text: &str| -> Option<u32> {
        let digits = text.strip_prefix('e')?;
        parse_digits(digits, 3)
    };
    // An `S01E02[E03…]` style season-and-episode marker.
    let parse_season_episode = |text: &str| -> Option<(u32, u32)> {
        let (season, remainder) = take_digits(text.strip_prefix('s')?)?;
        let (episode, tail) = take_digits(remainder.strip_prefix('e')?)?;
        if season == 0
            || episode == 0
            || season > MAX_SEASON
            || episode > MAX_EPISODE
            || !is_episode_tail(tail)
        {
            return None;
        }
        Some((season, episode))
    };
    // An `S02` style season marker that carries no episode.
    let parse_season_word = |text: &str| -> Option<u32> {
        let digits = text.strip_prefix('s')?;
        parse_digits(digits, 2)
    };
    // Split the trailing year off the surviving content tokens.
    let split_year = |content: Vec<String>| -> (String, Option<u32>, Option<String>) {
        let year = content
            .last()
            .filter(|_| content.len() > 1)
            .and_then(|token| parse_year(token));
        match year {
            Some(found_year) => {
                let title_with_year = join_title(&content);
                let mut title_tokens = content;
                title_tokens.pop();
                (
                    join_title(&title_tokens),
                    Some(found_year),
                    Some(title_with_year),
                )
            }
            None => (join_title(&content), None, None),
        }
    };

    let mut tokens = tokenize(value).into_iter().peekable();
    let mut content: Vec<String> = Vec::new();
    let mut episode = None;
    let mut previous_release = false;
    let mut season = None;

    while let Some(token) = tokens.next() {
        let lower = token.lower.as_str();
        let next = tokens.peek().map(|following| following.lower.as_str());
        if is_compound_release(lower, next) {
            let _second = tokens.next();
            previous_release = true;
            continue;
        }
        if is_release(lower) {
            previous_release = true;
            continue;
        }
        if let Some((found_season, found_episode)) = parse_season_episode(lower) {
            season = Some(found_season);
            episode = Some(found_episode);
            previous_release = false;
            continue;
        }
        if let Some((found_season, found_episode)) = parse_cross(lower) {
            season = Some(found_season);
            episode = Some(found_episode);
            previous_release = false;
            continue;
        }
        if let Some(found_episode) = parse_episode_word(lower) {
            episode = Some(found_episode);
            previous_release = false;
            continue;
        }
        if matches!(lower, "episode" | "ep" | "e")
            && let Some(found_episode) = next.and_then(|neighbour| parse_digits(neighbour, 3))
        {
            let _number = tokens.next();
            episode = Some(found_episode);
            previous_release = false;
            continue;
        }
        if let Some(found_season) = parse_season_word(lower) {
            season = Some(found_season);
            previous_release = false;
            continue;
        }
        if previous_release && lower.len() <= 2 && lower.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        previous_release = false;
        if token.bracketed && parse_year(lower).is_none() {
            continue;
        }
        content.push(token.text);
    }

    if episode.is_none()
        && (season.is_some() || season_hint.is_some())
        && content.len() == 1
        && let Some(found_episode) = content.first().and_then(|token| parse_digits(token, 3))
    {
        episode = Some(found_episode);
        content.clear();
    }

    let (title, year, title_with_year) = split_year(content);
    ParsedName {
        episode,
        season,
        title,
        title_with_year,
        year,
    }
}

/// Parse a `Season 2` or `S02` style folder.
fn parse_season_folder(value: &str) -> Option<u32> {
    let normalized = normalized_words(value);
    if normalized == "specials" {
        return Some(0);
    }
    let digits = normalized
        .strip_prefix("season ")
        .or_else(|| normalized.strip_prefix("seasons "))
        .or_else(|| normalized.strip_prefix('s'))?;
    parse_digits(digits, 2)
}

/// Parse a four-digit release year.
fn parse_year(text: &str) -> Option<u32> {
    if text.len() != 4 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u32>()
        .ok()
        .filter(|year| (MIN_YEAR..=MAX_YEAR).contains(year))
}

/// Split the leading digits off `text`, returning them with the remainder.
fn take_digits(text: &str) -> Option<(u32, &str)> {
    let end = text
        .bytes()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    let value = text.get(..end)?.parse::<u32>().ok()?;
    Some((value, text.get(end..)?))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use crate::domain::model::media_query::EpisodePosition;
    use crate::domain::model::media_query::MediaQuery;
    use crate::domain::model::media_query::VideoKind;

    use super::MediaNameInterpreter;

    /// Assert every query of one interpreted name keeps the corpus invariants.
    fn assert_invariants(name: &str) {
        let queries = MediaNameInterpreter::interpret(name);
        let release_like = [
            "1080p", "720p", "2160p", "480p", "bluray", "webdl", "webrip", "x264", "x265", "hevc",
        ];

        for query in &queries {
            assert!(
                !query.title().is_empty(),
                "every query of {name:?} must carry a title"
            );
            assert!(
                query.episode().is_none() || query.kind() == VideoKind::Series,
                "an episode of {name:?} must belong to a show"
            );
            assert!(
                query.title().split_whitespace().all(|word| {
                    let lower = word.to_lowercase();
                    !super::RELEASE_TOKENS.contains(&lower.as_str())
                        && !release_like.contains(&lower.as_str())
                }),
                "no release token may survive in the title of {name:?}, got: {:?}",
                query.title()
            );
        }
        assert_eq!(
            queries,
            MediaNameInterpreter::interpret(name),
            "the order of {name:?} must be deterministic"
        );
        assert!(
            queries.iter().enumerate().all(|(index, query)| queries
                .iter()
                .skip(index.saturating_add(1))
                .all(|other| other != query)),
            "a distinct query of {name:?} must appear once"
        );
    }

    #[rstest]
    #[case::folder_year(
        "Movies/Inception (2010)/Inception.2010.1080p.BluRay.mkv",
        VideoKind::Movie,
        "Inception",
        Some(2010),
        None
    )]
    #[case::file_year(
        "The.Matrix.1999.1080p.mkv",
        VideoKind::Movie,
        "The Matrix",
        Some(1999),
        None
    )]
    #[case::numeric_title("1917.2019.1080p.mkv", VideoKind::Movie, "1917", Some(2019), None)]
    #[case::acronym(
        "S.W.A.T.2017.S01E02.1080p.mkv",
        VideoKind::Series,
        "SWAT",
        Some(2017),
        Some((1, 2))
    )]
    #[case::season_episode_token(
        "Show.S01E02.1080p.mkv",
        VideoKind::Series,
        "Show",
        None,
        Some((1, 2))
    )]
    #[case::cross("Show.1x02.mkv", VideoKind::Series, "Show", None, Some((1, 2)))]
    #[case::season_folder(
        "Show/Season 2/Episode 5.mkv",
        VideoKind::Series,
        "Show",
        None,
        Some((2, 5))
    )]
    #[case::season_folder_bare_episode(
        "Show/Season 1/05.mkv",
        VideoKind::Series,
        "Show",
        None,
        Some((1, 5))
    )]
    #[case::absolute("Show - 12.mkv", VideoKind::Series, "Show", None, None)]
    #[case::multi_episode(
        "Show.S01E02E03.1080p.mkv",
        VideoKind::Series,
        "Show",
        None,
        Some((1, 2))
    )]
    #[case::brackets(
        "[Group] Show - S01E02 [1080p].mkv",
        VideoKind::Series,
        "Show",
        None,
        Some((1, 2))
    )]
    #[case::non_ascii("Amélie.2001.1080p.mkv", VideoKind::Movie, "Amélie", Some(2001), None)]
    #[case::extension_less(
        "Movies/Inception (2010)/Inception.2010.1080p.BluRay",
        VideoKind::Movie,
        "Inception",
        Some(2010),
        None
    )]
    fn media_name_interpreter_interpret_yields_the_expected_first_query(
        #[case] name: &str,
        #[case] kind: VideoKind,
        #[case] title: &str,
        #[case] year: Option<u32>,
        #[case] episode: Option<(u32, u32)>,
    ) {
        // Act
        let queries = MediaNameInterpreter::interpret(name);

        // Assert
        assert_eq!(
            queries.first().map(MediaQuery::kind),
            Some(kind),
            "unexpected kind for {name:?}"
        );
        assert_eq!(
            queries.first().map(MediaQuery::title),
            Some(title),
            "unexpected title for {name:?}"
        );
        assert_eq!(
            queries.first().and_then(MediaQuery::year),
            year,
            "unexpected year for {name:?}"
        );
        assert_eq!(
            queries.first().and_then(MediaQuery::episode),
            episode.map(|(found_season, found_episode)| {
                EpisodePosition::new(found_season, found_episode)
            }),
            "unexpected episode position for {name:?}"
        );
    }

    #[rstest]
    #[case("Movies/Inception (2010)/Inception.2010.1080p.BluRay.mkv")]
    #[case("The.Matrix.1999.1080p.mkv")]
    #[case("1917.2019.1080p.mkv")]
    #[case("S.W.A.T.2017.S01E02.1080p.mkv")]
    #[case("Show.S01E02.1080p.mkv")]
    #[case("Show.1x02.mkv")]
    #[case("Show/Season 2/Episode 5.mkv")]
    #[case("Show/Season 1/05.mkv")]
    #[case("Show - 12.mkv")]
    #[case("Show.S01E02E03.1080p.mkv")]
    #[case("[Group] Show - S01E02 [1080p].mkv")]
    #[case("Amélie.2001.1080p.mkv")]
    #[case("Movies/Inception (2010)/Inception.2010.1080p.BluRay")]
    fn media_name_interpreter_interpret_keeps_the_corpus_invariants(#[case] name: &str) {
        // Act & Assert
        assert_invariants(name);
    }

    #[rstest]
    #[case::sample("sample.mkv")]
    #[case::extras_folder("Movie/Extras/featurette.mkv")]
    #[case::release_only("1080p.mkv")]
    fn media_name_interpreter_interpret_yields_no_query_for_an_unusable_name(#[case] name: &str) {
        // Act
        let queries = MediaNameInterpreter::interpret(name);

        // Assert
        assert!(
            queries.is_empty(),
            "an unusable name must yield no query, got: {queries:?}"
        );
    }

    #[test]
    fn media_name_interpreter_interpret_orders_the_folder_year_before_the_file_year() {
        // Arrange: the folder names 2010 and the file names 2011.
        let name = "Inception (2010)/Inception.2011.1080p.mkv";

        // Act
        let queries = MediaNameInterpreter::interpret(name);

        // Assert
        assert_eq!(
            queries.first().and_then(MediaQuery::year),
            Some(2010),
            "the folder-backed year must come first, got: {queries:?}"
        );
        assert!(
            queries.iter().any(|query| query.year() == Some(2011)),
            "the file-backed year must follow the folder-backed one, got: {queries:?}"
        );
        assert_eq!(
            queries
                .iter()
                .filter(|query| query.title() == "Inception")
                .count(),
            3,
            "each distinct year hypothesis must appear once, got: {queries:?}"
        );
    }

    #[test]
    fn media_name_interpreter_interpret_returns_a_numeric_title_once() {
        // Arrange: the folder and the file name the same work.
        let name = "Movies/1917 (2019)/1917.2019.1080p.mkv";

        // Act
        let queries = MediaNameInterpreter::interpret(name);

        // Assert
        assert_eq!(
            queries.first().map(MediaQuery::title),
            Some("1917"),
            "unexpected first title, got: {queries:?}"
        );
        assert_eq!(queries.first().and_then(MediaQuery::year), Some(2019));
        assert_eq!(
            queries
                .iter()
                .filter(|query| query.title() == "1917")
                .count(),
            2,
            "the title must appear once per distinct year, got: {queries:?}"
        );
    }
}
