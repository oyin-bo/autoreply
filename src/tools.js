// @ts-check

const { addToArray, getFeedBlobUrl, getFeedVideoBlobUrl } = require('./core');

/**
 * @param {any} post
 */
function formatPost(post) {
  /** @type {any} */
  const postRecord = post.record
  let replyToURI = postRecord.reply?.parent?.uri;
  if (replyToURI === post.uri) replyToURI = undefined;

  const header =
    post.indexedAt + ' @' + post.author.handle +
    (
      post.author.displayName ?
        ' ' + JSON.stringify(post.author.displayName) + ' ' :
        ''
    ) +
    ' postURI: ' + post.uri +
    (replyToURI ? ' reply to: ' + replyToURI : '');

  const text = /** @type {string} */(
    post.record.text || ''
  ).split('\n').map(line => '> ' + line).join('\n');

  const stats =
    (post.likeCount || post.replyCount || post.repostCount || post.quoteCount ?
      '(' +
      [
        post.likeCount ? post.likeCount + ' likes' : '',
        post.replyCount ? post.replyCount + ' replies' : '',
        post.repostCount ? post.repostCount + ' reposts' : '',
        post.quoteCount ? post.quoteCount + ' quotes' : ''
      ].filter(Boolean).join(', ') +
      ')'
      : ''
    );

  const textual = header + '\n' + text + stats;

  let links = extractEmbeds(post.author.did, postRecord.embed);

  return {
    textual,
    structured: {
      indexedAt: post.indexedAt,
      author: post.author.handle,
      authorName: post.author.displayName,
      postURI: post.uri,
      replyToURI,
      text: /** @type {string} */(post.record.text),
      likeCount: post.likeCount,
      replyCount: post.replyCount,
      repostCount: post.repostCount,
      quoteCount: post.quoteCount,
      links
    }
  };
}

/**
 * @param {string} shortDID
 * @param {any} embed
 */
function extractEmbeds(shortDID, embed) {
  if (!embed) return;

  /** @type {{ url: string, title?: string }[] | undefined} */
  let embeds = undefined;

  embeds = addEmbedImages(shortDID, /** @type {any} */(embed).images, embeds);
  embeds = addEmbedVideo(shortDID, /** @type {any} */(embed), embeds);
  embeds = addEmbedExternal(shortDID, /** @type {any} */(embed).external, embeds);
  embeds = addEmbedRecord(/** @type {any} */(embed).record, embeds);
  embeds = addEmbedRecordMedia(shortDID, /** @type {any} */(embed), embeds);

  return embeds;
}

/**
 * @param {string} shortDID
 * @param {any} embedImages 
 * @param {{ url: string, title?: string }[] | undefined} embeds 
 */
function addEmbedImages(shortDID, embedImages, embeds) {
  if (!embedImages?.length) return embeds;
  for (const img of embedImages) {
    if (!img) continue;
    const url = getFeedBlobUrl(shortDID, img.image?.ref?.$link);
    if (url) {
      embeds = addToArray(embeds, {
        url,
        title: img.alt || undefined
      });
    }
  }
  return embeds;
}

/**
 * @param {string} shortDID
 * @param {any} embedVideo 
 * @param {{ url: string, title?: string }[] | undefined} embeds 
 */
function addEmbedVideo(shortDID, embedVideo, embeds) {
  const url = getFeedVideoBlobUrl(shortDID, embedVideo?.video?.ref?.$link);
  if (url) {
    embeds = addToArray(embeds, {
      url,
      title: embedVideo?.alt || undefined
    });
  }
  return embeds;
}

/**
 * @param {string} shortDID
 * @param {any} embedExternal
 * @param {{ url: string, title?: string }[] | undefined} embeds 
 */
function addEmbedExternal(shortDID, embedExternal, embeds) {
  if (!embedExternal?.uri) return embeds;
  const url = embedExternal.uri || undefined;
  if (!url) return embeds;
  return addToArray(embeds, {
    url,
    title: embedExternal.title || embedExternal.description || undefined,
    // imgSrc: getFeedBlobUrl(shortDID, embedExternal.thumb?.ref?.toString())
  });
}

/**
 * @param {any} embedRecord
 * @param {{ url: string, title?: string }[] | undefined} embeds 
 */
function addEmbedRecord(embedRecord, embeds) {
  if (!embedRecord?.uri) return embeds;
  return addToArray(embeds, {
    url: embedRecord.uri
  });
}

/**
 * @param {string} shortDID
 * @param {any} embedRecordMedia
 * @param {{ url: string, title?: string }[] | undefined} embeds 
 */
function addEmbedRecordMedia(shortDID, embedRecordMedia, embeds) {
  embeds = addEmbedImages(
    shortDID,
    /** @type {any} */(embedRecordMedia?.media)?.images,
    embeds);

  embeds = addEmbedVideo(
    shortDID,
    /** @type {any} */(embedRecordMedia?.media),
    embeds);

  embeds = addEmbedExternal(
    shortDID,
    /** @type {any} */(embedRecordMedia?.media)?.external,
    embeds);

  embeds = addEmbedRecord(
    /** @type {any} */(embedRecordMedia?.record)?.record,
    embeds);

  return embeds;
}

module.exports = {
  formatPost
};