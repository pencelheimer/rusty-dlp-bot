"""yt-dlp extractor plugin: resolve Spotify / Deezer / Apple Music / Tidal
track links to YouTube Music via the MusicLink API, then hand off to the
built-in YouTube extractor.

Install:
  <plugin dir>/musiclink/yt_dlp_plugins/extractor/musiclink.py

API key (either):
  export RUSTY_DLP_MUSICLINK_API_KEY=...
  yt-dlp --extractor-args "musiclink:api_key=..." URL

Resolved links are cached in yt-dlp's cache dir to save API quota
(disable with --no-cache-dir).
"""

import hashlib
import os

from yt_dlp.extractor.common import InfoExtractor
from yt_dlp.utils import ExtractorError, traverse_obj


class MusicLinkIE(InfoExtractor):
    IE_NAME = 'musiclink'
    IE_DESC = 'Resolve streaming-service track links to YouTube Music via MusicLink'
    # One pattern per service; MusicLink validates the rest.
    _VALID_URL = '|'.join((
        r'https?://open\.spotify\.com/(?:[\w-]+/)?track/\w+',
        r'https?://(?:www\.)?deezer\.com/(?:\w+/)?track/\d+',
        r'https?://music\.apple\.com/\w+/(?:song/(?:[^/?#]+/)?\d+|album/[^/?#]+/\d+\?(?:[^#]*&)?i=\d+)',
        r'https?://(?:listen\.)?tidal\.com/(?:browse/)?track/\d+',
    ))
    _API_URL = 'https://api.musiclink.one/v2/resolve'
    _CACHE_SECTION = 'musiclink'

    _TESTS = [{
        'url': 'https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT',
        'only_matching': True,
    }, {
        'url': 'https://www.deezer.com/track/14408104',
        'only_matching': True,
    }]

    def _api_key(self):
        key = self._configuration_arg('api_key', [None], casesense=True)[0]
        key = key or os.environ.get('RUSTY_DLP_MUSICLINK_API_KEY')
        if not key:
            raise ExtractorError(
                'No MusicLink API key: set RUSTY_DLP_MUSICLINK_API_KEY or use '
                '--extractor-args "musiclink:api_key=KEY"', expected=True)
        return key

    def _resolve(self, url):
        cache_key = hashlib.sha1(url.split('#')[0].encode()).hexdigest()
        cached = self.cache.load(self._CACHE_SECTION, cache_key)
        if cached:
            self.write_debug('Using cached MusicLink result')
            return cached

        resp = self._download_json(
            self._API_URL, cache_key, 'Resolving via MusicLink',
            query={'q': url},
            headers={'Authorization': f'Bearer {self._api_key()}'})

        item = traverse_obj(resp, ('data', 0, {dict}))
        if not item:
            raise ExtractorError('MusicLink found no match for this track', expected=True)
        self.cache.store(self._CACHE_SECTION, cache_key, item)
        return item

    def _real_extract(self, url):
        item = self._resolve(url)
        platforms = item.get('platforms') or {}
        target = platforms.get('youtube_music') or platforms.get('youtube')
        if not target:
            raise ExtractorError('Track is not available on YouTube / YouTube Music', expected=True)

        # Hand off to the YouTube extractor; keep the cleaner source metadata.
        return self.url_result(
            target, 'Youtube', url_transparent=True,
            title=item.get('title'),
            track=item.get('title'),
            artists=[item['artist']] if item.get('artist') else None)
