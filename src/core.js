// @ts-check

/**
 * @template T
 * @param {T[] | undefined} array
 * @param {T | undefined} element
 * @returns T[] | undefined
 */
export function addToArray(array, element) {
  if (!element) return array;
  if (!array) return [element];
  array.push(element);
  return array;
}

const _breakBskyPostURL_Regex = /^http[s]?\:\/\/bsky\.app\/profile\/([a-z0-9\.\:\-]+)\/post\/([a-z0-9]+)(\/|$)/i;
const _breakBskyStylePostURL_Regex = /^http[s]?\:\/\/(bsky\.app|6sky\.app|gist\.ing|gisti\.ng|gist\.ink)\/profile\/([a-z0-9\.\:\-]+)\/post\/([a-z0-9]+)(\/|$)/i;
const _breakGistingPostURL_Regex = /^http[s]?\:\/\/(6sky\.app|gist\.ing|gisti\.ng|gist\.ink)\/([a-z0-9\.\:\-]+)\/([a-z0-9]+)(\/|$)/i;

const _shortenDID_Regex = /^did\:plc\:/;

const _breakFeedUri_Regex = /^at\:\/\/(did:plc:)?([a-z0-9]+)\/([a-z\.]+)\/?(.*)?$/;

/**
 * @param {string | null | undefined} did
 * @param {string | null | undefined} cid
 */
export function getFeedBlobUrl(did, cid) {
  if (!did || !cid) return undefined;
  return `https://cdn.bsky.app/img/feed_thumbnail/plain/${unwrapShortDID(did)}/${cid}@jpeg`;
}

/**
 * @param {string | null | undefined} did
 * @param {string | null | undefined} cid
 */
export function getFeedVideoBlobUrl(did, cid) {
  if (!did || !cid) return undefined;
  return `https://video.bsky.app/watch/${unwrapShortDID(did)}/${cid}/thumbnail.jpg`;
}

/**
 * @param {string | null | undefined} url
 */
export function breakPostURL(url) {
  if (!url) return;
  const matchBsky = _breakBskyPostURL_Regex.exec(url);
  if (matchBsky) return { shortDID: shortenDID(matchBsky[1]), postID: matchBsky[2]?.toString().toLowerCase() };
  const matchGisting = _breakGistingPostURL_Regex.exec(url);
  if (matchGisting) return { shortDID: shortenDID(matchGisting[2]), postID: matchGisting[3]?.toString().toLowerCase() };
  const matchBskyStyle = _breakBskyStylePostURL_Regex.exec(url);
  if (matchBskyStyle) return { shortDID: shortenDID(matchBskyStyle[2]), postID: matchBskyStyle[3]?.toString().toLowerCase() };
}

/**
 * @param {string | null | undefined} text
 * @return {text is string}
 */
export function likelyDID(text) {
  return !!text && (
    !text.trim().indexOf('did:') ||
    text.trim().length === 24 && !/[^\sa-z0-9]/i.test(text)
  );
}

/**
 * @template {string | undefined | null} T
 * @param {T} did
 * @returns {T}
 */
export function shortenDID(did) {
  return did && /** @type {T} */(did.replace(_shortenDID_Regex, '').toLowerCase() || undefined);
}

/**
 * @template {string | undefined | null} T
 * @param {T} shortDID
 * @returns {T}
 */
export function unwrapShortDID(shortDID) {
  return /** @type {T} */(
    !shortDID ? undefined : shortDID.indexOf(':') < 0 ? 'did:plc:' + shortDID.toLowerCase() : shortDID.toLowerCase()
  );
}

/**
 * Normalize a short handle into a fully qualified host handle string.
 * @param {string | null | undefined} shortHandle
 * @returns {string | undefined}
 */
export function unwrapShortHandle(shortHandle) {
  if (likelyDID(shortHandle)) return unwrapShortDID(/** @type {string} */(shortHandle));
  const normalized = cheapNormalizeHandle(shortHandle);
  if (!normalized) return undefined;
  return normalized.indexOf('.') < 0 ? normalized.toLowerCase() + '.bsky.social' : normalized.toLowerCase();
}

/**
 * @param {string | null | undefined} handle
 * @returns {string | undefined}
 */
export function cheapNormalizeHandle(handle) {
  handle = handle && handle.trim().toLowerCase();

  if (handle && handle.charCodeAt(0) === 64)
    handle = handle.slice(1);

  const urlprefix = 'https://bsky.app/';
  if (handle && handle.lastIndexOf(urlprefix, 0) === 0) {
    const postURL = breakPostURL(handle);
    if (postURL && postURL.shortDID)
      return postURL.shortDID;
  }

  if (handle && handle.lastIndexOf('at:', 0) === 0) {
    const feedUri = breakFeedURI(handle);
    if (feedUri && feedUri.shortDID)
      return feedUri.shortDID;

    if (handle && handle.lastIndexOf('at://', 0) === 0) handle = handle.slice(5);
    else handle = handle.slice(3);
  }

  return handle || undefined;
}

/**
 * @param {string | null | undefined} uri
 */
export function breakFeedURI(uri) {
  if (!uri) return;
  const match = _breakFeedUri_Regex.exec(uri);
  if (!match || !match[4]) return;
  if (match[3] === 'app.bsky.feed.post') return { shortDID: shortenDID(match[2]), postID: match[4] };
  return { shortDID: match[2], postID: match[4], feedType: match[3] };
}

/**
 * @param {string | null | undefined} shortDID
 * @param {string | number} postID
 */
export function makeFeedUri(shortDID, postID) {
  return 'at://' + unwrapShortDID(shortDID) + '/app.bsky.feed.post/' + postID;
}
