const { validateHandoff } = require('./protocol');

const seen = new Set();

function shouldHandoff(download) {
  if (!validateHandoff({ url: download.url }) || seen.has(download.url)) {
    return false;
  }
  seen.add(download.url);
  return true;
}

if (typeof chrome !== 'undefined' && chrome.downloads) {
  chrome.downloads.onCreated.addListener((download) => {
    if (shouldHandoff(download)) {
      chrome.runtime.sendNativeMessage('org.open_download_manager.host', {
        version: 1,
        type: 'download.request',
        url: download.url
      });
    }
  });
}

module.exports = { shouldHandoff };
