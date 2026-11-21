Quire is now **Zotebook**, part of the Klectr Z suite. 0.6.0 adds page banners and icons, a journal calendar, folder notes and emoji folder icons, GitHub sync and a new app icon.

## Download

| Platform | File | How to start it |
| --- | --- | --- |
| Linux, x86-64 | `zotebook-0.6.0-linux-x86_64.tar.gz` | Unpack it and run `./install.sh`. Then open Zotebook from the app launcher, or run `zotebook`. |
| macOS, Apple silicon | `zotebook-0.6.0-macos-arm64.zip` | Unzip it and move `Zotebook.app` to Applications. The app is not notarized, so right-click it and choose **Open** the first time. |
| Windows, x86-64 | `zotebook-0.6.0-windows-x86_64.zip` | Unzip it and run `zotebook.exe`. The app is not signed, so Windows SmartScreen can warn you. Choose **More info**, then **Run anyway**. |

The Linux build needs Wayland or X11 with Vulkan. The macOS and Windows builds come from CI and are not tested on those systems yet. Please open an issue if they do not start.

## Coming from Quire

- The first time Zotebook starts, it moves Quire's settings over: your notes folders, open tabs, themes and sync keys. Folders that synced to `quire/<folder name>/` keep that remote folder. Notes locked in Quire open in Zotebook.
- Quire's **Update** button can't install Zotebook, so download it once from this page. Later versions update themselves.
- On macOS, delete `Quire.app` after you install `Zotebook.app`.
- If an AI app starts Quire's MCP server, paste the new configuration from **Settings, MCP**.

## New

- **Page banners.** Hover over a note's name and choose **Add banner**, or run "Add banner…" from the command palette. The image is copied into `attachments/` and kept in the note's front matter as `banner:`. Hover over the banner to change or remove it.
- **Page icons.** **Add icon** gives a note an emoji. It goes at the start of the file name, so the file tree shows it too, and links to the note are rewritten. The icon shows on the page and opens the picker when you click it.
- **Journal calendar.** A small calendar above the outline marks today and puts a dot under each day with a note. Click a day to open its entry or start one in `Journal/`. **Today** jumps back.
- **Folder notes.** A note with its folder's name, such as `Projects/Projects.md`, is the folder's page. Click the folder's name to open it; the chevron opens the folder.
- **Folder icons.** Give a folder any emoji as its icon. The journal still finds its folder when it has one.
- **GitHub sync.** A notes folder that is the root of a Git repository syncs with its remote, using the Git credentials already on the computer.
- **Front matter.** A note's YAML front matter stays out of the preview.
- **Maple Mono** is a new editor font.
- **A new app icon**, in the style of the other Z apps.

## Fixed

- Preview mode stays on when you open a folder's note or a search result.
- The folder watcher no longer undoes typing when a save lands while you type.

## Removed

- The Android app.
