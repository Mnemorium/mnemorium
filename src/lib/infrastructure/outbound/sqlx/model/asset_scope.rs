use crate::domain::model::AssetScope;

/// Parse a stored asset scope token into its domain value.
///
/// Returns `None` when `value` is not one of the tokens the
/// `chk_image_scope` and `chk_video_scope` check constraints admit, so an
/// adapter can report a value the domain cannot hold as a data-integrity
/// failure rather than serving it.
#[must_use]
pub(crate) fn parse_asset_scope(value: &str) -> Option<AssetScope> {
    match value {
        "LIBRARY" => Some(AssetScope::Library),
        "USER" => Some(AssetScope::User),
        _ => None,
    }
}

/// Map a domain asset scope to its persisted representation.
#[must_use]
pub(crate) const fn asset_scope_token(scope: AssetScope) -> &'static str {
    match scope {
        AssetScope::Library => "LIBRARY",
        AssetScope::User => "USER",
    }
}
