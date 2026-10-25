# rusty-dlp-bot

Telegram bot that downloads videos and audio by links, using [yt-dlp](https://github.com/yt-dlp/yt-dlp).

Video: YouTube, Reddit, TikTok, Instagram. Audio (MP3): YouTube Music, SoundCloud, and Spotify,
Deezer, Apple Music, Tidal via the bundled [MusicLink](https://musiclink.one) plugin.

## Running

```sh
cp .env.example .env  # fill in the values, all variables are described there
docker compose up -d
```

Outside Docker you need `yt-dlp`, `ffmpeg` and Node.js (`--js-runtimes node` in the yt-dlp
config). For Spotify and the like, put [`yt-dlp-musiclink-extractor.py`](yt-dlp-musiclink-extractor.py)
at `<plugin dir>/musiclink/yt_dlp_plugins/extractor/musiclink.py` (see yt-dlp's
[plugin directories](https://github.com/yt-dlp/yt-dlp#installing-plugins)).

## Configuration

`config.toml` holds the admins, the supported hosts (`[sources]`, exact host → `video` | `audio`)
and `max_file_size_mb` (50 MB is the Telegram limit; bigger files are rejected).

## Commands

Everyone: `/start`, `/help`, `/sources`.

Admins: `/addsource <host> <video|audio>`, `/removesource <host>`, `/admins`,
`/addadmin <@username>`, `/removeadmin <@username>`, `/update` (runs `RUSTY_DLP_UPDATE_COMMAND`),
`/setcookies` (as the caption of a `cookies.txt`, or as a reply to one).
