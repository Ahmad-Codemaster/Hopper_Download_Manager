// Hopper Download Manager - Popup Logic

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

document.addEventListener('DOMContentLoaded', async () => {
  const toggle = document.getElementById('auto-intercept-toggle');
  const mediaList = document.getElementById('media-list');
  const mediaCount = document.getElementById('media-count');

  // Load auto-intercept setting
  if (chrome.storage && chrome.storage.local) {
    const data = await chrome.storage.local.get('autoIntercept');
    if (typeof data.autoIntercept === 'boolean') {
      toggle.checked = data.autoIntercept;
    }
  }

  toggle.addEventListener('change', async () => {
    if (chrome.storage && chrome.storage.local) {
      await chrome.storage.local.set({ autoIntercept: toggle.checked });
    }
  });

  // Query active tab
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab || !tab.id) return;

  chrome.runtime.sendMessage({ action: 'GET_MEDIA', tabId: tab.id }, (response) => {
    if (!response || !response.ok || !Array.isArray(response.items)) return;

    const items = response.items;
    mediaCount.textContent = `${items.length} found`;

    if (items.length === 0) return;

    mediaList.innerHTML = '';
    items.forEach((item) => {
      const card = document.createElement('div');
      card.className = 'media-item';

      let badgeClass = 'badge-video';
      let badgeLabel = item.quality || 'Video';
      if (item.type === 'audio') {
        badgeClass = 'badge-audio';
        badgeLabel = 'Audio';
      } else if (item.type === 'hls-stream' || item.type === 'dash-stream') {
        badgeClass = 'badge-stream';
        badgeLabel = item.quality || (item.type === 'hls-stream' ? 'HLS' : 'DASH');
      } else if (item.type === 'file' || item.type === 'archive-or-binary') {
        badgeClass = 'badge-file';
        badgeLabel = 'File';
      }

      const sizeStr = item.size ? formatBytes(item.size) : '';

      card.innerHTML = `
        <div class="item-left">
          <span class="item-name" title="${item.filename || item.url}">${item.filename || 'Media Resource'}</span>
          <div class="item-meta">
            <span class="badge ${badgeClass}">${badgeLabel}</span>
            ${sizeStr ? `<span>${sizeStr}</span>` : ''}
          </div>
        </div>
        <button class="dl-button">Download</button>
      `;

      const dlBtn = card.querySelector('.dl-button');
      dlBtn.addEventListener('click', () => {
        dlBtn.disabled = true;
        dlBtn.textContent = 'Sending...';

        chrome.runtime.sendMessage(
          {
            action: 'HANDOFF_DOWNLOAD',
            url: item.url,
            filename: item.filename,
          },
          (res) => {
            if (res && res.ok) {
              dlBtn.textContent = 'Sent ✓';
              dlBtn.style.backgroundColor = '#10b981';
            } else {
              dlBtn.disabled = false;
              dlBtn.textContent = 'Failed ✕';
              dlBtn.style.backgroundColor = '#ef4444';
              setTimeout(() => {
                dlBtn.textContent = 'Download';
                dlBtn.style.backgroundColor = '#2563eb';
              }, 2500);
            }
          }
        );
      });

      mediaList.appendChild(card);
    });
  });
});
