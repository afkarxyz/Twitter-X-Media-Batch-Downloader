"""Command-line helper for gallery-dl's Twitter extractor.

Features:
- Cursor-based resume: Use --output to save progress, --resume to continue
- Progress tracking: See fetch progress in real-time
- Rate limit: Automatically handled by gallery-dl
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Dict, List, Optional

sys.path.append(os.path.dirname(os.path.abspath(__file__)))

from twitter_common import (  # type: ignore
    TwitterRequest,
    coerce_literal,
    load_resume_state,
    merge_options,
    run_request_dict,
    save_state,
)

DEFAULT_AUTH_TOKEN = ""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Light-weight CLI wrapper around gallery-dl's Twitter extractor",
    )
    parser.add_argument("url", help="Any supported Twitter/X URL (timeline, media, likes, ...)")
    parser.add_argument(
        "--auth-token",
        default=DEFAULT_AUTH_TOKEN,
        help="Value of the auth_token cookie (leave empty to skip)",
    )
    parser.add_argument(
        "--guest",
        action="store_true",
        help="Force guest mode (do not send auth_token)",
    )
    parser.add_argument(
        "--retweets",
        choices=["skip", "include", "original"],
        default="skip",
        help="Control retweet handling (skip, include, or swap for originals)",
    )
    parser.add_argument(
        "--no-videos",
        action="store_true",
        help="Skip video/animated GIF variants",
    )
    parser.add_argument(
        "--size",
        default="orig",
        help="Image size to request (orig, 4096x4096, large, ...)",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=0,
        help="Stop after N media files (0 = all)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Emit JSON instead of plain URLs",
    )
    parser.add_argument(
        "--metadata",
        action="store_true",
        help="Include tweet metadata in output",
    )
    parser.add_argument(
        "--text-tweets",
        action="store_true",
        help="Include text tweets (without media)",
    )
    parser.add_argument(
        "--type",
        choices=["photo", "video", "animated_gif", "all"],
        default="all",
        help="Filter by media type (photo, video, animated_gif, or all)",
    )
    parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Echo tweet metadata to stderr while crawling",
    )
    parser.add_argument(
        "--set",
        metavar="KEY=VALUE",
        action="append",
        default=[],
        help="Add override for extractor.twitter.* options (repeatable)",
    )
    parser.add_argument(
        "--output",
        "-o",
        metavar="FILE",
        help="Save results to JSON file (includes cursor for resume)",
    )
    parser.add_argument(
        "--resume",
        "-r",
        metavar="FILE",
        help="Resume from previous JSON file (continues from saved cursor)",
    )
    parser.add_argument(
        "--cursor",
        metavar="CURSOR",
        help="Resume from specific cursor position",
    )
    parser.add_argument(
        "--progress",
        action="store_true",
        help="Show progress during fetch",
    )
    return parser.parse_args()


def _retweets_value(mode: str):
    if mode == "original":
        return "original"
    if mode == "include":
        return True
    return False


def _parse_overrides(entries: List[str]) -> Dict[str, object]:
    overrides: Dict[str, object] = {}
    for entry in entries:
        if "=" not in entry:
            raise ValueError(f"Format must be KEY=VALUE, got: {entry}")
        key, raw = entry.split("=", 1)
        overrides[key.strip()] = coerce_literal(raw.strip())
    return overrides


def _progress_callback(count: int, cursor: Optional[str]) -> None:
    """Print progress to stderr."""
    cursor_info = f" (cursor: {cursor[:20]}...)" if cursor and len(cursor) > 20 else ""
    cursor_info = f" (cursor: {cursor})" if cursor and len(cursor) <= 20 else cursor_info
    print(f"Fetching... {count} media{cursor_info}", file=sys.stderr)


def main() -> None:
    args = parse_args()
    
    # Handle resume from file
    resume_cursor: Optional[str] = None
    previous_media: List[Dict] = []
    previous_metadata: List[Dict] = []
    seen_urls: set = set()
    url = args.url
    
    # Convert --type to filter expression
    type_filter = None
    if args.type != "all":
        type_filter = f"type == '{args.type}'"
    
    if args.resume:
        state = load_resume_state(args.resume)
        if state:
            if state.get("completed"):
                print(f"Previous fetch already completed ({state.get('total', 0)} media)", file=sys.stderr)
                print("Use --output to start fresh or remove the file", file=sys.stderr)
                sys.exit(0)
            resume_cursor = state.get("cursor")
            previous_media = state.get("media", [])
            previous_metadata = state.get("metadata", [])
            # Track seen URLs to avoid duplicates
            seen_urls = {m.get("url") for m in previous_media if m.get("url")}
            url = state.get("url", args.url)
            
            if resume_cursor:
                print(f"Resuming from cursor, {len(previous_media)} media already fetched", file=sys.stderr)
            else:
                print(f"No cursor available, will deduplicate {len(previous_media)} existing media", file=sys.stderr)
        else:
            print(f"Could not load resume file: {args.resume}", file=sys.stderr)
            sys.exit(1)
    
    # Manual cursor override
    if args.cursor:
        resume_cursor = args.cursor
    
    include_videos = not args.no_videos
    auth_token = None if args.guest or not args.auth_token.strip() else args.auth_token
    
    # Parse user overrides
    user_overrides = _parse_overrides(args.set)
    
    # Add type filter if specified (merge with existing filter if any)
    if type_filter:
        existing_filter = user_overrides.get("filter")
        if existing_filter:
            # Combine with AND
            user_overrides["filter"] = f"({existing_filter}) and ({type_filter})"
        else:
            user_overrides["filter"] = type_filter
    
    options = merge_options(
        {
            "auth_token": auth_token,
            "retweets": _retweets_value(args.retweets),
            "videos": include_videos,
            "size": args.size,
            "text-tweets": args.text_tweets,
        },
        user_overrides,
    )

    request = TwitterRequest(
        url=url,
        limit=args.limit,
        metadata=(args.metadata or args.verbose),
        options=options,
        cursor=resume_cursor,
    )

    progress_cb = _progress_callback if (args.progress or args.verbose) else None
    
    # Pass seen_urls for deduplication if no cursor available
    skip_urls = seen_urls if (not resume_cursor and seen_urls) else None

    try:
        result = run_request_dict(request, on_progress=progress_cb, skip_urls=skip_urls)
    except KeyboardInterrupt:
        print("\nInterrupted by user", file=sys.stderr)
        sys.exit(130)
    except Exception as exc:
        print(f"Error: {exc}", file=sys.stderr)
        sys.exit(1)

    # Merge with previous results if resuming
    if previous_media:
        if resume_cursor:
            # With cursor: filter duplicates that might occur at boundary
            new_media = [m for m in result["media"] if m.get("url") not in seen_urls]
            result["media"] = previous_media + new_media
            result["metadata"] = previous_metadata + result["metadata"]
        else:
            # Without cursor: results already deduplicated via skip_urls
            result["media"] = previous_media + result["media"]
            result["metadata"] = previous_metadata + result["metadata"]
        
        result["total"] = len(result["media"])
        new_count = result["total"] - len(previous_media)
        
        if new_count > 0:
            print(f"Added {new_count} new media (total: {result['total']})", file=sys.stderr)
        else:
            print(f"No new media found", file=sys.stderr)

    media = result["media"]
    metadata = result["metadata"]

    # Save to file if --output specified
    if args.output:
        save_state(args.output, result, url)
        status = "completed" if result.get("completed") else "partial (can resume)"
        print(f"Saved {len(media)} media to {args.output} [{status}]", file=sys.stderr)
        if not result.get("completed") and result.get("cursor"):
            print(f"Resume with: --resume {args.output}", file=sys.stderr)

    if args.verbose:
        for entry in metadata:
            author = (entry.get("author") or {}).get("name")
            print(
                f"Processing tweet {entry.get('tweet_id')} / {author}",
                file=sys.stderr,
            )

    if args.json:
        payload: Dict[str, object] = {
            "media": media,
            "total": len(media),
            "completed": result.get("completed", True),
        }
        if result.get("cursor"):
            payload["cursor"] = result["cursor"]
        if args.metadata:
            payload["metadata"] = metadata
        print(json.dumps(payload, indent=2, default=str))
    else:
        for item in media:
            print(item["url"])
        if args.metadata:
            print("\n# Metadata", file=sys.stderr)
            for entry in metadata:
                print(
                    f"{entry['tweet_id']}: {entry['content']}",
                    file=sys.stderr,
                )
        
        # Print summary
        print(f"\nTotal: {len(media)} media", file=sys.stderr)
        if not result.get("completed") and result.get("cursor"):
            print(f"Cursor for resume: {result['cursor']}", file=sys.stderr)


if __name__ == "__main__":
    main()
