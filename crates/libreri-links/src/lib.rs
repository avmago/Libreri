//! Links to web pages and videos from a place in a book (Phase 8c,
//! ADR 0026).
//!
//! - [`address`]: what an address is (a YouTube or Vimeo video and its
//!   start time, a web page), with no network.
//! - [`fetch`]: details fetched once when a link is added (oEmbed and the
//!   page's own tags); after that the link works offline.
//! - [`copy`]: an offline copy of a web page, as one safe HTML file.
//! - [`player`]: the local page that holds embedded players.

pub mod address;
pub mod copy;
pub mod fetch;
pub mod player;

pub use address::{embed_url, format_time, parse_time, watch_url, KnownVideo};
pub use fetch::{fetch, Fetched, LinkDetails, LinkKind};
pub use player::PlayerServer;
