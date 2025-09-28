// @ts-check

import fs from 'fs';
import path from 'path';
import os from 'os';

const __dirname = path.dirname(new URL(import.meta.url).pathname);

const STORAGE_DIR = path.join(__dirname, '../.storage');

export function storage() {
  const { did, didnt } = makeDirectories();

  return {
    readCars,
    addCar
  };

  /**
   * @param {string} shortDID
   */
  function readCars(shortDID) {
  }

  /**
   * @param {string} shortDID
   * @param {Buffer} buffer
   */
  function addCar(shortDID, buffer) {

  }
}

function makeDirectories() {
  const did = path.join(STORAGE_DIR, 'did');
  const didnt = path.join(STORAGE_DIR, 'didnt');

  for (const dir of [did, didnt]) {
    if (!fs.existsSync(dir)) {
      fs.mkdirSync(dir, { recursive: true });
    }
  }

  return { did, didnt };
}