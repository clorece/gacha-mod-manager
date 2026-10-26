# Fixing setup problems

What each setup state on the Dashboard means and how to get to Ready.

GMM checks every game when it starts and whenever you press **Check again**. A game that isn't
ready shows a setup card; its setup guide opens at the step that's missing. The names in quotes
are XXMI Launcher's own buttons and labels.

## XXMI not found

GMM couldn't find XXMI Launcher. Install it from its official releases page (the setup guide
has a button for it) and start it once. If you use the portable version somewhere unusual,
choose its folder in **Settings › XXMI Launcher**.

## Mod loader not installed

XXMI is there, but not the part that loads this game's mods (ZZMI for Zenless Zone Zero, WWMI
for Wuthering Waves). In XXMI Launcher, tick the game under "Select Games To Mod:", select it
and press "Install".

## Mod loader damaged

Some of the loader's files are missing or changed. Select the game in XXMI Launcher and press
"Install" again; it repairs the loader without touching your mods.

## Game folder not set, or wrong

XXMI doesn't know where the game is, or points at a folder without it. In XXMI Launcher, select
the game, open its settings and the "General" tab, then press "Detect Installations" or set
"Game Folder" yourself. Press "Start" once so XXMI saves it.

If GMM itself can't find your game (an unusual install place), add the folder in
**Settings › Games**.

## Launch setting doesn't fit

XXMI's "Game Launch" setting has to match how your copy of the game is installed; otherwise the
game closes itself when XXMI starts it. The setup card says which value to pick: in XXMI
Launcher, select the game, open its settings, "General" tab, and change "Game Launch".

## Still stuck?

Press **Check again** after each change. GMM reads XXMI's settings but never changes them, so
the fix always happens in XXMI Launcher.
