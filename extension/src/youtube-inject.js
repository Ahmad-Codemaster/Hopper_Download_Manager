// Hopper Download Manager - YouTube Stream Extractor (Runs in MAIN world)

(function () {
  function getPlayerResponse() {
    try {
      // 1. Direct window property
      if (window.ytInitialPlayerResponse && window.ytInitialPlayerResponse.streamingData) {
        return window.ytInitialPlayerResponse;
      }

      // 2. Movie player API
      const player = document.getElementById('movie_player');
      if (player && typeof player.getPlayerResponse === 'function') {
        const resp = player.getPlayerResponse();
        if (resp && resp.streamingData) {
          return resp;
        }
      }

      // 3. YouTube config args
      if (window.ytplayer && window.ytplayer.config && window.ytplayer.config.args) {
        const raw = window.ytplayer.config.args.raw_player_response;
        if (raw) {
          return typeof raw === 'string' ? JSON.parse(raw) : raw;
        }
      }
    } catch {
      // ignore
    }
    return null;
  }

  function extractStreams() {
    const data = getPlayerResponse();
    if (!data || !data.streamingData) return;

    const title = (data.videoDetails && data.videoDetails.title) || document.title;
    const author = (data.videoDetails && data.videoDetails.author) || '';
    const formats = [];

    // 1. Progressive streams (Video + Audio combined, e.g. 720p, 360p)
    if (Array.isArray(data.streamingData.formats)) {
      for (const f of data.streamingData.formats) {
        if (f.url) {
          const quality = f.qualityLabel || (f.height ? `${f.height}p` : '360p');
          formats.push({
            url: f.url,
            quality: `${quality} (MP4 Video+Audio)`,
            mimeType: f.mimeType || 'video/mp4',
            size: f.contentLength ? parseInt(f.contentLength, 10) : 0,
            type: 'video',
            hasAudio: true,
            itag: f.itag,
          });
        }
      }
    }

    // 2. Adaptive streams (Individual video & audio tracks)
    if (Array.isArray(data.streamingData.adaptiveFormats)) {
      for (const f of data.streamingData.adaptiveFormats) {
        if (f.url) {
          const isAudio = (f.mimeType || '').startsWith('audio/');
          let quality = f.qualityLabel || (f.height ? `${f.height}p` : 'Video');
          if (isAudio) {
            quality = f.audioBitrate ? `Audio (${f.audioBitrate}kbps)` : 'Audio Track';
          }
          formats.push({
            url: f.url,
            quality,
            mimeType: f.mimeType,
            size: f.contentLength ? parseInt(f.contentLength, 10) : 0,
            type: isAudio ? 'audio' : 'video',
            hasAudio: false,
            itag: f.itag,
          });
        }
      }
    }

    if (formats.length > 0) {
      window.postMessage(
        {
          source: 'hopper-yt-extractor',
          title,
          author,
          formats,
        },
        '*'
      );
    }
  }

  // Initial runs (retry as player initializes)
  extractStreams();
  setTimeout(extractStreams, 800);
  setTimeout(extractStreams, 2000);
  setTimeout(extractStreams, 4000);

  // Hook YouTube SPA navigation events
  window.addEventListener('yt-navigate-finish', () => {
    setTimeout(extractStreams, 500);
    setTimeout(extractStreams, 1500);
  });
  window.addEventListener('yt-page-data-updated', () => {
    setTimeout(extractStreams, 500);
  });
  window.addEventListener('spfdone', () => {
    setTimeout(extractStreams, 500);
  });
})();
