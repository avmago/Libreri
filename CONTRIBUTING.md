# Contributing to Libreri

Thank you for wanting to help. Bug reports, ideas, fixes and translations are all welcome.

## Reporting a bug or an idea

Open an [issue](https://github.com/avmg0/Libreri/issues). For a bug, say which system you use (macOS, Windows or Linux, and the version), what you did, what you expected, and what happened instead. A screenshot helps.

## Contributing code

1. Fork the repository and make your change on a branch.
2. Follow [code structure](docs/code-structure.md) for where code goes, and [data portability](docs/data-portability.md) before touching anything that links notes to books.
3. Run the checks before you open a pull request:
   ```sh
   pnpm lint
   pnpm test
   pnpm format
   ```
4. Open a pull request that explains what changed and why.

## Contributor agreement

The first time you open a pull request, a bot asks you to sign the [contributor licence agreement](CLA.md) by adding a comment. You keep the copyright to your work; the agreement lets Libreri stay free software under the GPL while keeping the option to publish it in other places or under other terms later. Every version that includes your work stays available under the GPL.

## Licence

Libreri is under the [GNU General Public License, version 3 or later](LICENSE).
