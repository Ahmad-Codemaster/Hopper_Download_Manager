(function (root) {
  const MEDIA_EXTENSIONS = new Set([
    '.mp4', '.webm', '.mkv', '.avi', '.mov', '.flv', '.wmv',
    '.m4v', '.m4a', '.mp3', '.wav', '.aac', '.flac', '.ogg',
    '.m3u8', '.mpd', '.ts',
  ]);

  const DOWNLOAD_EXTENSIONS = new Set([
    '.zip', '.rar', '.7z', '.tar', '.gz', '.bz2', '.xz',
    '.exe', '.msi', '.iso', '.bin', '.img', '.dmg', '.pkg',
    '.apk', '.ipa', '.deb', '.rpm', '.pdf', '.epub', '.torrent',
  ]);

  const IGNORED_EXTENSIONS = new Set([
    '.png', '.jpg', '.jpeg', '.webp', '.gif', '.svg', '.ico', '.bmp',
    '.woff', '.woff2', '.ttf', '.eot', '.otf',
    '.css', '.js', '.json', '.xml', '.html', '.htm',
    // Text/data files — Google sends f.txt as a 133-byte tracking attachment;
    // these should never be captured as media (MED-01 fix).
    '.txt', '.log', '.csv',
  ]);

  // Minimum size for an audio file to be treated as real media rather than a
  // small UI sound effect (e.g. YouTube's failure.mp3 / open.mp3 are ~6 KB).
  const MIN_AUDIO_BYTES = 256 * 1024; // 256 KB

  function validateHandoff(message) {
    try {
      const url = new URL(message.url);
      return url.protocol === 'http:' || url.protocol === 'https:';
    } catch {
      return false;
    }
  }

  function redactUrl(value) {
    const url = new URL(value);
    for (const key of url.searchParams.keys()) {
      url.searchParams.set(key, '[REDACTED]');
    }
    return url.toString();
  }

  function getUrlExtension(rawUrl) {
    try {
      const parsed = new URL(rawUrl);
      const pathname = parsed.pathname;
      const lastDot = pathname.lastIndexOf('.');
      if (lastDot !== -1) {
        return pathname.slice(lastDot).toLowerCase();
      }
    } catch {
      // ignore
    }
    return '';
  }

  function isMediaUrl(url, contentType = '', contentLength = 0) {
    const ext = getUrlExtension(url);
    if (IGNORED_EXTENSIONS.has(ext)) {
      return false;
    }

    const ct = (contentType || '').toLowerCase();
    if (ct.startsWith('image/') || ct.startsWith('font/')) {
      return false;
    }

    if (MEDIA_EXTENSIONS.has(ext)) {
      // MED-01 fix: For audio-only extensions (mp3, wav, aac, ogg, flac, m4a)
      // require a minimum verified Content-Length so tiny UI sound effects
      // (e.g. YouTube's failure.mp3 at ~6 KB) are not classified as media.
      const AUDIO_ONLY_EXTS = new Set(['.mp3', '.wav', '.aac', '.ogg', '.flac', '.m4a']);
      if (AUDIO_ONLY_EXTS.has(ext) && typeof contentLength === 'number' && contentLength > 0 && contentLength < MIN_AUDIO_BYTES) {
        return false;
      }
      return true;
    }

    if (
      ct.startsWith('video/') ||
      ct.startsWith('audio/') ||
      ct.includes('application/vnd.apple.mpegurl') ||
      ct.includes('application/x-mpegurl') ||
      ct.includes('application/dash+xml')
    ) {
      return true;
    }

    const lowerUrl = url.toLowerCase();
    if (
      lowerUrl.includes('.m3u8') ||
      lowerUrl.includes('.mpd') ||
      lowerUrl.includes('mime=video') ||
      lowerUrl.includes('mime=audio') ||
      /\.(mp4|webm|mkv|m4a|mp3)(\?|$)/i.test(lowerUrl)
    ) {
      return true;
    }

    return false;
  }

  function isDownloadableUrl(url, contentDisposition = '', contentType = '', contentLength = 0) {
    const ext = getUrlExtension(url);
    if (IGNORED_EXTENSIONS.has(ext)) {
      return false;
    }

    const ct = (contentType || '').toLowerCase();
    if (ct.startsWith('image/') || ct.startsWith('font/') || ct.startsWith('text/') || ct.includes('javascript')) {
      return false;
    }

    if (DOWNLOAD_EXTENSIONS.has(ext)) {
      return true;
    }

    const cd = (contentDisposition || '').toLowerCase();
    if (cd.includes('attachment') || cd.includes('filename=')) {
      return true;
    }

    if (ct.includes('application/octet-stream')) {
      // Filter out tiny tracking/beacon pings (< 100 KB) unless explicitly designated as attachment
      if (typeof contentLength === 'number' && contentLength > 0 && contentLength < 100 * 1024 && !cd.includes('attachment')) {
        return false;
      }
      return true;
    }

    if (
      ct.includes('application/zip') ||
      ct.includes('application/x-rar-compressed') ||
      ct.includes('application/x-7z-compressed') ||
      ct.includes('application/x-iso9660-image')
    ) {
      return true;
    }

    return false;
  }

  function sanitizeTitle(title) {
    if (!title) return '';
    return title
      .replace(/[\/\\:*?"<>|]/g, '_')
      .replace(/\s+/g, ' ')
      .replace(/(?: - Watch Online.*| \| Netflix.*| - YouTube.*| \| Prime Video.*| \| Disney\+.*)$/i, '')
      .trim();
  }

  function isHashedOrGenericName(name) {
    if (!name) return true;
    const base = name.split('.')[0];
    if (base.length > 20 && !base.includes(' ') && !base.includes('-')) {
      return true;
    }
    const generic = ['master', 'index', 'playlist', 'video', 'stream', 'chunk', 'segment', 'frag', 'videoplayback', 'download'];
    return generic.includes(base.toLowerCase());
  }

  function classifyMedia(url, contentType = '') {
    const ct = (contentType || '').toLowerCase();
    const cleanUrl = url.split('?')[0].toLowerCase();
    const fullUrl = url.toLowerCase();

    if (
      ct.includes('vnd.apple.mpegurl') ||
      ct.includes('x-mpegurl') ||
      cleanUrl.endsWith('.m3u8') ||
      fullUrl.includes('.m3u8')
    ) {
      return 'hls-stream';
    }
    if (
      ct.includes('dash+xml') ||
      cleanUrl.endsWith('.mpd') ||
      fullUrl.includes('.mpd')
    ) {
      return 'dash-stream';
    }
    if (
      ct.startsWith('video/') ||
      cleanUrl.endsWith('.mp4') ||
      cleanUrl.endsWith('.webm') ||
      cleanUrl.endsWith('.mkv') ||
      cleanUrl.endsWith('.avi') ||
      cleanUrl.endsWith('.mov') ||
      cleanUrl.endsWith('.flv') ||
      cleanUrl.endsWith('.ts') ||
      fullUrl.includes('mime=video')
    ) {
      return 'video';
    }
    if (
      ct.startsWith('audio/') ||
      cleanUrl.endsWith('.mp3') ||
      cleanUrl.endsWith('.m4a') ||
      cleanUrl.endsWith('.wav') ||
      cleanUrl.endsWith('.aac') ||
      cleanUrl.endsWith('.flac') ||
      cleanUrl.endsWith('.ogg') ||
      fullUrl.includes('mime=audio')
    ) {
      return 'audio';
    }
    if (/\.(zip|rar|7z|tar|gz|iso|exe|msi|dmg|bin|apk|torrent)/i.test(cleanUrl)) {
      return 'archive-or-binary';
    }
    return 'file';
  }

  function extractFilename(url, contentDisposition = '', fallback = 'download.bin', pageTitle = '', mediaType = '') {
    if (contentDisposition) {
      const utf8Match = contentDisposition.match(/filename\*=(?:UTF-8''|utf-8'')([^;]+)/i);
      if (utf8Match && utf8Match[1]) {
        try {
          return decodeURIComponent(utf8Match[1].trim().replace(/^["']|["']$/g, ''));
        } catch {
          // fallback
        }
      }
      const standardMatch = contentDisposition.match(/filename=(?:"([^"]+)"|([^;\s]+))/i);
      if (standardMatch) {
        const match = standardMatch[1] || standardMatch[2];
        if (match) {
          return match.trim();
        }
      }
    }

    const cleanTitle = sanitizeTitle(pageTitle);

    let extracted = '';
    try {
      const parsed = new URL(url);
      const segments = parsed.pathname.split('/').filter(Boolean);
      if (segments.length > 0) {
        const last = decodeURIComponent(segments[segments.length - 1]);
        if (last && !last.startsWith('?')) {
          extracted = last;
        }
      }
    } catch {
      // fallback
    }

    const type = mediaType || classifyMedia(url);

    // If we have a page title and the URL filename looks like a hash or generic stream chunk:
    if (cleanTitle && (isHashedOrGenericName(extracted) || !extracted || !extracted.includes('.'))) {
      if (type === 'video') return `${cleanTitle}.mp4`;
      if (type === 'audio') return `${cleanTitle}.mp3`;
      if (type === 'hls-stream') return `${cleanTitle}.m3u8`;
      if (type === 'dash-stream') return `${cleanTitle}.mpd`;
      if (extracted && extracted.includes('.')) {
        const ext = extracted.slice(extracted.lastIndexOf('.'));
        return `${cleanTitle}${ext}`;
      }
      return `${cleanTitle}.mp4`;
    }

    if (extracted && extracted.includes('.')) {
      return extracted;
    }

    if (cleanTitle) {
      return `${cleanTitle}.mp4`;
    }

    return fallback;
  }

  function parseM3u8Variants(content, baseUrl) {
    const lines = content.split(/\r?\n/);
    const variants = [];
    let pendingVariant = null;

    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (line.startsWith('#EXT-X-STREAM-INF:')) {
        const info = line.substring(18);
        let resolution = 'auto';
        let bandwidth = 0;

        const resMatch = info.match(/RESOLUTION=(\d+x\d+)/i);
        if (resMatch) {
          resolution = resMatch[1];
        }

        const bwMatch = info.match(/BANDWIDTH=(\d+)/i);
        if (bwMatch) {
          bandwidth = parseInt(bwMatch[1], 10);
        }

        pendingVariant = { resolution, bandwidth };
      } else if (line && !line.startsWith('#') && pendingVariant) {
        let variantUrl = line;
        if (baseUrl && !/^https?:\/\//i.test(line)) {
          try {
            variantUrl = new URL(line, baseUrl).toString();
          } catch {
            // keep as is
          }
        }
        variants.push({
          ...pendingVariant,
          url: variantUrl,
          label: pendingVariant.resolution !== 'auto'
            ? `${pendingVariant.resolution} (${Math.round(pendingVariant.bandwidth / 1000)} kbps)`
            : `Stream (${Math.round(pendingVariant.bandwidth / 1000)} kbps)`,
        });
        pendingVariant = null;
      }
    }

    return variants;
  }

  function formatNativeCommand(command, params = {}) {
    switch (command) {
      case 'List':
        return { major: 1, minor: 0, command: 'List' };
      case 'Create':
        return {
          major: 1,
          minor: 0,
          command: {
            Create: {
              source: params.source,
              destination: params.destination || 'download.bin',
            },
          },
        };
      case 'Start':
        return { major: 1, minor: 0, command: { Start: { job_id: params.job_id } } };
      case 'Pause':
        return { major: 1, minor: 0, command: { Pause: { job_id: params.job_id } } };
      case 'Resume':
        return { major: 1, minor: 0, command: { Resume: { job_id: params.job_id } } };
      case 'Cancel':
        return { major: 1, minor: 0, command: { Cancel: { job_id: params.job_id } } };
      case 'RefreshSource':
        return {
          major: 1,
          minor: 0,
          command: {
            RefreshSource: {
              job_id: params.job_id,
              new_source: params.new_source,
            },
          },
        };
      default:
        throw new Error(`Unknown command: ${command}`);
    }
  }

  const protocolExports = {
    validateHandoff,
    redactUrl,
    getUrlExtension,
    classifyMedia,
    isMediaUrl,
    isDownloadableUrl,
    sanitizeTitle,
    isHashedOrGenericName,
    extractFilename,
    parseM3u8Variants,
    formatNativeCommand,
  };

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = protocolExports;
  }
  if (root) {
    root.HopperProtocol = protocolExports;
  }
})(typeof globalThis !== 'undefined' ? globalThis : this);
