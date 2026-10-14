let protocol;
if (typeof require !== 'undefined') {
  protocol = require('./protocol');
} else if (typeof importScripts !== 'undefined') {
  importScripts('protocol.js');
  protocol = globalThis.HopperProtocol;
} else {
  protocol = globalThis.HopperProtocol || {};
}

const {
  validateHandoff,
  isMediaUrl,
  isDownloadableUrl,
  classifyMedia,
  extractFilename,
  parseM3u8Variants,
  formatNativeCommand,
  sanitizeTitle,
} = protocol;

const HOST_NAME = 'org.open_download_manager.host';
const seen = new Set();
const tabMedia = new Map();
const tabTitles = new Map();

function shouldHandoff(download, consented = false) {
  if (!download || !download.url) return false;
  if (!consented || !validateHandoff({ url: download.url }) || seen.has(download.url)) {
    return false;
  }
  seen.add(download.url);
  return true;
}

function getHeader(headers, name) {
  if (!headers) return '';
  const target = name.toLowerCase();
  for (const h of headers) {
    if (h.name && h.name.toLowerCase() === target) {
      return h.value || '';
    }
  }
  return '';
}

function recordMediaItem(tabId, item) {
  if (tabId < 0) return;
  if (!tabMedia.has(tabId)) {
    tabMedia.set(tabId, []);
  }
  const list = tabMedia.get(tabId);
  const exists = list.some((existing) => existing.url === item.url);
  if (!exists) {
    list.push(item);
    if (typeof chrome !== 'undefined' && chrome.action) {
      chrome.action.setBadgeText({ tabId, text: String(list.length) }).catch(() => {});
      chrome.action.setBadgeBackgroundColor({ tabId, color: '#2563eb' }).catch(() => {});
    }
    if (typeof chrome !== 'undefined' && chrome.tabs) {
      chrome.tabs.sendMessage(tabId, { type: 'MEDIA_DETECTED', media: item }).catch(() => {});
    }
  }
}

async function sendNativeCommand(commandObj) {
  return new Promise((resolve) => {
    if (typeof chrome === 'undefined' || !chrome.runtime || !chrome.runtime.sendNativeMessage) {
      return resolve({ ok: false, error: 'Native messaging not available' });
    }
    try {
      chrome.runtime.sendNativeMessage(HOST_NAME, commandObj, (response) => {
        if (chrome.runtime.lastError) {
          return resolve({ ok: false, error: chrome.runtime.lastError.message });
        }
        resolve(response || { ok: true });
      });
    } catch (err) {
      resolve({ ok: false, error: String(err) });
    }
  });
}

async function executeHandoff(sourceUrl, destinationName) {
  if (!validateHandoff({ url: sourceUrl })) {
    return { ok: false, error: 'Invalid URL scheme' };
  }
  const cleanFilename = destinationName
    ? destinationName.replace(/[\/\\:*?"<>|]/g, '_')
    : extractFilename(sourceUrl);

  const createCmd = formatNativeCommand('Create', {
    source: sourceUrl,
    destination: cleanFilename,
  });

  const createResp = await sendNativeCommand(createCmd);
  if (createResp && createResp.ok && createResp.payload && createResp.payload.job_id) {
    const jobId = createResp.payload.job_id;
    const startCmd = formatNativeCommand('Start', { job_id: jobId });
    await sendNativeCommand(startCmd);
    return { ok: true, job_id: jobId, method: 'native' };
  }

  // Resilient Fallback: If native host is not available or rejected, trigger browser download
  if (typeof chrome !== 'undefined' && chrome.downloads && chrome.downloads.download) {
    try {
      const downloadId = await chrome.downloads.download({
        url: sourceUrl,
        filename: cleanFilename,
        saveAs: false,
      });
      return { ok: true, downloadId, method: 'browser', fallback: true };
    } catch (dlErr) {
      return { ok: false, error: (createResp && createResp.error) || String(dlErr) };
    }
  }

  return createResp || { ok: false, error: 'Download failed' };
}

async function getTabTitle(tabId) {
  if (tabTitles.has(tabId)) {
    return tabTitles.get(tabId);
  }
  if (typeof chrome !== 'undefined' && chrome.tabs && tabId >= 0) {
    try {
      const tab = await chrome.tabs.get(tabId);
      if (tab && tab.title) {
        tabTitles.set(tabId, tab.title);
        return tab.title;
      }
    } catch {
      // ignore
    }
  }
  return '';
}

async function parseAndRecordHlsStream(tabId, m3u8Url, pageTitle) {
  try {
    const res = await fetch(m3u8Url);
    if (!res.ok) return;
    const text = await res.text();
    const cleanTitle = (sanitizeTitle && sanitizeTitle(pageTitle)) || pageTitle || 'Video';
    const variants = parseM3u8Variants(text, m3u8Url);

    if (variants && variants.length > 0) {
      for (const v of variants) {
        const qualityName = v.resolution !== 'auto' ? v.resolution : `${Math.round(v.bandwidth / 1000)}kbps`;
        recordMediaItem(tabId, {
          id: `${tabId}-${v.resolution}-${v.bandwidth}`,
          url: v.url,
          filename: `${cleanTitle} [${qualityName}].mp4`,
          masterUrl: m3u8Url,
          quality: qualityName,
          type: 'video',
          size: 0,
          timestamp: Date.now(),
        });
      }
    } else {
      recordMediaItem(tabId, {
        id: `${tabId}-hls-${Date.now()}`,
        url: m3u8Url,
        filename: `${cleanTitle}.m3u8`,
        quality: 'HLS',
        type: 'hls-stream',
        size: 0,
        timestamp: Date.now(),
      });
    }
  } catch {
    const cleanTitle = (sanitizeTitle && sanitizeTitle(pageTitle)) || pageTitle || 'Video';
    recordMediaItem(tabId, {
      id: `${tabId}-hls-${Date.now()}`,
      url: m3u8Url,
      filename: `${cleanTitle}.m3u8`,
      quality: 'HLS',
      type: 'hls-stream',
      size: 0,
      timestamp: Date.now(),
    });
  }
}

// 1. Sniff HTTP traffic for media streams and large files
if (typeof chrome !== 'undefined' && chrome.webRequest && chrome.webRequest.onHeadersReceived) {
  chrome.webRequest.onHeadersReceived.addListener(
    async (details) => {
      if (details.tabId < 0) return;
      const contentType = getHeader(details.responseHeaders, 'content-type');
      const contentDisposition = getHeader(details.responseHeaders, 'content-disposition');
      const contentLength = getHeader(details.responseHeaders, 'content-length');
      const size = contentLength ? parseInt(contentLength, 10) : 0;

      const isMedia = isMediaUrl(details.url, contentType, size);
      const isDownload = isDownloadableUrl(details.url, contentDisposition, contentType, size);

      if (isMedia || isDownload) {
        const type = classifyMedia(details.url, contentType);
        const pageTitle = await getTabTitle(details.tabId);

        if (type === 'hls-stream') {
          await parseAndRecordHlsStream(details.tabId, details.url, pageTitle);
          return;
        }

        if (details.url.includes('googlevideo.com/videoplayback')) {
          try {
            const parsed = new URL(details.url);
            // Crucial: remove range parameter so URL requests full stream instead of a 1MB chunk
            parsed.searchParams.delete('range');
            const cleanUrl = parsed.toString();
            const itag = parsed.searchParams.get('itag') || '';
            const itagMap = {
              '18': '360p',
              '22': '720p HD',
              '137': '1080p Full HD',
              '136': '720p',
              '135': '480p',
              '134': '360p',
              '140': 'Audio (M4A)',
              '251': 'Audio (Opus)',
            };
            const quality = itagMap[itag] || (type === 'audio' ? 'Audio' : 'Video');
            const isAudio = quality.startsWith('Audio') || type === 'audio';
            const ext = isAudio ? '.mp3' : '.mp4';
            const cleanTitle = (sanitizeTitle && sanitizeTitle(pageTitle)) || pageTitle || 'YouTube Video';
            const filename = `${cleanTitle} [${quality}]${ext}`;

            recordMediaItem(details.tabId, {
              id: `${details.tabId}-yt-${itag || Date.now()}`,
              url: cleanUrl,
              filename,
              quality,
              contentType,
              type: isAudio ? 'audio' : 'video',
              size,
              timestamp: Date.now(),
            });
            return;
          } catch {
            // fallback
          }
        }

        const filename = extractFilename(details.url, contentDisposition, 'download.bin', pageTitle, type);

        recordMediaItem(details.tabId, {
          id: `${details.tabId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
          url: details.url,
          filename,
          contentType,
          type,
          size,
          timestamp: Date.now(),
        });
      }
    },
    { urls: ['<all_urls>'] },
    ['responseHeaders']
  );
}

// 2. Intercept browser downloads
if (typeof chrome !== 'undefined' && chrome.downloads && chrome.downloads.onCreated) {
  chrome.downloads.onCreated.addListener(async (download) => {
    let autoIntercept = true;
    if (chrome.storage && chrome.storage.local) {
      const stored = await chrome.storage.local.get('autoIntercept');
      if (typeof stored.autoIntercept === 'boolean') {
        autoIntercept = stored.autoIntercept;
      }
    }

    if (shouldHandoff(download, autoIntercept)) {
      const filename = download.filename ? download.filename.split(/[\\/]/).pop() : undefined;
      const result = await executeHandoff(download.url, filename);
      if (result && result.ok && result.method === 'native') {
        chrome.downloads.cancel(download.id, () => {
          if (chrome.runtime.lastError) void chrome.runtime.lastError;
          chrome.downloads.erase({ id: download.id });
        });
      }
    }
  });
}

// 3. Tab lifecycle cleanup
if (typeof chrome !== 'undefined' && chrome.tabs) {
  if (chrome.tabs.onRemoved) {
    chrome.tabs.onRemoved.addListener((tabId) => {
      tabMedia.delete(tabId);
      tabTitles.delete(tabId);
    });
  }
  if (chrome.tabs.onUpdated) {
    chrome.tabs.onUpdated.addListener((tabId, changeInfo) => {
      if (changeInfo.title) {
        tabTitles.set(tabId, changeInfo.title);
      }
      if (changeInfo.status === 'loading') {
        // clear old page media on navigation
        tabMedia.delete(tabId);
        if (chrome.action) {
          chrome.action.setBadgeText({ tabId, text: '' }).catch(() => {});
        }
      }
    });
  }
}

// 4. Message dispatch from UI popup & floating grabber content script
if (typeof chrome !== 'undefined' && chrome.runtime && chrome.runtime.onMessage) {
  chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    (async () => {
      if (message.action === 'GET_MEDIA') {
        const tabId = message.tabId || (sender.tab && sender.tab.id);
        const items = tabId && tabMedia.has(tabId) ? tabMedia.get(tabId) : [];
        sendResponse({ ok: true, items });
      } else if (message.action === 'HANDOFF_DOWNLOAD') {
        const result = await executeHandoff(message.url, message.filename);
        sendResponse(result);
      } else if (message.action === 'PARSE_M3U8') {
        try {
          const resp = await fetch(message.url);
          const text = await resp.text();
          const variants = parseM3u8Variants(text, message.url);
          sendResponse({ ok: true, variants });
        } catch (err) {
          sendResponse({ ok: false, error: String(err) });
        }
      } else if (message.action === 'RECORD_DOM_MEDIA') {
        const tabId = sender.tab ? sender.tab.id : -1;
        if (tabId >= 0 && message.item) {
          recordMediaItem(tabId, message.item);
          sendResponse({ ok: true });
        } else {
          sendResponse({ ok: false });
        }
      } else {
        sendResponse({ ok: false, error: 'Unknown action' });
      }
    })();
    return true; // keep async channel open
  });
}

if (typeof module !== 'undefined' && module.exports) {
  module.exports = {
    shouldHandoff,
    classifyMedia,
    extractFilename,
    recordMediaItem,
    tabMedia,
    executeHandoff,
  };
}
