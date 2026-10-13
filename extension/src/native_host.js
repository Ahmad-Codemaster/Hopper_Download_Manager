const HOST_NAME = 'org.open_download_manager.host';

function connect(chromeApi = chrome) {
  return chromeApi.runtime.connectNative(HOST_NAME);
}

module.exports = { HOST_NAME, connect };
