"""Behavioural cases for the `library.metadata.video` settings.

The provider list is optional: an empty list is the built-in default and leaves identification unavailable. A configured
entry is accepted at startup, and because the adapters are built lazily no request leaves the server until a trigger
identifies a video. The API key is configuration only: it is read from `config.yaml`, never written back, and never
reaches a log sink.
"""

import yaml
from harness import CONFIG_PATH, HEALTH_PATH

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


def test_an_empty_provider_list_starts_with_identification_unavailable(sut):
    """An empty list is a valid configuration: the server starts with the feature off."""
    sut.start(config=EMPTY_PROVIDERS)

    assert sut.request("GET", HEALTH_PATH).status_code == 200


def test_a_configured_provider_starts_without_a_network_call(sut):
    """Two configured providers do not delay startup: no adapter is built until it is used."""
    sut.start(config=CONFIGURED_PROVIDERS)

    assert sut.request("GET", HEALTH_PATH).status_code == 200


def test_the_key_never_reaches_a_log_sink(sut):
    """The configured API key is absent from both sinks."""
    sut.start(config=CONFIGURED_PROVIDERS)
    sut.stop()

    assert SENTINEL_KEY not in sut.logs()
    assert SENTINEL_KEY not in sut.log_text()


def test_the_configuration_file_is_never_rewritten(sut):
    """The server reads the operator's file and writes nothing back to it."""
    sut.start(config=CONFIGURED_PROVIDERS)
    sut.stop()

    assert sut.read(CONFIG_PATH) == yaml.safe_dump(CONFIGURED_PROVIDERS)
