const test = require('node:test');
const assert = require('node:assert/strict');
const {
  validateHandoff,
  redactUrl,
  isMediaUrl,
  isDownloadableUrl,
  extractFilename,
  parseM3u8Variants,
  formatNativeCommand,
} = require('../src/protocol');
const { shouldHandoff } = require('../src/background');

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

