# Flatpak (Flathub)

Libreri's Flatpak is built from the Linux `.deb` of a GitHub release, so it is the same program as the other Linux packages. The app id is `io.github.avmg0.Libreri` (Flathub asks for an id based on a domain you control; a GitHub account gives `io.github.<name>`).

## Build it yourself

On Linux with `flatpak` and `flatpak-builder`:

```sh
flatpak install flathub org.gnome.Platform//48 org.gnome.Sdk//48
# Put the release's .deb address and sha256 in the manifest first:
curl -L -o Libreri.deb <address of Libreri_<version>_amd64.deb>
sha256sum Libreri.deb
flatpak-builder --user --install --force-clean build flatpak/io.github.avmg0.Libreri.yml
flatpak run io.github.avmg0.Libreri
```

## Submit to Flathub (once, then for each new version)

1. Make a release on GitHub (push a tag `v…`) and publish it.
2. In the manifest, set the `.deb` address and its sha256; add the version to `<releases>` in the metainfo file.
3. First time: fork [flathub/flathub](https://github.com/flathub/flathub), add these three files plus `libreri.sh` on a new branch, and open a pull request against its `new-pr` branch, following [Flathub's submission guide](https://docs.flathub.org/docs/for-app-authors/submission). Flathub then makes a repository `flathub/io.github.avmg0.Libreri` for later updates.
4. Later versions: update the address, sha256 and `<releases>` in that repository.

## What is different inside a Flatpak

- **Helper programs** (DjVuLibre, Tesseract, unar, eSpeak NG) cannot be installed from inside the sandbox. DjVu books, OCR, some comic formats and read aloud without system voices need them; the AppImage, `.deb` and `.rpm` packages do not have this limit. Bundling them into the Flatpak is the next step.
- **Updates** come from Flathub (the app's own updater should be switched off in the Flatpak build).
- The library folder can be anywhere in your home folder or Documents.
