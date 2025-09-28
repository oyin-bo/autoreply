// @ts-check

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

import package_json from '../package.json';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

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
async function initKeytar() {
  try {
    // dynamic import to avoid failing on platforms without keytar
    const keytarMod = await import('keytar');
    const tryPromise = keytarMod.getPassword(package_json.name, 'default_handle');
    return tryPromise
      .then(() => keytarMod)
      .catch(() => fallbackKeytar);
  } catch (e) {
    return Promise.resolve(fallbackKeytar);
  }
}

// Export the promise only
export default initKeytar();

