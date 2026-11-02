use crate::domain::model::scope::Scope;

/// Parse a stored scope token into its domain value.
///
/// Returns `None` when `value` is not one of the tokens the
/// `chk_image_scope` and `chk_video_scope` check constraints admit, so an
/// adapter can report a value the domain cannot hold as a data-integrity
/// failure rather than serving it.
#[must_use]
pub(crate) fn parse_scope(value: &str) -> Option<Scope> {
    match value {
        "LIBRARY" => Some(Scope::Library),
        "USER" => Some(Scope::User),
        _ => None,
    }
}

/// Map a domain scope to its persisted representation.
#[must_use]
pub(crate) const fn scope_token(scope: Scope) -> &'static str {
    match scope {
        Scope::Library => "LIBRARY",
        Scope::User => "USER",
    }
}
