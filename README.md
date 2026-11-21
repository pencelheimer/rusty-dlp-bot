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
and `max_file_size_mb` (bigger files are rejected; defaults to the upload limit of the Bot API
server: 50 MB for the cloud one, 2000 MB for a local one).

## Local Bot API server

A self-hosted [telegram-bot-api](https://github.com/tdlib/telegram-bot-api) server in `--local`
mode lifts the upload limit from 50 MB to 2000 MB. `compose.yml` has one under the `local-api`
profile:

1. Get an `api_id` and `api_hash` at <https://my.telegram.org/apps>.
2. Log the bot out of the cloud Bot API, otherwise the local server can't take over (switching
   back is possible only 10 minutes later):
   `curl https://api.telegram.org/bot<token>/logOut`
3. Uncomment the local server block in `.env` and run `docker compose up -d`.

`RUSTY_DLP_API_URL` points the bot at the server. `RUSTY_DLP_API_LOCAL` tells it that the server
shares its filesystem: files are sent by path instead of being uploaded, and files from `getFile`
are read from the server's working directory, so both directories have to be mounted at the same
paths in both containers (as in `compose.yml`).

## Commands

Everyone: `/start`, `/help`, `/sources`.

Admins: `/addsource <host> <video|audio>`, `/removesource <host>`, `/admins`,
`/addadmin <@username>`, `/removeadmin <@username>`, `/update` (runs `RUSTY_DLP_UPDATE_COMMAND`),
`/setcookies` (as the caption of a `cookies.txt`, or as a reply to one).
