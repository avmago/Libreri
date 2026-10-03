# 8. Dragging inside the window uses pointer events

Status: accepted.

Files dropped from the desktop arrive through Tauri's drag-and-drop events. Dragging books and folders inside the window (onto sidebar folders, folder tiles) is built on pointer events instead of HTML5 drag-and-drop, because on Windows the webview cannot offer both at once. Drop targets are marked with `data-drop-folder`.
