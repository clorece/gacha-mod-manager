# Privacy

No accounts, no tracking: nothing leaves your computer except requests to GameBanana.

## Network

GMM only talks to GameBanana: its website for mod information, and its image and file servers
for previews and downloads. Every request goes over HTTPS to an address built into the app;
anything else is refused. There's no GMM server, no account and no telemetry. The app window
itself never connects to the internet: pictures come through GMM, which fetches them from
GameBanana and keeps a copy in its cache.

GameBanana sees the requests like it sees a browser's: your IP address and that they come
from GMM. GMM keeps its traffic low, and pauses everything if GameBanana asks it to slow down.

Opening a page in your browser (a mod's GameBanana page, XXMI's releases, a link in a
description) always asks you first and shows the address.

## What GMM stores

On your computer only: a small database with your settings, the mods GMM installed, profiles
and your saved in-game toggles; a cache of downloaded archives, preview images and GameBanana
answers. **Settings › Storage** shows the sizes and clears each cache.

## What GMM reads

GMM looks for your games (in Steam, HoYoPlay and Epic's own lists) and reads a few of XXMI
Launcher's settings: where it is, which games it handles, each game's folder and launch
setting. It never reads passwords, tokens, login files or game logs, and XXMI's settings file
is read into a list of only the fields GMM needs.
