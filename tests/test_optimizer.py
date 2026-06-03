"""PrxyClaude · Tests for Optimizer"""

from __future__ import annotations

from providers.common.optimizer import is_quota_probe, is_title_generation


def test_quota_probe_detection():
    """Test that quota probe messages are detected."""
    messages = [{"role": "user", "content": "What is my current quota?"}]
    assert is_quota_probe(messages) is True


def test_title_generation_detection():
    """Test that title generation messages are detected."""
    messages = [{"role": "user", "content": "Generate a title for this conversation."}]
    assert is_title_generation(messages) is True


def test_normal_message_not_detected():
    """Test that normal messages are not detected as special."""
    messages = [{"role": "user", "content": "Hello, how are you?"}]
    assert is_quota_probe(messages) is False
    assert is_title_generation(messages) is False
