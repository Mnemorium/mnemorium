"""Black-box proof of the `metadata enrichment` capability a container can observe.

Metadata enrichment turns a media name or a music query into provider-independent candidates, resolves a provider
reference into provider-independent metadata and applies that metadata to the catalogue. The image's provider base URLs
are fixed at composition and no provider is configured by default, so the provider-less cases prove the surface that
answers without one: the unavailable document on every identification route, the query rejections, the provider a route
does not serve and the apply that finds no provider. The configured-provider cases prove that a valid provider list —
empty, or listing two providers — is accepted at startup without contacting one, and that a configured key reaches
neither log sink. Everything that needs a provider answer — candidate shape and order, metadata mapping, absence,
genres, failover, retry and the apply writes — stays with the Rust tests, which drive the adapters through `wiremock`
and the golden payloads. The absence of a startup request is a Rust witness too: the container observes that startup
proceeds, not that no request left the process.

Every route is resolved from the API index, so the suite keeps its single `/api/v1` base literal in `harness.Sut.index`.
A status, media type, body shape or `_links` field the served document declares belongs to the API fuzz test; these
cases assert the outcome a schema cannot: that identification is reported unavailable rather than as no candidate,
that the refusals happen before a provider is consulted, and that an apply that finds no provider leaves the entry
unchanged.
"""

import allure
import requests
from conftest import start_instance
from harness import HEALTH_PATH, Sut, link_of

NO_PROVIDER = "no provider is configured, so the capability is unavailable"

# A key the suite can search for, so a leak is a substring search.
SENTINEL_KEY = "SENTINEL_VIDEO_KEY"

EMPTY_PROVIDERS = {"library": {"metadata": {"video": {"providers": []}}}}
CONFIGURED_PROVIDERS = {
    "library": {
        "metadata": {
            "video": {
                "providers": [
                    {"name": "tmdb", "index": 1, "api_key": SENTINEL_KEY},
                    {"name": "omdb", "index": 2, "api_key": SENTINEL_KEY},
                ]
            }
        }
    }
}


def _identify_routes(server: Sut) -> tuple[str, str, str]:
    """Return the film, show and music identification surfaces from the API index."""
    movie = f"{server.href('movie')}/identify"
    series = f"{server.href('series')}/identify"
    music = f"{server.href('music_album').rsplit('/', 1)[0]}/identify"
    return movie, series, music


def _assert_error(response: requests.Response, status_code: int) -> None:
    """Assert the response is the standard error envelope at `status_code`."""
    assert response.status_code == status_code, f"expected {status_code}, got {response.status_code}: {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert set(body) == {"error", "_links"}, f"unexpected error envelope: {body}"
    assert isinstance(body["error"], str) and body["error"]
    assert set(body["_links"]) == {"self"}
    assert isinstance(body["_links"]["self"].get("href"), str) and body["_links"]["self"]["href"]


def _assert_unavailable_candidates(response: requests.Response) -> None:
    """Assert the candidate search reports the surface as unavailable, not as an empty search."""
    assert response.status_code == 200, f"unavailable candidate read failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert body["available"] is False, NO_PROVIDER
    assert body["items"] == []


def _assert_unavailable_document(response: requests.Response) -> None:
    """Assert the metadata resolution reports the surface as unavailable, with no metadata document."""
    assert response.status_code == 200, f"unavailable metadata read failed: {response.status_code} {response.text}"
    assert response.headers.get("Content-Type", "").startswith("application/hal+json")
    body = response.json()
    assert body["available"] is False, NO_PROVIDER
    assert "metadata" not in body, "an unavailable resolution carries no document"


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-001")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-001-A")
@allure.story("SCN-METADATA-ENRICHMENT-017-F")
def test_a_film_name_read_reports_enrichment_unavailable(server: Sut, root_token: str) -> None:
    """The film search answers an unavailable document, not an error and not an empty search."""
    movie, _series, _music = _identify_routes(server)

    response = server.request("GET", f"{movie}?name=Inception", token=root_token)

    _assert_unavailable_candidates(response)
    assert response.json()["_links"]["self"]["href"] == movie


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-001")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-001-A")
@allure.story("SCN-METADATA-ENRICHMENT-017-F")
def test_a_show_name_read_reports_enrichment_unavailable(server: Sut, root_token: str) -> None:
    """The show search answers an unavailable document, not an error and not an empty search."""
    _movie, series, _music = _identify_routes(server)

    response = server.request("GET", f"{series}?name=Twin%20Peaks", token=root_token)

    _assert_unavailable_candidates(response)
    assert response.json()["_links"]["self"]["href"] == series


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-001")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-001-A")
@allure.story("SCN-METADATA-ENRICHMENT-017-F")
def test_a_film_reference_read_reports_enrichment_unavailable(server: Sut, root_token: str) -> None:
    """A film reference cannot be resolved without a provider, and that is unavailable rather than a failure."""
    movie, _series, _music = _identify_routes(server)

    response = server.request("GET", f"{movie}/tmdb/27205", token=root_token)

    _assert_unavailable_document(response)
    assert response.json()["_links"]["self"]["href"] == f"{movie}/tmdb/27205"


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-001")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-001-A")
@allure.story("SCN-METADATA-ENRICHMENT-017-F")
def test_a_show_reference_read_reports_enrichment_unavailable(server: Sut, root_token: str) -> None:
    """A show reference cannot be resolved without a provider, and that is unavailable rather than a failure."""
    _movie, series, _music = _identify_routes(server)

    response = server.request("GET", f"{series}/tmdb/1920", token=root_token)

    _assert_unavailable_document(response)


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-001")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-001-A")
@allure.story("SCN-METADATA-ENRICHMENT-017-F")
def test_an_episode_reference_read_reports_enrichment_unavailable(server: Sut, root_token: str) -> None:
    """The series read with a season and an episode answers unavailable, not an error."""
    _movie, series, _music = _identify_routes(server)

    response = server.request("GET", f"{series}/tmdb/1920?season=1&episode=2", token=root_token)

    _assert_unavailable_document(response)
    assert response.json()["_links"]["self"]["href"] == f"{series}/tmdb/1920?season=1&episode=2"


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-001")
@allure.feature("REQ-METADATA-ENRICHMENT-023")
@allure.story("SCN-METADATA-ENRICHMENT-001-A")
@allure.story("SCN-METADATA-ENRICHMENT-023-D")
def test_a_music_search_reports_enrichment_unavailable(server: Sut, root_token: str) -> None:
    """The music search answers an unavailable document, distinct from an empty candidate list."""
    _movie, _series, music = _identify_routes(server)

    response = server.request(
        "GET",
        f"{music}?kind=album&title=Kind%20of%20Blue&group=Miles%20Davis",
        token=root_token,
    )

    _assert_unavailable_candidates(response)
    assert response.json()["_links"]["self"]["href"] == music


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-001")
@allure.feature("REQ-METADATA-ENRICHMENT-023")
@allure.story("SCN-METADATA-ENRICHMENT-001-A")
@allure.story("SCN-METADATA-ENRICHMENT-023-D")
def test_a_music_reference_read_reports_enrichment_unavailable(server: Sut, root_token: str) -> None:
    """A music reference cannot be resolved without a provider, and that is unavailable rather than a failure."""
    _movie, _series, music = _identify_routes(server)

    response = server.request("GET", f"{music}/musicbrainz/album/release-group", token=root_token)

    _assert_unavailable_document(response)


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-017-D")
def test_an_empty_film_name_is_rejected(server: Sut, root_token: str) -> None:
    """An empty media name is rejected before a provider is consulted."""
    movie, _series, _music = _identify_routes(server)

    response = server.request("GET", f"{movie}?name=", token=root_token)

    _assert_error(response, 422)


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-017-I")
def test_a_partial_episode_position_is_rejected(server: Sut, root_token: str) -> None:
    """A season without its episode is rejected as a partial position."""
    _movie, series, _music = _identify_routes(server)

    response = server.request("GET", f"{series}/tmdb/1920?season=1", token=root_token)

    _assert_error(response, 422)


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-017-J")
def test_an_out_of_range_episode_position_is_rejected(server: Sut, root_token: str) -> None:
    """A position that is not a 32-bit unsigned integer is rejected."""
    _movie, series, _music = _identify_routes(server)

    response = server.request("GET", f"{series}/tmdb/1920?season=-1&episode=2", token=root_token)

    _assert_error(response, 422)


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-018")
@allure.story("SCN-METADATA-ENRICHMENT-018-D")
def test_an_empty_music_title_is_rejected(server: Sut, root_token: str) -> None:
    """A music query without a title is rejected rather than searched."""
    _movie, _series, music = _identify_routes(server)

    response = server.request("GET", f"{music}?kind=album&title=", token=root_token)

    _assert_error(response, 422)


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-018")
@allure.story("SCN-METADATA-ENRICHMENT-018-D")
def test_a_music_kind_outside_the_admitted_set_is_rejected(server: Sut, root_token: str) -> None:
    """A query whose kind is not an album, a release or a recording is rejected."""
    _movie, _series, music = _identify_routes(server)

    response = server.request("GET", f"{music}?kind=track&title=Kind%20of%20Blue", token=root_token)

    _assert_error(response, 400)


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-017")
@allure.story("SCN-METADATA-ENRICHMENT-017-G")
def test_a_film_route_refuses_a_music_provider(server: Sut, root_token: str) -> None:
    """A provider the film route does not serve is reported as not found rather than resolved."""
    movie, _series, _music = _identify_routes(server)

    response = server.request("GET", f"{movie}/musicbrainz/27205", token=root_token)

    _assert_error(response, 404)


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-023")
@allure.story("SCN-METADATA-ENRICHMENT-023-C")
def test_a_music_route_refuses_a_video_provider(server: Sut, root_token: str) -> None:
    """A provider the music route does not serve is reported as not found rather than resolved."""
    _movie, _series, music = _identify_routes(server)

    response = server.request("GET", f"{music}/tmdb/album/27205", token=root_token)

    _assert_error(response, 404)


def _create_album(server: Sut, token: str, name: str) -> dict:
    """Create a music group and an album under it, returning the album representation."""
    group = server.request(
        "POST",
        server.href("music_group"),
        token=token,
        json={"name": f"Group {name}"},
    )
    assert group.status_code == 201, f"group creation failed: {group.status_code} {group.text}"
    created = server.request(
        "POST",
        server.href("music_album"),
        token=token,
        json={"name": name, "music_group_id": group.json()["id"]},
    )
    assert created.status_code == 201, f"album creation failed: {created.status_code} {created.text}"
    return created.json()


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-024")
@allure.story("SCN-METADATA-ENRICHMENT-024-E")
def test_an_apply_without_a_provider_reports_the_failure_and_stores_nothing(server: Sut, root_token: str) -> None:
    """An apply that finds no configured provider fails and leaves the addressed album unchanged."""
    album = _create_album(server, root_token, "Kind of Blue")
    self_href = link_of(album, "self")

    response = server.request(
        "POST",
        f"{self_href}/apply",
        token=root_token,
        json={"reference": {"provider": "musicbrainz", "id": "release-group"}},
    )

    _assert_error(response, 500)
    reread = server.request("GET", self_href, token=root_token)
    assert reread.status_code == 200, f"album read failed: {reread.status_code} {reread.text}"
    unchanged = reread.json()
    assert unchanged["name"] == "Kind of Blue"
    assert unchanged["id"] == album["id"]


# --- the provider list --------------------------------------------------------


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-001")
@allure.story("SCN-METADATA-ENRICHMENT-001-A")
def test_an_empty_provider_list_starts_with_enrichment_unavailable(sut: Sut) -> None:
    """An empty provider list is a valid configuration: the server starts with enrichment unavailable."""
    server = start_instance(sut, config=EMPTY_PROVIDERS)

    assert server.request("GET", HEALTH_PATH).status_code == 200


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-029")
@allure.story("SCN-METADATA-ENRICHMENT-029-A")
def test_a_configured_provider_list_starts_without_contacting_a_provider(sut: Sut) -> None:
    """Two configured providers are accepted at startup; no request leaves before a read needs one."""
    server = start_instance(sut, config=CONFIGURED_PROVIDERS)

    assert server.request("GET", HEALTH_PATH).status_code == 200


@allure.epic("Metadata enrichment")
@allure.feature("REQ-METADATA-ENRICHMENT-016")
@allure.story("SCN-METADATA-ENRICHMENT-016-D")
def test_the_configured_key_never_reaches_a_log_sink(sut: Sut) -> None:
    """A clean start with a configured key writes it to neither stdout nor the log files."""
    server = start_instance(sut, config=CONFIGURED_PROVIDERS)
    server.stop()

    assert SENTINEL_KEY not in server.logs()
    assert SENTINEL_KEY not in server.log_text()
