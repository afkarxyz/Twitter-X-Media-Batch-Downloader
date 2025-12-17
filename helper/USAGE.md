# Usage Guide

CLI tool for extracting media and metadata from Twitter/X using gallery-dl extractor.

## Command Line Arguments

### Required
- `url` — Twitter/X URL (timeline, media, likes, search, etc.)
- `--auth-token TOKEN` — Your auth_token cookie value (required)

### Content Options
- `--retweets {skip|include|original}` — Control retweets (default: skip)
- `--no-videos` — Skip video downloads
- `--text-tweets` — Include tweets without media
- `--type {photo|video|animated_gif|all}` — Filter by media type (default: all)

### Quality & Format
- `--size SIZE` — Image size: `orig`, `4096x4096`, `large`, `medium`, `small` (default: orig)

### Fetch Control
- `--limit N` — Maximum media items to fetch (0 = unlimited)
- `--cursor CURSOR` — Resume from specific cursor position

### Output Options
- `--json` — Output results as JSON
- `--metadata` — Include tweet metadata (author, counts, etc.)
- `--output FILE` / `-o FILE` — Save to JSON file with resume capability
- `--resume FILE` / `-r FILE` — Resume from previous JSON file
- `--progress` — Show progress during fetch

### Advanced
- `--set KEY=VALUE` — Set gallery-dl extractor options (repeatable)
- `-v` / `--verbose` — Show detailed metadata during fetch

---

## Usage Examples

### Basic Fetch

```powershell
# Fetch media with authentication
python twitter_cli.py https://x.com/username/media --limit 10 --json --auth-token YOUR_TOKEN
```

### Media Type Filtering

```powershell
# Photos only
python twitter_cli.py https://x.com/username/media --type photo --json --auth-token YOUR_TOKEN

# Videos only
python twitter_cli.py https://x.com/username/media --type video --json --auth-token YOUR_TOKEN

# Animated GIFs only
python twitter_cli.py https://x.com/username/media --type animated_gif --json --auth-token YOUR_TOKEN
```

### With Metadata

```powershell
# Include tweet metadata (author, date, counts)
python twitter_cli.py https://x.com/username/media --limit 5 --json --metadata --auth-token YOUR_TOKEN
```

### Retweets Control

```powershell
# Skip retweets (default)
python twitter_cli.py https://x.com/username/timeline --retweets skip --json --auth-token YOUR_TOKEN

# Include retweets
python twitter_cli.py https://x.com/username/timeline --retweets include --json --auth-token YOUR_TOKEN

# Get original tweet from retweets
python twitter_cli.py https://x.com/username/timeline --retweets original --json --auth-token YOUR_TOKEN
```

### Save & Resume

```powershell
# Save to file (with resume capability)
python twitter_cli.py https://x.com/username/media --limit 100 --output results.json --progress --auth-token YOUR_TOKEN

# Resume from saved file
python twitter_cli.py https://x.com/username/media --resume results.json --limit 100 --output results.json --auth-token YOUR_TOKEN

# Fetch all remaining
python twitter_cli.py https://x.com/username/media --resume results.json --limit 0 --output results.json --auth-token YOUR_TOKEN
```

### Private Content

```powershell
# Your likes
python twitter_cli.py https://x.com/username/likes --limit 20 --json --auth-token YOUR_TOKEN

# Your bookmarks
python twitter_cli.py https://x.com/i/bookmarks --limit 20 --json --auth-token YOUR_TOKEN
```

### Search

```powershell
# Search with filters
python twitter_cli.py "https://x.com/search?q=from:username filter:media" --limit 50 --json --auth-token YOUR_TOKEN

# Date range search
python twitter_cli.py "https://x.com/search?q=from:username since:2024-01-01 until:2024-12-31" --json --auth-token YOUR_TOKEN
```

### Advanced Filtering

```powershell
# Filter by date (2024 only)
python twitter_cli.py https://x.com/username/media --json --metadata --set filter="date.year == 2024" --auth-token YOUR_TOKEN

# Filter by engagement (>1000 likes)
python twitter_cli.py https://x.com/username/media --json --metadata --set filter="favorite_count > 1000" --auth-token YOUR_TOKEN

# Combine filters (photos from 2024 with >100 likes)
python twitter_cli.py https://x.com/username/media --type photo --json --metadata --set filter="date.year == 2024 and favorite_count > 100" --auth-token YOUR_TOKEN
```

### Text-Only Tweets

```powershell
# Fetch tweets without media
python twitter_cli.py https://x.com/username/tweets --text-tweets --json --metadata --auth-token YOUR_TOKEN
```

---

## Output Format

### JSON Structure

```json
{
  "media": [
    {
      "url": "https://pbs.twimg.com/media/...",
      "tweet_id": 1234567890,
      "extension": "jpg",
      "width": 2048,
      "height": 1536,
      "type": "photo"
    }
  ],
  "metadata": [
    {
      "tweet_id": 1234567890,
      "date": "2024-01-15T10:30:00+00:00",
      "author": {
        "id": 123456,
        "name": "username",
        "nick": "Display Name"
      },
      "content": "Tweet text content",
      "favorite_count": 150,
      "retweet_count": 25,
      "reply_count": 5,
      "view_count": 5000
    }
  ],
  "cursor": "1/DAABCgABG7OZH1D___0KAAIW...",
  "total": 100,
  "completed": false
}
```

### Resume File Structure

When using `--output`, the file includes:
- `url` — Original URL
- `cursor` — Position for resume
- `total` — Total items fetched
- `completed` — Whether fetch is complete
- `media` — All media items
- `metadata` — All metadata (if requested)

---

## Pagination & Resume

### How It Works

- Twitter API returns ~100-200 items per batch
- Cursor tracks position for next fetch
- System ensures cursor availability before stopping
- Resume continues from exact position (no duplicates)

### Resume Workflow

```powershell
# Step 1: Initial fetch (saves state)
python twitter_cli.py https://x.com/user/media --limit 100 --output data.json --progress --auth-token YOUR_TOKEN
# Output: Saved 202 media to data.json [partial (can resume)]

# Step 2: Resume and fetch more
python twitter_cli.py https://x.com/user/media --resume data.json --limit 100 --output data.json --auth-token YOUR_TOKEN
# Output: Resuming from cursor, 202 media already fetched
#         Added 100 new media (total: 302)

# Step 3: Fetch all remaining
python twitter_cli.py https://x.com/user/media --resume data.json --limit 0 --output data.json --auth-token YOUR_TOKEN
# Output: Added 244 new media (total: 546) [completed]
```

---

## Supported URL Patterns

### User Timelines
- `https://x.com/username/media` — Media timeline
- `https://x.com/username/timeline` — Full timeline
- `https://x.com/username/tweets` — Tweets only
- `https://x.com/username/with_replies` — With replies
- `https://x.com/username/likes` — Liked tweets

### Private Content
- `https://x.com/i/bookmarks` — Your bookmarks

### Search
- `https://x.com/search?q=QUERY` — Search tweets
- `https://x.com/hashtag/NAME` — Hashtag search

### Individual Tweet
- `https://x.com/username/status/1234567890` — Single tweet

---

## Search Operators

Use in search URLs:

| Operator | Example | Description |
|----------|---------|-------------|
| `from:USER` | `from:username` | Tweets from user |
| `to:USER` | `to:username` | Tweets mentioning user |
| `filter:media` | `cats filter:media` | Only with media |
| `filter:images` | `cats filter:images` | Only images |
| `filter:videos` | `cats filter:videos` | Only videos |
| `since:DATE` | `since:2024-01-01` | After date |
| `until:DATE` | `until:2024-12-31` | Before date |
| `-filter:retweets` | `cats -filter:retweets` | Exclude retweets |
| `min_faves:N` | `min_faves:1000` | Min likes |
| `min_retweets:N` | `min_retweets:100` | Min retweets |

---

## Tips & Tricks

### 1. Progressive Fetch with Resume
```powershell
# Fetch in batches to avoid long waits
python twitter_cli.py URL --limit 200 --output data.json --progress --auth-token YOUR_TOKEN
python twitter_cli.py URL --resume data.json --limit 200 --output data.json --auth-token YOUR_TOKEN
```

### 2. Combine Type Filter with Custom Filter
```powershell
# Photos from 2024 with high engagement
python twitter_cli.py URL --type photo --metadata --set filter="date.year == 2024 and favorite_count > 500" --json --auth-token YOUR_TOKEN
```

### 3. Use Progress Flag for Long Fetches
```powershell
python twitter_cli.py URL --limit 1000 --progress --output data.json --auth-token YOUR_TOKEN
```

### 4. Check Cursor Availability
- If `completed: false` in output → can resume
- If `completed: true` → all data fetched
- If `cursor: null` → no more data available
