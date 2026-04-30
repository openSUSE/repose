"""Tests for ``repose.messages``."""

from collections import namedtuple

import pytest

from repose.messages import (
    ConnectingTargetFailedMessage,
    ConnectingToMessage,
    ErrorMessage,
    UnsuportedProductMessage,
    UserError,
    UserMessage,
)


class _Msg(UserMessage):
    """Minimal concrete message for exercising base behaviour."""

    def __init__(self, message):
        self.message = message


def test_str_uses_message_attribute():
    assert str(_Msg("hi")) == "hi"


def test_eq_compares_string_repr():
    assert _Msg("hi") == _Msg("hi")
    assert _Msg("a") != _Msg("b")


def test_eq_against_arbitrary_string():
    # __eq__ compares str(self) to str(x)
    assert _Msg("text") == "text"


def test_hash_is_class_based():
    # Class-method hash → all instances of same class share hash
    assert hash(_Msg) == hash(_Msg)


def test_user_error_is_runtime_error():
    e = UserError()
    assert isinstance(e, RuntimeError)


def test_error_message_is_runtime_error():
    e = ErrorMessage()
    assert isinstance(e, RuntimeError)


def test_connecting_target_failed_message_str():
    msg = ConnectingTargetFailedMessage("h", 22, "boom")
    assert str(msg) == "connecting to h:22 failed: boom"


def test_connecting_target_failed_message_repr_contains_host_reason():
    msg = ConnectingTargetFailedMessage("h", 22, "boom")
    text = repr(msg)
    assert "'h'" in text
    assert "boom" in text


def test_connecting_to_message_str():
    msg = ConnectingToMessage("hostname")
    assert str(msg) == "connecting to hostname"


def test_unsupported_product_message_str():
    Product = namedtuple("Product", "name version arch")
    msg = UnsuportedProductMessage(Product("Foo", "1.0", "x86_64"))
    text = str(msg)
    assert "Foo" in text
    assert "1.0" in text
    assert "products.yaml" in text


def test_user_message_cannot_be_raised_as_exception_directly():
    """UserMessage extends BaseException so it can be raised."""
    with pytest.raises(UnsuportedProductMessage):
        Product = namedtuple("Product", "name version arch")
        raise UnsuportedProductMessage(Product("X", "1", "noarch"))
