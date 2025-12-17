"""Shared abstraction for all Twitter gallery-dl examples.

This file provides utility functions that build gallery-dl configuration
based on parameters in ``twitter-parameters.md`` and execute the Twitter
extractor in a single run. All four interfaces (direct module, CLI, FastAPI, and
Flask) rely on this module to ensure consistent behavior.

Features:
- Cursor-based resume: Save progress and continue from where you left off
- Progress tracking: Monitor fetch progress in real-time
- Rate limit handling: Automatically handled by gallery-dl
"""

from __future__ import annotations

import json
import os
import sys
import threading
from dataclasses import dataclass, field
from datetime import date, datetime
from typing import Any, Callable, Dict, Iterable, List, MutableMapping, Optional

REPO_ROOT = os.path.dirname(os.path.abspath(__file__))
if REPO_ROOT not in sys.path:
    sys.path.append(REPO_ROOT)

from gallery_dl import config, extractor as extractor_mod  # type: ignore
from gallery_dl.extractor.common import Message  # type: ignore

_CONFIG_LOCK = threading.Lock()

# Internal keys that should not be directly passed to extractor config.
_INTERNAL_KEYS = {
    "url",
    "limit",
    "metadata",
    "options",
    "auth_token",
    "guest",
    "cookies",
    "cursor",
}


@dataclass
class TwitterRequest:
    """Standard request structure for all frontends."""

    url: str
    options: Dict[str, Any] = field(default_factory=dict)
    limit: int = 0
    metadata: bool = False
    cursor: Optional[str] = None  # Resume from this cursor position


@dataclass
class TwitterResult:
    """Result structure with cursor for resume capability."""

    media: List[Dict[str, Any]]
    metadata: List[Dict[str, Any]]
    cursor: Optional[str] = None  # Cursor to resume from
    total: int = 0
    completed: bool = True  # False if stopped before completion


def _serialize_value(value: Any) -> Any:
    if isinstance(value, (datetime, date)):
        return value.isoformat()
    if isinstance(value, set):
        return sorted(value)
    return value


def _extract_tweet_metadata(data: MutableMapping[str, Any]) -> Dict[str, Any]:
    keys = (
        "tweet_id",
        "retweet_id",
        "quote_id",
        "reply_id",
        "conversation_id",
        "date",
        "author",
        "content",
        "lang",
        "hashtags",
        "mentions",
        "sensitive",
        "sensitive_flags",
        "favorite_count",
        "retweet_count",
        "quote_count",
        "reply_count",
        "bookmark_count",
        "view_count",
    )
    meta = {key: _serialize_value(data.get(key)) for key in keys}
    if isinstance(meta["author"], dict):
        meta["author"] = {
            "id": meta["author"].get("id"),
            "name": meta["author"].get("name"),
            "nick": meta["author"].get("nick"),
        }
    elif isinstance(meta["author"], str):
        # Handle case where author is just a string (username)
        meta["author"] = {
            "id": None,
            "name": meta["author"],
            "nick": meta["author"],
        }
    else:
        # Handle None or other types
        meta["author"] = {
            "id": None,
            "name": None,
            "nick": None,
        }
    return meta


def _clean_file_metadata(meta: MutableMapping[str, Any]) -> Dict[str, Any]:
    return {key: _serialize_value(value) for key, value in meta.items()}


def _apply_options(options: Dict[str, Any], cursor: Optional[str] = None) -> None:
    """Reset config and apply all extractor options."""

    config.clear()
    cookies = options.get("cookies")
    auth_token = options.get("auth_token")
    if auth_token:
        cookies = dict(cookies or {})
        cookies["auth_token"] = auth_token
    if cookies:
        config.set(("extractor", "twitter"), "cookies", cookies)

    # Enable cursor tracking, or set specific cursor for resume
    if cursor:
        config.set(("extractor", "twitter"), "cursor", cursor)
    else:
        config.set(("extractor", "twitter"), "cursor", True)

    for key, value in options.items():
        if key in _INTERNAL_KEYS or value is None:
            continue
        config.set(("extractor", "twitter"), key, value)


def run_request(
    request: TwitterRequest,
    on_progress: Optional[Callable[[int, Optional[str]], None]] = None,
    skip_urls: Optional[set] = None,
    ensure_cursor: bool = True,
) -> TwitterResult:
    """Run Twitter extractor and return media & metadata results.
    
    Args:
        request: The TwitterRequest configuration
        on_progress: Optional callback(count, cursor) called periodically during fetch
        skip_urls: Optional set of URLs to skip (for resume/deduplication)
        ensure_cursor: If True, continue fetching until cursor is available (for reliable resume)
    
    Returns:
        TwitterResult with media, metadata, cursor for resume, and completion status
    """
    skip_urls = skip_urls or set()

    with _CONFIG_LOCK:
        _apply_options(request.options, request.cursor)
        extractor = extractor_mod.find(request.url)
        if extractor is None or extractor.category != "twitter":
            raise ValueError(f"URL not recognized by Twitter extractor: {request.url}")

        media: List[Dict[str, Any]] = []
        metadata: List[Dict[str, Any]] = []
        collected = 0
        skipped = 0
        last_cursor: Optional[str] = None
        last_tweet_id: Optional[int] = None
        completed = True
        limit_reached = False

        try:
            for message in extractor:
                mtype = message[0]
                if mtype is Message.Directory and request.metadata:
                    # Only extract metadata if message[1] is a dict
                    if isinstance(message[1], dict):
                        metadata.append(_extract_tweet_metadata(message[1]))
                elif mtype is Message.Url:
                    url = message[1]
                    
                    # Skip if already seen
                    if url in skip_urls:
                        skipped += 1
                        continue
                    
                    file_meta = _clean_file_metadata(message[2])
                    
                    # Apply client-side filter if specified
                    # Note: gallery-dl's filter only works for downloads, not message iteration
                    filter_expr = request.options.get("filter")
                    if filter_expr:
                        try:
                            # Create evaluation context with file metadata
                            eval_context = {**file_meta}
                            # Add common functions that might be used in filters
                            eval_context['datetime'] = datetime
                            eval_context['date'] = date
                            
                            # Evaluate filter expression
                            if not eval(filter_expr, {"__builtins__": {}}, eval_context):
                                continue  # Skip this item
                        except Exception as e:
                            # If filter evaluation fails, include the item (fail-open)
                            pass
                    
                    media.append({"url": url, **file_meta})
                    collected += 1
                    
                    # Track last tweet_id for progress display
                    if "tweet_id" in file_meta:
                        last_tweet_id = file_meta["tweet_id"]
                    
                    # Try to get cursor from extractor
                    if hasattr(extractor, '_cursor') and extractor._cursor:
                        last_cursor = extractor._cursor
                    
                    # Report progress every 10 items
                    if on_progress and collected % 10 == 0:
                        on_progress(collected, last_cursor)
                    
                    # Check if limit reached
                    if request.limit and collected >= request.limit:
                        if ensure_cursor and not last_cursor:
                            # Continue until cursor is available for reliable resume
                            limit_reached = True
                            continue
                        completed = False  # Stopped due to limit
                        break
                    
                    # If we were waiting for cursor and now have it, stop
                    if limit_reached and last_cursor:
                        completed = False
                        break
                        
        except KeyboardInterrupt:
            completed = False
        finally:
            # Get final cursor from extractor
            if hasattr(extractor, '_cursor') and extractor._cursor:
                last_cursor = extractor._cursor

        return TwitterResult(
            media=media,
            metadata=metadata,
            cursor=last_cursor,
            total=collected,
            completed=completed,
        )


def run_request_dict(
    request: TwitterRequest,
    on_progress: Optional[Callable[[int, Optional[str]], None]] = None,
    skip_urls: Optional[set] = None,
    ensure_cursor: bool = True,
) -> Dict[str, Any]:
    """Run request and return as dictionary (for JSON serialization)."""
    result = run_request(request, on_progress, skip_urls, ensure_cursor)
    return {
        "media": result.media,
        "metadata": result.metadata,
        "cursor": result.cursor,
        "total": result.total,
        "completed": result.completed,
    }


def load_resume_state(filepath: str) -> Optional[Dict[str, Any]]:
    """Load previous state from JSON file for resume."""
    if not os.path.exists(filepath):
        return None
    try:
        with open(filepath, "r", encoding="utf-8") as f:
            data = json.load(f)
        return data
    except (json.JSONDecodeError, IOError):
        return None


def save_state(filepath: str, result: Dict[str, Any], url: str) -> None:
    """Save current state to JSON file."""
    state = {
        "url": url,
        "cursor": result.get("cursor"),
        "total": result.get("total", len(result.get("media", []))),
        "completed": result.get("completed", True),
        "media": result.get("media", []),
        "metadata": result.get("metadata", []),
    }
    with open(filepath, "w", encoding="utf-8") as f:
        json.dump(state, f, indent=2, default=str)


def merge_options(*sources: Optional[Dict[str, Any]]) -> Dict[str, Any]:
    """Merge multiple configuration dicts (None values ignored)."""

    merged: Dict[str, Any] = {}
    for source in sources:
        if not source:
            continue
        for key, value in source.items():
            merged[key] = value
    return merged


def coerce_literal(value: str) -> Any:
    """Infer basic type from CLI argument string."""

    lowered = value.lower()
    if lowered in {"true", "false"}:
        return lowered == "true"
    if lowered in {"null", "none"}:
        return None
    try:
        return int(value)
    except ValueError:
        pass
    try:
        return float(value)
    except ValueError:
        pass
    if (value.startswith("[") and value.endswith("]")) or (
        value.startswith("{") and value.endswith("}")
    ):
        try:
            return json.loads(value)
        except json.JSONDecodeError:
            return value
    return value
