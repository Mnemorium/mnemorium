"""The harness's own configuration case: an operator supplied nothing at all.

No `config.yaml` and no `MNEMORIUM_*` variable is a real configuration — the one where every setting takes its built-in
default. It is the only test in the suite with no `state_scenario` marker: no scenario of the instance state spec
describes it, and it is not a smoke test but the assertion that the generated file records the defaults.
"""

from harness import CONFIG_PATH, DATABASE_PATH, HEALTH_PATH, STATE_PATH


def test_an_empty_configuration_starts_and_generates_the_file(sut):
    sut.start()

    assert sut.request("GET", HEALTH_PATH).status_code == 200
    assert sut.exists(CONFIG_PATH)
    assert sut.exists(STATE_PATH)
    assert sut.exists(DATABASE_PATH)
