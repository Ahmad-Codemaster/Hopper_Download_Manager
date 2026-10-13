const { validateHandoff } = require('./protocol');

const seen = new Set();

function shouldHandoff(download, consented = false) {
  if (!consented || !validateHandoff({ url: download.url }) || seen.has(download.url)) {
    return false;
  }
  seen.add(download.url);
  return true;
}

if (typeof chrome !== 'undefined' && chrome.downloads) {
  chrome.downloads.onCreated.addListener((download) => {
    // The download is only queued for an explicit action/consent flow.
    void download;
  });
}

module.exports = { shouldHandoff };
