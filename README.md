# ✉️ autoreply⧸𝗻𝗽𝗺: an MCP for BlueSky

Welcome to `autoreply`, make your AI a poster (or a lurker).

## Installation

To skip downloading, dealing with `mcp.json` and all that, just run this command:

```bash
npx autoreply install
```

It gets `autoreply` MCP tool in your local Gemini CLI and VSCode ready to talk to BlueSky.

## What is this Sorcery?

**autoreply** is a lightweight Model Context Protocol (MCP) server that plugs directly into your favorite AI assistant (like Google's Gemini in VSCode or any other MCP-compatible tool). It's your personal, scriptable gateway to the BlueSky social network, giving your AI the power to read, write, and interact as if it were a person.

*   **Log in once, command forever:** Use your handle and an [app password](https://bsky.app/settings/app-passwords). It's saved to your system's secure storage and used automatically.
*   **Fly under the radar:** Access a universe of public BlueSky data—feeds, profiles, threads—without ever logging in. Perfect for research, analysis, or just satisfying your curiosity.
*   **Your AI is the client:** Wean yourself off scrolling. Now your AI assistant summarises, curates, likes and posts whatever you want.

## ✉️ autoreply: The Toolkit

These are **autoreply's** tools giving your AI access to BlueSky:

*   **feed**: Fetching unlimited posts from BlueSky, either for a general Discovery feed or any of the thousands of curated and algorithmic feeds the platform hosts.
*   **search**: Search by query and/or user, streaming results by chunks if you need to go deep.
*   **profile**: General profile information and stats BlueSky provides.
*   **thread**: Unrolls a thread with replies and so on.
*   **post**: Yes, you can post to BlueSky. And that includes QTs and replies.
*   **like & `repost`**: Very simple what it says on the tin.
*   **delete**: Natural need for when you're trying a new tool and make something silly.

The real power of MCPs is in freeing your time in small ways that add up. You don't know how much you actually hate scrolling until your AI does it for you.

## Developer Notes

The tool went through a couple iterations, starting with heavier official client and a clunky official MCP/NPM library. Probably all very good, but I wanted it to be easy and lightweight.

It's lightweigt all right now, with almost no dependencies: the core is [@atcute/client](https://www.npmjs.com/package/@atcute/client).

I do recognise the implementation is pretty dull and verbose, no unit tests. One of these weekends I will get it better, for now the key is ease of use and stability. It works well, that's the big thing.

We consider using Gemma's tokenize vocabulary, which is licensed under [the Gemma license attached](src/gemma/license.md). Users of the included Gemma file must comply with [the Gemma Prohibited Use Policy attached](src/gemma/license-prohibited-use.md).

*Allons-y!*