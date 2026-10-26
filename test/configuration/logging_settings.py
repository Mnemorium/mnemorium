"""Behavioural cases for the `logging.*` settings.

`level` changes which messages are written, `ansi` colours stdout, `rotation` names the log file and `max_files` bounds
how many are kept. Every assertion on file contents happens after `stop()`: the file sink is
`tracing_appender::non_blocking`, so only the dropped `WorkerGuard` guarantees the flush. Directory listings are safe
while the server runs.

No case waits for a rotation: `rotation` is observed through the file name, and `max_files` prunes when the appender is
built, so a pre-seeded `logs/` needs no clock.
"""

import shutil
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

import pytest
from harness import LOG_DIRECTORY, MSG_CONFIG_GENERATED, Sut

_LEVEL_ENV = "MNEMORIUM_LOGGING_LEVEL"
_ROTATION_NEVER_FILE = "mnemorium.log"

# Five sessions old enough that any pruning order keeps the count predictable: the
# appender keeps `max_files - 1` of them and then creates today's file.
_SEEDED_LOGS = {f"logs/mnemorium.log.2020-01-0{day}": "old\n" for day in range(1, 6)}


@contextmanager
def _control_server(image: str, tmp_root: Path, **start) -> Iterator[Sut]:
    """A second server for the paired control run, removed with its working directory."""
    server = Sut(image=image, tmp_root=tmp_root)
    tmp_root.mkdir(parents=True, exist_ok=True)
    try:
        yield server.start(**start)
    finally:
        server.stop()
        shutil.rmtree(server.dir, ignore_errors=True)


@pytest.mark.slow
def test_level_filters_operator_messages(image, sut, tmp_path):
    """`logging.level: error` drops the start-up warning that `warn` keeps."""
    sut.start(env={_LEVEL_ENV: "error"})
    sut.stop()

    assert MSG_CONFIG_GENERATED not in sut.log_text()

    with _control_server(image, tmp_path / "control", env={_LEVEL_ENV: "warn"}) as control:
        control.stop()

        assert MSG_CONFIG_GENERATED in control.log_text()


def test_the_security_floor_survives_a_quiet_level(sut):
    """A quiet level cannot suppress the security target's own lifecycle event."""
    sut.start(env={_LEVEL_ENV: "error"})
    sut.stop()

    assert 'event="lifecycle" component="logging" outcome="initialized"' in sut.log_text()


@pytest.mark.slow
def test_ansi_colours_stdout(image, sut, tmp_path):
    """`logging.ansi` colours the standard-output sink only."""
    sut.start(config={"logging": {"ansi": True}})

    assert b"\x1b[" in sut.logs().encode()

    with _control_server(image, tmp_path / "control", config={"logging": {"ansi": False}}) as control:
        assert b"\x1b[" not in control.logs().encode()


@pytest.mark.slow
def test_rotation_changes_the_file_name(image, sut, tmp_path):
    """`NEVER` writes one undated file; `DAILY` writes a dated one."""
    sut.start(config={"logging": {"rotation": "NEVER"}})

    assert sut.listing(LOG_DIRECTORY) == [_ROTATION_NEVER_FILE]

    with _control_server(image, tmp_path / "control", config={"logging": {"rotation": "DAILY"}}) as control:
        names = control.listing(LOG_DIRECTORY)

        assert _ROTATION_NEVER_FILE not in names
        assert any(name.startswith(f"{_ROTATION_NEVER_FILE}.") for name in names)


@pytest.mark.slow
def test_max_files_prunes_at_startup(image, sut, tmp_path):
    """`max_files: 2` keeps one seeded file plus today's; `max_files: 0` keeps everything."""
    sut.start(config={"logging": {"max_files": 2}}, files=_SEEDED_LOGS)
    sut.stop()

    assert len(sut.listing(LOG_DIRECTORY)) == 2

    quiet_control = _control_server(
        image,
        tmp_path / "control",
        config={"logging": {"max_files": 0}},
        files=_SEEDED_LOGS,
    )
    with quiet_control as control:
        control.stop()

        assert len(control.listing(LOG_DIRECTORY)) == 6
