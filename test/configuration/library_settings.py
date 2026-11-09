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


REQUIRED_ROOT = {
    "library": {
        "startup": {
            "fail_without_media_root": True,
            "media_root_wait_seconds": 0,
        }
    }
}
OPTIONAL_ROOT = {
    "library": {
        "startup": {
            "fail_without_media_root": False,
            "media_root_wait_seconds": 0,
        }
    }
}


def test_a_required_absent_root_refuses_startup(sut):
    """The required root is absent, so startup is refused with the root named."""
    failure = sut.start_expecting_failure(create_media=False, config=REQUIRED_ROOT)

    assert failure.exit_code != 0
    assert "media" in failure.output.lower()
    assert "fail_without_media_root" in failure.output
    assert not (sut.dir / "media").exists(), "the server must never create the library root"


def test_a_wait_beyond_one_day_is_rejected_before_startup(sut):
    """An unbounded wait would overflow the startup deadline, so configuration refuses it."""
    failure = sut.start_expecting_failure(
        create_media=False,
        config={"library": {"startup": {"media_root_wait_seconds": 86_401}}},
    )

    assert failure.exit_code != 0
    assert "media_root_wait_seconds" in failure.output
    assert "86400" in failure.output
    assert not (sut.dir / "media").exists(), "the server must never create the library root"


def test_an_optional_absent_root_starts_without_creating_it(sut):
    """The optional root is absent, so the server starts and reconciles the library as empty."""
    sut.start(create_media=False, config=OPTIONAL_ROOT)

    assert sut.request("GET", HEALTH_PATH).status_code == 200
    sut.stop()
    assert not (sut.dir / "media").exists(), "the server must never create the library root"


def test_a_present_root_starts_without_waiting(sut):
    """A present root is decided immediately, even with a non-zero window."""
    sut.start(
        config={
            "library": {
                "startup": {
                    "fail_without_media_root": True,
                    "media_root_wait_seconds": 30,
                }
            }
        }
    )

    assert sut.request("GET", HEALTH_PATH).status_code == 200
