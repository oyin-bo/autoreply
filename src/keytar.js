// @ts-check

const fs = require('fs');
const path = require('path');

const { name } = require('../package.json');

const CRED_FILE = path.join(__dirname, '..', '.bluesky_creds.json');

/** @type {Pick<import('keytar'), 'getPassword' | 'setPassword'>} */
const fallbackKeytar = {
  async setPassword(service, account, password) {
    /** @type {Record<string, string>} */
    let creds = {};
    if (fs.existsSync(CRED_FILE)) {
      try { creds = JSON.parse(fs.readFileSync(CRED_FILE, 'utf8')); } catch { }
    }
    creds[account] = password;
    fs.writeFileSync(CRED_FILE, JSON.stringify(creds, null, 2));
  },
  async getPassword(service, account) {
    /** @type {Record<string, string>} */
    let creds = {};
    if (fs.existsSync(CRED_FILE)) {
      try { creds = JSON.parse(fs.readFileSync(CRED_FILE, 'utf8')); } catch { }
    }
    return creds[account] || null;
  }
};

/**
 * Initialize keytar or fall back to file-based storage.
 * Always returns a Promise resolving to a Keytar-like API.
 * @returns {Promise<Pick<import('keytar'), 'getPassword' | 'setPassword'>>} Promise resolving to keytar-like module
 */
function initKeytar() {
  try {
    const keytarMod = require('keytar');
    const tryPromise = keytarMod.getPassword(name, 'default_handle');
    return tryPromise
      .then(() => keytarMod)
      .catch(() => fallbackKeytar);
  } catch (e) {
    return Promise.resolve(fallbackKeytar);
  }
}

// Export the promise only
module.exports = initKeytar();

