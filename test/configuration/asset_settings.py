"""Behavioural cases for the `asset.*` settings.

`max_file_size_bytes` and `chunk_size_bytes` change what the begin and chunk endpoints accept; `expiry_seconds` changes
how long a session can still be completed. Input parsing and input rejection are not subjects here (TEST-057): every
case supplies a valid value and asserts its effect.

`asset.storage.root` has no case: the root is expected to become a fixed constant rather than a setting (#328).
"""

import hashlib
import time

from harness import ROOT_ADMIN_USERNAME, UPLOAD_PATH, begin_upload, complete_upload, put_chunk

_CHUNK_SIZE = 1024


def _root_token(sut) -> str:
    """A session for the only user that owns every upload in these cases."""
    return sut.login(ROOT_ADMIN_USERNAME, sut.root_password())


def test_max_file_size_rejects_a_bigger_upload(sut):
    """A declared size above `max_file_size_bytes` is refused before any chunk arrives."""
    sut.start(config={"asset": {"upload": {"max_file_size_bytes": 2048}}})
    token = _root_token(sut)

    response = sut.request(
        "POST",
        UPLOAD_PATH,
        token=token,
        json={
            "content_type": "image/png",
            "file_name": "too_big.png",
            "file_size": 4096,
            "integrity_hash": hashlib.sha256(b"").hexdigest(),
        },
    )

    assert response.status_code == 413


def test_a_chunk_at_the_configured_size_is_accepted(sut):
    """A body exactly at `chunk_size_bytes` passes the route's body limit and the use case."""
    sut.start(config={"asset": {"upload": {"chunk_size_bytes": _CHUNK_SIZE}}})
    token = _root_token(sut)
    data = b"\x00" * _CHUNK_SIZE

    upload_id = begin_upload(sut, token, "within_limit.png", data)
    response = put_chunk(sut, token, upload_id, 0, data, total=len(data))

    assert response.status_code == 200


def test_a_chunk_above_the_configured_size_is_rejected(sut):
    """A body above `chunk_size_bytes` is refused by the route's body limit."""
    sut.start(config={"asset": {"upload": {"chunk_size_bytes": _CHUNK_SIZE}}})
    token = _root_token(sut)
    data = b"\x00" * (_CHUNK_SIZE * 2)

    upload_id = begin_upload(sut, token, "too_big_chunk.png", data)
    response = put_chunk(sut, token, upload_id, 0, data, total=len(data))

    assert response.status_code == 413


def test_an_expired_upload_cannot_be_completed(sut):
    """A session older than `expiry_seconds` is gone, not completable."""
    sut.start(config={"asset": {"upload": {"expiry_seconds": 1}}})
    token = _root_token(sut)
    upload_id = begin_upload(sut, token, "expired.png", b"\x00")

    time.sleep(2)

    response = complete_upload(sut, token, upload_id)

    assert response.status_code == 410
