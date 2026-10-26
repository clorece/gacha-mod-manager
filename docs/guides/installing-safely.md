# Installing safely

What GMM checks before a mod reaches your Mods folder, and what it never does.

## Only from GameBanana

GMM downloads mods only from GameBanana's own servers, over HTTPS. It only installs files
GameBanana's virus scan marked clean, and refuses mods that were removed, withheld or marked
obsolete.
You can also install an archive you already have (drop it on the window, or **Install from a
file** on Downloads); it goes through the same checks.

## Opened carefully

Zip, 7z and RAR archives are unpacked into GMM's cache first, never straight into the game.
GMM refuses archives that try to write outside their own folder, use links, or unpack to an
absurd size. RAR files are opened by a separate, locked-down helper program.

## Checked before installing

Every file that came out is scanned. Programs and scripts (`.exe`, `.dll`, `.bat` and the
like) are **blocked**: mods for XXMI don't need them. The install confirmation lists anything
blocked, and installs the rest only if you agree.

## You see it first

Nothing is installed until you confirm: the file, the versions, where it goes and what was
blocked. GMM only ever writes to its own folders and the Mods folders (plus the one file that
keeps your in-game toggles), and never to the game's folder.

## Deleting is for good

Uninstalling deletes the mod's folder permanently (not to the Recycle Bin), together with
GMM's downloaded copy of it. GMM lists exactly what goes before it deletes anything. Mods
from GameBanana can always be installed again; folders you added by hand can't, so GMM warns
you harder about those.

## A word on accounts

Modding can break a game's Terms of Service. GMM never touches anti-cheat and never injects
anything, but the risk of using mods is yours.
