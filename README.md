# rusty-dlp-bot

Telegram bot that downloads videos and audio from various platforms.

## Running

Copy `.env.example` to `.env` and fill in the values.
Then start with Docker Compose:

```sh
docker compose up -d
```

## Supported sources

| Source | Type |
|--------|------|
| Reddit | Video |
| TikTok | Video |
| Instagram | Video |
| YouTube Music | Audio (MP3) |
| SoundCloud | Audio (MP3) |

## Requirements

- [yt-dlp](https://github.com/yt-dlp/yt-dlp)
- [ffmpeg](https://ffmpeg.org)

Both are included in the Docker image.
