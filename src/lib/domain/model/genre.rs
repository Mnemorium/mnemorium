/// A category the shared genre vocabulary recognizes.
///
/// The vocabulary is closed: an adapter maps every provider's spelling of an
/// equivalent genre to one of these values and drops a term it cannot map, so
/// the canonical response never carries a genre outside this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Genre {
    /// Action.
    Action,
    /// Adventure.
    Adventure,
    /// Animation.
    Animation,
    /// Biography.
    Biography,
    /// Comedy.
    Comedy,
    /// Crime.
    Crime,
    /// Documentary.
    Documentary,
    /// Drama.
    Drama,
    /// Family.
    Family,
    /// Fantasy.
    Fantasy,
    /// Film noir.
    FilmNoir,
    /// Game show.
    GameShow,
    /// History.
    History,
    /// Horror.
    Horror,
    /// Music.
    Music,
    /// Musical.
    Musical,
    /// Mystery.
    Mystery,
    /// News.
    News,
    /// Reality television.
    RealityTv,
    /// Romance.
    Romance,
    /// Science fiction.
    ScienceFiction,
    /// Short film.
    Short,
    /// Sport.
    Sport,
    /// Talk show.
    TalkShow,
    /// Thriller.
    Thriller,
    /// Television movie.
    TvMovie,
    /// War.
    War,
    /// Western.
    Western,
}

impl Genre {
    /// Return the canonical, uppercase identifier of the genre.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Action => "ACTION",
            Self::Adventure => "ADVENTURE",
            Self::Animation => "ANIMATION",
            Self::Biography => "BIOGRAPHY",
            Self::Comedy => "COMEDY",
            Self::Crime => "CRIME",
            Self::Documentary => "DOCUMENTARY",
            Self::Drama => "DRAMA",
            Self::Family => "FAMILY",
            Self::Fantasy => "FANTASY",
            Self::FilmNoir => "FILM NOIR",
            Self::GameShow => "GAME SHOW",
            Self::History => "HISTORY",
            Self::Horror => "HORROR",
            Self::Music => "MUSIC",
            Self::Musical => "MUSICAL",
            Self::Mystery => "MYSTERY",
            Self::News => "NEWS",
            Self::RealityTv => "REALITY TV",
            Self::Romance => "ROMANCE",
            Self::ScienceFiction => "SCIENCE FICTION",
            Self::Short => "SHORT",
            Self::Sport => "SPORT",
            Self::TalkShow => "TALK SHOW",
            Self::Thriller => "THRILLER",
            Self::TvMovie => "TV MOVIE",
            Self::War => "WAR",
            Self::Western => "WESTERN",
        }
    }

    /// Map a provider's genre name onto the vocabulary.
    ///
    /// Normalization is structural first — case-fold, replace every run of
    /// non-alphanumeric characters with one space, trim — and only then does the
    /// small alias table map the remaining synonyms (`sci fi` and
    /// `science fiction` both resolve to [`Genre::ScienceFiction`]). A term the
    /// vocabulary does not contain is absent, never guessed.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let folded = name.to_lowercase();
        let mut normalized = String::with_capacity(folded.len());
        let mut pending_space = false;
        for character in folded.chars() {
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
        match normalized.as_str() {
            "action" => Some(Self::Action),
            "adventure" => Some(Self::Adventure),
            "animation" => Some(Self::Animation),
            "biography" | "biopic" => Some(Self::Biography),
            "comedy" => Some(Self::Comedy),
            "crime" => Some(Self::Crime),
            "documentary" => Some(Self::Documentary),
            "drama" => Some(Self::Drama),
            "family" => Some(Self::Family),
            "fantasy" => Some(Self::Fantasy),
            "film noir" => Some(Self::FilmNoir),
            "game show" => Some(Self::GameShow),
            "history" => Some(Self::History),
            "horror" => Some(Self::Horror),
            "music" => Some(Self::Music),
            "musical" => Some(Self::Musical),
            "mystery" => Some(Self::Mystery),
            "news" => Some(Self::News),
            "reality tv" | "reality television" => Some(Self::RealityTv),
            "romance" => Some(Self::Romance),
            "science fiction" | "sci fi" | "scifi" => Some(Self::ScienceFiction),
            "short" => Some(Self::Short),
            "sport" => Some(Self::Sport),
            "talk show" => Some(Self::TalkShow),
            "thriller" => Some(Self::Thriller),
            "tv movie" | "tv film" | "television movie" => Some(Self::TvMovie),
            "war" => Some(Self::War),
            "western" => Some(Self::Western),
            _ => None,
        }
    }
}
