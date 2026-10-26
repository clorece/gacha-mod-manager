# How mods load

What happens between pressing Launch and seeing a mod in game.

## XXMI does the loading

XXMI Launcher starts the game and adds its mod loader (built on 3Dmigoto) as the game starts.
The loader reads every mod in the game's **Mods** folder, for example `ZZMI\Mods` for Zenless
Zone Zero or `WWMI\Mods` for Wuthering Waves. Nothing is written into the game's own files, so
starting the game any other way starts it without mods.

## What GMM does

GMM puts mods in that Mods folder (in a `GMM` folder of its own), switches them on and off,
and starts XXMI for you. It never changes the game, never injects anything itself, and never
changes XXMI's files.

## On and off

Switching a mod off renames its folder to start with `DISABLED `. XXMI skips folders named that
way, and nothing is deleted. Changes take effect the next time the game starts.

Mods often have their own in-game toggles (a key that swaps an outfit part, for example). The
loader saves those choices in one file, `d3dx_user.ini`, and forgets a mod's choices once the
mod stops loading. GMM copies a mod's lines from that file before switching it off and puts
them back when it comes on, so your choices survive. It only does this while the game isn't
running.

## Versions

Some mods come as several versions in one download (different colours, with or without an
accessory). GMM shows them as one card. In **Mod settings › Versions** you choose which one is
on; switching one on switches off the others that change the same things.

## Mods you added by hand

Folders you put in the Mods folder yourself show up too, marked **Added by hand**. You can
switch them, delete them, or use **Find on GameBanana** so GMM can manage and update them.
