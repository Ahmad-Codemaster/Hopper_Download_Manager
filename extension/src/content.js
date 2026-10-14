// Hopper Download Manager - In-Page Media Grabber Content Script

(function () {
  if (window.__hopperGrabberLoaded) return;
  window.__hopperGrabberLoaded = true;

  const capturedMedia = new Map();
  let container = null;
  let triggerBtn = null;
  let menu = null;
  let countBadge = null;

  function formatBytes(bytes) {
    if (!bytes || bytes <= 0) return '';
    const units = ['B', 'KB', 'MB', 'GB'];
    let b = bytes;
    let i = 0;
    while (b >= 1024 && i < units.length - 1) {
      b /= 1024;
      i++;
    }
    return `${b.toFixed(1)} ${units[i]}`;
  }

  function cleanPageTitle() {
    return (document.title || 'Video')
      .replace(/[\/\\:*?"<>|]/g, '_')
      .replace(/\s+/g, ' ')
      .replace(/(?: - Watch Online.*| \| Netflix.*| - YouTube.*| \| Prime Video.*| \| Disney\+.*)$/i, '')
      .trim();
  }

  function showToast(msg, isSuccess = true) {
    const toast = document.createElement('div');
    toast.className = 'hopper-toast';
    toast.style.background = isSuccess ? '#10b981' : '#ef4444';
    toast.textContent = msg;
    document.body.appendChild(toast);
    setTimeout(() => {
      toast.remove();
    }, 3200);
  }

  function createUI() {
    if (container) return;

    container = document.createElement('div');
    container.className = 'hopper-grabber-container';
    container.style.display = 'none';

    // Floating Button
    triggerBtn = document.createElement('button');
    triggerBtn.className = 'hopper-grabber-btn';
    triggerBtn.innerHTML = `
      <div class="hopper-icon">⚡</div>
      <span>Download Video</span>
      <span class="hopper-count-badge">0</span>
    `;

    countBadge = triggerBtn.querySelector('.hopper-count-badge');

    // Dropdown Menu
    menu = document.createElement('div');
    menu.className = 'hopper-grabber-menu';
    menu.innerHTML = `
      <div class="hopper-menu-header">
        <span>Available Streams</span>
        <span style="cursor:pointer;" id="hopper-close-menu">✕</span>
      </div>
      <div id="hopper-media-items-list" style="display:flex; flex-direction:column; gap:6px;"></div>
    `;

    const closeBtn = menu.querySelector('#hopper-close-menu');
    closeBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      menu.classList.remove('hopper-open');
    });

    triggerBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      menu.classList.toggle('hopper-open');
    });

    document.addEventListener('click', (e) => {
      if (container && !container.contains(e.target)) {
        menu.classList.remove('hopper-open');
      }
    });

    container.appendChild(menu);
    container.appendChild(triggerBtn);
    document.body.appendChild(container);
  }

  function updateUI() {
    createUI();
    const items = Array.from(capturedMedia.values());
    if (items.length === 0) {
      container.style.display = 'none';
      return;
    }

    container.style.display = 'block';
    if (countBadge) {
      countBadge.textContent = String(items.length);
    }

    const list = menu.querySelector('#hopper-media-items-list');
    if (!list) return;

    list.innerHTML = '';
    items.forEach((item) => {
      const el = document.createElement('div');
      el.className = 'hopper-media-item';

      let typeClass = 'hopper-type-video';
      let typeLabel = item.quality || 'Video';

      if (item.type === 'audio') {
        typeClass = 'hopper-type-audio';
        typeLabel = 'Audio';
      } else if (item.type === 'hls-stream' || item.type === 'dash-stream') {
        typeClass = 'hopper-type-stream';
        typeLabel = item.quality || (item.type === 'hls-stream' ? 'HLS' : 'DASH');
      } else if (item.quality) {
        typeLabel = item.quality;
      }

      const sizeStr = item.size ? formatBytes(item.size) : '';

      el.innerHTML = `
        <div class="hopper-media-info">
          <div class="hopper-media-title" title="${item.filename || item.url}">${item.filename || 'Video Stream'}</div>
          <div class="hopper-media-meta">
            <span class="hopper-type-pill ${typeClass}">${typeLabel}</span>
            ${sizeStr ? `<span>${sizeStr}</span>` : ''}
          </div>
        </div>
        <button class="hopper-download-action-btn">Download</button>
      `;

      const dlBtn = el.querySelector('.hopper-download-action-btn');
      dlBtn.addEventListener('click', () => {
        dlBtn.disabled = true;
        dlBtn.textContent = 'Starting...';

        chrome.runtime.sendMessage(
          {
            action: 'HANDOFF_DOWNLOAD',
            url: item.url,
            filename: item.filename,
          },
          (response) => {
            if (response && response.ok) {
              dlBtn.textContent = 'Sent ✓';
              dlBtn.style.backgroundColor = '#10b981';
              const msg = response.method === 'native'
                ? '✓ Added to Hopper Download Manager'
                : '✓ Download started in browser';
              showToast(msg, true);
              setTimeout(() => {
                menu.classList.remove('hopper-open');
                dlBtn.disabled = false;
                dlBtn.textContent = 'Download';
                dlBtn.style.backgroundColor = '';
              }, 1200);
            } else {
              dlBtn.disabled = false;
              dlBtn.textContent = 'Download';
              const err = (response && response.error) || 'Could not send download';
              showToast(`✕ ${err}`, false);
            }
          }
        );
      });

      list.appendChild(el);
    });
  }

  function addMedia(item) {
    if (!item || !item.url) return;
    if (!capturedMedia.has(item.url)) {
      capturedMedia.set(item.url, item);
      updateUI();
    }
  }

  // 1. Listen for background media sniffer events
  chrome.runtime.onMessage.addListener((message) => {
    if (message.type === 'MEDIA_DETECTED' && message.media) {
      addMedia(message.media);
    }
  });

  // 2. Fetch existing media already discovered for this tab
  chrome.runtime.sendMessage({ action: 'GET_MEDIA' }, (response) => {
    if (response && response.ok && Array.isArray(response.items)) {
      response.items.forEach(addMedia);
    }
  });

  // 3. Scan DOM for HTML5 <video> and <audio> elements
  function scanDOMMedia() {
    const videoAudioElements = document.querySelectorAll('video, audio');
    videoAudioElements.forEach((el) => {
      let src = el.currentSrc || el.src;
      if (src && (src.startsWith('http://') || src.startsWith('https://'))) {
        const isAudio = el.tagName.toLowerCase() === 'audio';
        const pageTitle = cleanPageTitle();
        const ext = isAudio ? '.mp3' : '.mp4';
        let quality = isAudio ? 'Audio' : 'Video';
        if (!isAudio && el.videoWidth && el.videoHeight) {
          quality = `${el.videoHeight}p`;
        }
        const filename = `${pageTitle}${quality !== 'Video' && quality !== 'Audio' ? ` [${quality}]` : ''}${ext}`;
        const item = {
          url: src,
          filename,
          quality,
          type: isAudio ? 'audio' : 'video',
          size: 0,
        };
        addMedia(item);
        chrome.runtime.sendMessage({ action: 'RECORD_DOM_MEDIA', item });
      }

      // Also scan <source> tags
      const sources = el.querySelectorAll('source');
      sources.forEach((s) => {
        if (s.src && (s.src.startsWith('http://') || s.src.startsWith('https://'))) {
          const isAudio = (s.type || '').includes('audio');
          const pageTitle = cleanPageTitle();
          const ext = isAudio ? '.mp3' : '.mp4';
          const filename = `${pageTitle}${ext}`;
          const item = {
            url: s.src,
            filename,
            quality: isAudio ? 'Audio' : 'Video',
            type: isAudio ? 'audio' : 'video',
            size: 0,
          };
          addMedia(item);
          chrome.runtime.sendMessage({ action: 'RECORD_DOM_MEDIA', item });
        }
      });
    });
  }

  // Initial scan + mutation observer for modern single-page apps (like YouTube/Netflix)
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', scanDOMMedia);
  } else {
    scanDOMMedia();
  }

  const observer = new MutationObserver(() => {
    scanDOMMedia();
  });
  observer.observe(document.documentElement, {
    childList: true,
    subtree: true,
  });
})();
