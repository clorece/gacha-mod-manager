# Conflicts

Why two mods sometimes can't both show, and what GMM does about it.

## What a conflict is

Mods for XXMI work by replacing parts of the game's models and textures. Each part has an
identifier (a hash). When two mods that are both on replace the same part, only one of them
shows in game, and which one wins isn't up to you. That's a conflict.

## How GMM finds them

GMM reads each mod's `.ini` files to see which parts it replaces, and compares the mods that
are on. It only reads; it never edits a mod.

## When you switch a mod on

If the mod would clash with one that's already on, the Library asks first and names the other
mod. **Keep it off** leaves things as they were; **Enable anyway** switches it on and leaves
both on. **Enable all** follows the same rule: mods that would clash stay off, and GMM tells
you which.

## Badges

A **Conflict** badge on a card (in the Library and the Mods Store) means the mod is on and
replaces something another mod that's on replaces too. Switch one of them off to fix it.

## Versions of one mod

Versions in one download often replace the same parts (eight colours of one weapon, say).
GMM knows this from the same check: switching one version on in **Mod settings › Versions**
switches the clashing ones off for you.
