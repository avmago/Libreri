//! RSS and Atom feeds (ADR 0027): subscriptions per profile, sorted into
//! folders, with new papers and articles to download or delete, and to
//! add to the library.
//!
//! - [`parse`]: reading RSS, Atom and JSON Feed.
//! - [`fetch`]: fetching feeds ("not modified" answers), finding a site's
//!   feed, and downloading PDFs and pages.
//! - [`state`]: a profile's folders, feeds and items (`feeds.json`).
//! - [`opml`], [`sources`]: OPML files, arXiv's categories and suggested
//!   sources.
//! - [`article`]: articles kept as Markdown with front matter.

pub mod article;
pub mod fetch;
pub mod opml;
pub mod parse;
pub mod podcasts;
pub mod sources;
pub mod state;

pub use parse::{FeedEntry, Parsed};
pub use state::{Feed, FeedFolder, FeedItem, FeedSettings, State};
