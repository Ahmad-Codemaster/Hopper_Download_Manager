const test = require('node:test');
const assert = require('node:assert/strict');
const {
  validateHandoff,
  redactUrl,
  isMediaUrl,
  isDownloadableUrl,
  classifyMedia,
  extractFilename,
  parseM3u8Variants,
  formatNativeCommand,
  isSupportedYouTubeFormat,
} = require('../src/protocol');
const { shouldHandoff, executeHandoff } = require('../src/background');

test('accepts authorized HTTP handoff and redacts query values', () => {
  assert.equal(validateHandoff({ url: 'https://example.test/file?token=secret' }), true);
  assert.equal(redactUrl('https://example.test/file?token=secret'), 'https://example.test/file?token=%5BREDACTED%5D');
});

test('rejects unsupported handoff schemes', () => {
  assert.equal(validateHandoff({ url: 'javascript:alert(1)' }), false);
});

test('requires explicit consent before handoff', () => {
  assert.equal(shouldHandoff({ url: 'https://example.test/file' }), false);
  assert.equal(shouldHandoff({ url: 'https://example.test/file' }, true), true);
});

test('identifies media urls by extension and content-type', () => {
  assert.equal(isMediaUrl('https://cdn.example.com/video.mp4'), true);
  assert.equal(isMediaUrl('https://cdn.example.com/stream.m3u8'), true);
  assert.equal(isMediaUrl('https://cdn.example.com/audio.m4a'), true);
  assert.equal(isMediaUrl('https://example.com/stream?id=123', 'video/mp4'), true);
  assert.equal(isMediaUrl('https://example.com/manifest', 'application/vnd.apple.mpegurl'), true);
  assert.equal(isMediaUrl('https://example.com/index.html', 'text/html'), false);
});

test('identifies downloadable files by extension, disposition, and mime', () => {
  assert.equal(isDownloadableUrl('https://example.com/setup.exe'), true);
  assert.equal(isDownloadableUrl('https://example.com/archive.zip'), true);
  assert.equal(isDownloadableUrl('https://example.com/disk.iso'), true);
  assert.equal(isDownloadableUrl('https://example.com/data', 'attachment; filename="report.pdf"'), true);
  assert.equal(isDownloadableUrl('https://example.com/blob', '', 'application/octet-stream'), true);
  assert.equal(isDownloadableUrl('https://example.com/style.css', '', 'text/css'), false);
});

test('filters out image assets and tiny octet-stream telemetry pings', () => {
  assert.equal(isMediaUrl('https://example.com/banner.png', 'image/png'), false);
  assert.equal(isMediaUrl('https://example.com/thumb.webp', 'image/webp'), false);
  assert.equal(isDownloadableUrl('https://example.com/pixel.png', '', 'image/png'), false);
  // 255-byte ping with octet-stream should be filtered out
  assert.equal(isDownloadableUrl('https://example.com/ping', '', 'application/octet-stream', 255), false);
  // Large binary with octet-stream should be allowed
  assert.equal(isDownloadableUrl('https://example.com/firmware.bin', '', 'application/octet-stream', 5 * 1024 * 1024), true);
});

test('extracts filename from content-disposition, url, and replaces random hashes with page title', () => {
  assert.equal(
    extractFilename('https://example.com/file', 'attachment; filename="archive_2026.zip"'),
    'archive_2026.zip'
  );
  assert.equal(
    extractFilename('https://example.com/downloads/game_setup.iso'),
    'game_setup.iso'
  );
  assert.equal(
    extractFilename('https://example.com/downloads/', '', 'default.bin'),
    'default.bin'
  );
  // Hashed URL with video should use page title
  assert.equal(
    extractFilename(
      'https://cdn.example.com/2sed-qjYxbC3UNEcc2JZaHdzPxpi',
      '',
      'download.bin',
      'The Strangers Chapter 1 - Watch Online Free',
      'video'
    ),
    'The Strangers Chapter 1.mp4'
  );
});

test('parses HLS m3u8 playlist variant streams and resolutions', () => {
  const m3u8Sample = `#EXTM3U
#EXT-X-VERSION:3
#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080
1080p/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2500000,RESOLUTION=1280x720
720p/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360
360p/index.m3u8`;

  const variants = parseM3u8Variants(m3u8Sample, 'https://cdn.example.com/hls/master.m3u8');
  assert.equal(variants.length, 3);
  assert.equal(variants[0].resolution, '1920x1080');
  assert.equal(variants[0].url, 'https://cdn.example.com/hls/1080p/index.m3u8');
  assert.equal(variants[1].resolution, '1280x720');
  assert.equal(variants[1].url, 'https://cdn.example.com/hls/720p/index.m3u8');
});

test('formats valid native commands for download-host', () => {
  const createCmd = formatNativeCommand('Create', {
    source: 'https://example.com/file.zip',
    destination: 'file.zip',
  });
  assert.deepEqual(createCmd, {
    major: 1,
    minor: 0,
    command: {
      Create: {
        source: 'https://example.com/file.zip',
        destination: 'file.zip',
      },
    },
  });

  const refreshCmd = formatNativeCommand('RefreshSource', {
    job_id: 'job-123',
    new_source: 'https://example.com/file_refreshed.zip',
  });
  assert.deepEqual(refreshCmd, {
    major: 1,
    minor: 0,
    command: {
      RefreshSource: {
        job_id: 'job-123',
        new_source: 'https://example.com/file_refreshed.zip',
      },
    },
  });
});

// ── MED-01 regression tests ──────────────────────────────────────────────────

test('MED-01: .txt files are excluded from downloadable classification (f.txt fix)', () => {
  // Google's f.txt tracking beacon (133 bytes, served with content-disposition: attachment)
  // must NOT be classified as a downloadable item.
  assert.equal(
    isDownloadableUrl(
      'https://www.google.com/gen_204?f.txt',
      'attachment; filename="f.txt"',
      'text/plain',
      133
    ),
    false,
    '.txt attachment must be filtered by IGNORED_EXTENSIONS'
  );
  assert.equal(
    isDownloadableUrl('https://example.com/readme.txt', '', 'text/plain'),
    false
  );
});

test('MED-01: small audio files (< 256 KB) are excluded as media (UI sound effects fix)', () => {
  // YouTube loads failure.mp3 (~6 KB) and open.mp3 (~6 KB) as UI sound effects.
  // These must be filtered out by the minimum audio size check.
  assert.equal(
    isMediaUrl('https://www.youtube.com/s/desktop/sounds/failure.mp3', 'audio/mpeg', 6 * 1024),
    false,
    'failure.mp3 at 6 KB must not be classified as media'
  );
  assert.equal(
    isMediaUrl('https://cdn.example.com/ui/open.mp3', 'audio/mpeg', 8 * 1024),
    false,
    'open.mp3 at 8 KB must not be classified as media'
  );
});

test('MED-01: large audio files (>= 256 KB) still classified as media', () => {
  // A genuine podcast or music track must still pass through.
  assert.equal(
    isMediaUrl('https://cdn.example.com/podcast_episode.mp3', 'audio/mpeg', 10 * 1024 * 1024),
    true,
    '10 MB mp3 must still be classified as media'
  );
  // When content-length is unknown (0 / not provided), default to allowing it.
  assert.equal(
    isMediaUrl('https://cdn.example.com/track.mp3', 'audio/mpeg', 0),
    true,
    'mp3 without content-length must not be blocked'
  );
});

test('CRIT-03: identifies YouTube videoplayback stream chunks as media', () => {
  const ytVideoUrl = 'https://rr1---sn-4g5ednks.googlevideo.com/videoplayback?expire=1728938400&ei=xyz&ip=1.2.3.4&id=o-ABC&itag=22&source=youtube&requiressl=yes&mime=video%2Fmp4&range=0-1048575';
  assert.equal(isMediaUrl(ytVideoUrl, 'video/mp4', 1048576), true);
  assert.equal(classifyMedia(ytVideoUrl, 'video/mp4'), 'video');

  const ytAudioUrl = 'https://rr2---sn-4g5ednks.googlevideo.com/videoplayback?expire=1728938400&id=o-ABC&itag=140&source=youtube&mime=audio%2Fmp4&range=0-524288';
  assert.equal(isMediaUrl(ytAudioUrl, 'audio/mp4', 524288), true);
  assert.equal(classifyMedia(ytAudioUrl, 'audio/mp4'), 'audio');
});

test('rejects adaptive YouTube tracks as unsupported complete videos', () => {
  assert.equal(isSupportedYouTubeFormat({
    url: 'https://cdn.example/video',
    mimeType: 'video/mp4',
    hasAudio: false,
  }), false);
  assert.equal(isSupportedYouTubeFormat({
    url: 'https://cdn.example/video',
    mimeType: 'video/mp4',
    hasAudio: true,
  }), true);
});

test('does not report native Start errors as successful handoffs', async () => {
  const previousChrome = global.chrome;
  let call = 0;
  global.chrome = {
    runtime: {
      sendNativeMessage(_host, _command, callback) {
        call += 1;
        callback(call === 1
          ? { ok: true, payload: { job_id: 'job-1' } }
          : { ok: false, error: 'HTTP 403 for https://signed.example/video?token=secret' });
      },
      lastError: null,
    },
  };
  try {
    const result = await executeHandoff('https://signed.example/video?token=secret', 'video.mp4');
    assert.equal(result.ok, false);
    assert.equal(result.error.includes('token=secret'), false);
  } finally {
    global.chrome = previousChrome;
  }
});
