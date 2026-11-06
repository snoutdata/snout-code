# Updates

SnoutCode keeps itself up to date. It looks for a new version in the background, downloads it
while you work, and installs it when you restart or quit. Nothing interrupts you, and your open
files and projects are never touched until you say so.

## How an update arrives

1. **It looks.** About 30 seconds after SnoutCode starts, and every 4 hours while it stays open,
   it asks the newest release on GitHub whether there is a newer version. When there is not, you
   hear nothing.
2. **It downloads.** A new version downloads in the background. The status bar shows how far
   along it is, among the [background tasks](troubleshooting.md#see-what-the-java-engine-is-doing),
   then **Preparing update** while it is checked and made ready. The whole version is downloaded
   each time.
3. **It asks.** A card at the bottom right says **Update ready**, with three buttons:
   - **Release Notes** opens what changed in that version.
   - **Later** puts it off. **Restart to update** stays in the status bar for when you are ready.
   - **Restart** installs it and starts SnoutCode again on the new version.
4. **It installs.** If you never press Restart, quitting installs it quietly, and the next start
   is on the new version.

After an update, a card says **Updated to** the new version, with its release notes one click
away.

If you have unsaved changes, or something is running, Restart asks first, with the same dialog as
quitting. A Restart with nothing to lose does not ask, even with **Confirm before quitting** on.

## Checking by hand

**Check for Updates…** is in the SnoutCode menu on a Mac and the Help menu on Linux and Windows,
and in the command palette (⇧⌘P). It answers every time: up to date, or a new version on its way,
or why it could not tell. **Help ▸ Release Notes** opens the notes of the version you are running.

## Turning it off

**Settings ▸ Appearance ▸ Updates ▸ Check for updates automatically** stops SnoutCode from looking
by itself, and so from downloading anything. Check for Updates still works when you ask. As JSON,
that is `"update_check": false` (see [Settings and keymap](settings-and-keymap.md)).

## On each system

| System | What happens |
|---|---|
| macOS | The new app replaces the old one in one step, in the folder it is installed in. It takes under a second. |
| Windows | The new version's installer runs. On a Restart you see its progress; on a plain quit it runs silently. |
| Linux | SnoutCode tells you a new version is out, and **Download** opens the download page. Install it as you installed the first one (see [SnoutCode on Linux](linux.md)). |

SnoutCode on a Mac can only replace itself from a folder you can write to. Run from the disk image,
or from a folder that belongs to someone else, it says a new version is available and offers the
download. Drag it to Applications and it updates itself from then on.

On Windows, an install for all users needs an administrator, so it only updates when you press
Restart, which asks for one. A plain quit leaves it for later, so no prompt appears after you have
already left.

## What is checked before anything runs

An update is installed only after all of these hold:

- **The release is ours.** The list of a release's files, with their checksums, is signed with a
  key kept off GitHub, and SnoutCode carries the matching public key. A release replaced by anyone
  else is refused, on every system.
- **The download is whole.** Its SHA-256 must match the signed list.
- **On a Mac, the code signature matches.** The new app must meet the same signature requirement as
  the app that is running.
- **On a Mac, it runs.** The new app is started once, briefly, to prove it starts and is the version
  the list named.

A refused download is deleted, and a card says so. Nothing is installed.

## When an update goes wrong

An update never leaves you without an app. If anything fails, the version you have stays as it was,
and a Restart starts it again. If a version has been tried twice and still has not arrived, SnoutCode
stops trying and offers the download instead.

A quit caused by the system (a signal, or the session ending) installs nothing. The update waits for
the next one.

Each step goes to `update.log`, among [SnoutCode's logs](troubleshooting.md#an-update-does-not-arrive).

## Builds from source

A build you made yourself has no update feed and never asks for one. Check for Updates says so, and
you update it by building the latest source again.

## Making a release

For maintainers. The feed is `latest.json` and `latest.json.sig`, attached to each GitHub release.
Installed apps read them from the newest published release, so a draft is invisible to them.

1. Set the version in `Cargo.toml`, and push the tag `v<version>`. Its Windows and Linux runs build
   the packages; a tag's builds are the ones marked as releases (`SNOUT_OFFICIAL_BUILD`).
2. Run `scripts/package-mac.sh` on a Mac, with the Developer ID set up. It also writes the `.zip`
   apps update from.
3. Run `scripts/make-release.sh <version> <notes file>`. It collects the packages, signs the feed
   with the key `SNOUT_UPDATE_KEY` names, and makes a draft release. It refuses a key the app would
   not accept.
4. Look the draft over, and publish it on GitHub. Apps hear of it within 4 hours.

The signing key cannot be replaced: every installed app trusts it and no other, so without it they
can never update again. Keep it backed up, and never in a repository.

To try an update without a release, a development build reads a local signed feed from
`SNOUT_UPDATE_FEED`. `SNOUT_NO_UPDATE=1` keeps an app from looking at all.
