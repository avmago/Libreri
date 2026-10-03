# 26. Links to web pages, videos and recordings

Status: accepted.

Libreri lets you link web pages, videos, recordings on your computer and other books to a passage or a page. Videos play in the side panel from a start time.

## Decisions

- **Players:** YouTube and Vimeo; other video sites that offer oEmbed (PeerTube, Dailymotion, TED and the like); and video and audio files on this computer.
- **Details:** fetched online once, when the link is added (title, channel or author, length, picture), then kept offline.
- **Web pages:** the link plus an offline copy of the page, kept in the notes folder.

## How links are kept

- **The annotation:** a link is an annotation of kind `link`.
  - Its locator is the reader's own locator (selected text, or the page) plus `link`, which holds `url`, `kind` (`video`, `web`, `file` or `book`), `title`, `site`, `author`, `description`, `duration`, `start` (seconds), `video` (YouTube or Vimeo id), `embed` (another site's player), `picture`, `copy`, `file` and `media`.
  - Validation allows only `http(s)` and `libreri://book/` addresses, or a file. `picture` and `copy` must be under `Notes/`.
- **Files Libreri makes** go in the profile's notes folder, so they travel with the notes:
  - `Notes/<profile>/Links/<title>.jpg|png|webp…`: the picture.
  - `Notes/<profile>/Web pages/<title>.html`: the offline copy.
  - Deleting the link keeps them, as for voice notes and paper notes, so Undo works.
- **Files you link:** the path is relative to the library when the file is inside it, so the link travels with the library. Outside the library it is a full path, which works on this computer only; the dialog says so.
- **Other books:** a `libreri://book/<id>…` address, opened like any Libreri link and followed through aliases.

## Fetching details (`libreri-links`)

- **YouTube and Vimeo** are recognised from the address, with no network. This covers watch, `youtu.be`, shorts, embed and live links, unlisted Vimeo hashes, and start times such as `t=1m30s`, `start=90` or `#t=90`.
  - Their details come from the sites' oEmbed endpoints.
  - YouTube's length is read from the video page's `duration` tag.
- **Other pages:** the page is fetched once.
  - Its oEmbed link, if it offers one, gives a video's title, author, length and player frame. Only `https` players are accepted.
  - Otherwise its Open Graph tags and title are used.
  - Pictures must be JPEG, PNG, GIF, WebP or AVIF, and at most 4 MB. SVG is refused, because it can carry scripts.
- **The offline copy:**
  - The readable part of the page is picked out with `dom_smoothie`, a Rust port of Mozilla's Readability, and cleaned with `ammonia`: no scripts, styles, frames, forms or event handlers, and links made absolute.
  - Pictures are put inside the file as data, up to 60 of them and 25 MB. Pictures that could not be kept are left out, never fetched later.
  - The file carries a Content Security Policy that allows nothing but its own pictures and styles. `book://` also serves `.html` files with a sandboxing policy, and the viewer shows them in a sandboxed frame.
  - The copy says where and when it was saved, and can be opened in a browser.
- **After adding:** nothing is fetched again. Playing a video needs the internet, of course.

## Playing

- **The mini player** sits at the top of the reader's *Links* tab. The tab is wider than the other tabs, and the player starts at the link's start time.
- **YouTube** plays through `youtube-nocookie.com` (privacy-enhanced mode); **Vimeo** plays with `dnt=1`.
- **Embedded players need a web address.**
  - YouTube refuses to play in a frame whose page has no web referrer. Libreri's pages are `tauri://` on macOS and Linux.
  - So the frame shows a tiny page served on this computer: `http://127.0.0.1:<port>/<token>/play?src=…`, from `libreri-links::player`.
  - That server listens on the loopback address only, answers only its random token, and only frames `https` addresses.
  - The app's CSP `frame-src` allows `http://127.0.0.1:*`.
- **Buttons:**
  - *Play in its own window* opens the same page in a separate window, which has no access to the app.
  - *Open in browser* opens the video's own page at the start time.
- **Files on this computer** play in `<video>` or `<audio>` through `book://…/.media/<token>/<name>`, with range requests so seeking works.
  - The token is made when the link is played, and it maps only to that file.
  - *Open in another app* hands the file to the system.
- **No site logos:** a video, globe, file or book icon plus the site's name.

## In the app

- **Adding a link:**
  - From a selection: the link button in the selection menu.
  - For the page: the toolbar's link button, the Links tab's *Add a link here*, or Alt+L. The shortcut can be changed.
  - The dialog fetches details when an address is pasted. Title, start time and note can be changed, and *Keep an offline copy* is on by default for web pages.
  - If fetching fails (offline), the address can still be saved with your own title.
- **On the page:** linked text gets a dashed blue underline. Clicking it shows the link, with *Play from 1:30*, *Open in browser* or *Offline copy*, a comment, and delete.
  - The status bar shows "2 links on this page" and opens the Links tab.
- **Links tab:** the player, then this page's links, then the rest of the book's. Each can be played or opened, its offline copy read, shown in the book, or deleted.
- **Notes hub:** a *Links* filter, cards with the picture, and *Open*, *Watch in browser* or *Offline copy*. Search also matches link titles, addresses and sites.
- **Exports:**
  - Markdown from the Notes hub, and Obsidian book notes (a *Links* section), write each link as a normal Markdown link.
  - Videos keep their start time (`&t=90s` or `#t=90s`), followed by the place in the book.

## Fixed along the way

- **Clicking highlights, voice notes and links on PDF, DjVu and comic pages found nothing.**
  - The boxes let clicks through to the text so it stays selectable, which also hides them from `elementsFromPoint`.
  - They are now found by position (`highlightAt`), and the smallest box wins where boxes overlap.
