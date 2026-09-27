//! Small Calibre and Zotero libraries for tests (here and in
//! `libreri-library`). Not used by the app.

use rusqlite::Connection;
use std::path::Path;

/// A small Calibre library with two books, one with two formats.
pub fn calibre_fixture(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let conn = Connection::open(dir.join("metadata.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE books (id INTEGER PRIMARY KEY, title TEXT, sort TEXT, timestamp TEXT,
            pubdate TEXT, series_index REAL, author_sort TEXT, isbn TEXT, path TEXT,
            has_cover BOOL, uuid TEXT, last_modified TEXT);
         CREATE TABLE authors (id INTEGER PRIMARY KEY, name TEXT, sort TEXT);
         CREATE TABLE books_authors_link (id INTEGER PRIMARY KEY, book INTEGER, author INTEGER);
         CREATE TABLE tags (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE books_tags_link (id INTEGER PRIMARY KEY, book INTEGER, tag INTEGER);
         CREATE TABLE series (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE books_series_link (id INTEGER PRIMARY KEY, book INTEGER, series INTEGER);
         CREATE TABLE publishers (id INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE books_publishers_link (id INTEGER PRIMARY KEY, book INTEGER, publisher INTEGER);
         CREATE TABLE languages (id INTEGER PRIMARY KEY, lang_code TEXT);
         CREATE TABLE books_languages_link (id INTEGER PRIMARY KEY, book INTEGER, lang_code INTEGER, item_order INTEGER);
         CREATE TABLE ratings (id INTEGER PRIMARY KEY, rating INTEGER);
         CREATE TABLE books_ratings_link (id INTEGER PRIMARY KEY, book INTEGER, rating INTEGER);
         CREATE TABLE comments (id INTEGER PRIMARY KEY, book INTEGER, text TEXT);
         CREATE TABLE identifiers (id INTEGER PRIMARY KEY, book INTEGER, type TEXT, val TEXT);
         CREATE TABLE data (id INTEGER PRIMARY KEY, book INTEGER, format TEXT, uncompressed_size INTEGER, name TEXT);

         INSERT INTO books VALUES (1, 'Dune', 'Dune', '2021-03-04 05:06:07+00:00',
            '1965-08-01 00:00:00+00:00', 1.0, 'Herbert, Frank', '', 'Frank Herbert/Dune (1)', 1, 'u1', '');
         INSERT INTO books VALUES (2, 'Notes', 'Notes', '2021-03-04 05:06:07+00:00',
            '0101-01-01 00:00:00+00:00', 1.0, '', '', 'Unknown/Notes (2)', 0, 'u2', '');
         INSERT INTO authors VALUES (1, 'Frank Herbert', 'Herbert, Frank');
         INSERT INTO books_authors_link VALUES (1, 1, 1);
         INSERT INTO tags VALUES (1, 'Science Fiction'), (2, 'Classics');
         INSERT INTO books_tags_link VALUES (1, 1, 1), (2, 1, 2);
         INSERT INTO series VALUES (1, 'Dune');
         INSERT INTO books_series_link VALUES (1, 1, 1);
         INSERT INTO publishers VALUES (1, 'Chilton');
         INSERT INTO books_publishers_link VALUES (1, 1, 1);
         INSERT INTO languages VALUES (1, 'eng');
         INSERT INTO books_languages_link VALUES (1, 1, 1, 0);
         INSERT INTO ratings VALUES (1, 8);
         INSERT INTO books_ratings_link VALUES (1, 1, 1);
         INSERT INTO comments VALUES (1, 1, '<p>A desert planet, spice and a young duke.</p>');
         INSERT INTO identifiers VALUES (1, 1, 'isbn', '9780441013593');
         INSERT INTO data VALUES (1, 1, 'MOBI', 10, 'Dune - Frank Herbert');
         INSERT INTO data VALUES (2, 1, 'EPUB', 10, 'Dune - Frank Herbert');
         INSERT INTO data VALUES (3, 2, 'TXT', 10, 'Notes');",
    )
    .unwrap();
    let book = dir.join("Frank Herbert/Dune (1)");
    std::fs::create_dir_all(&book).unwrap();
    std::fs::write(book.join("Dune - Frank Herbert.epub"), "epub bytes").unwrap();
    std::fs::write(book.join("Dune - Frank Herbert.mobi"), "mobi bytes").unwrap();
    std::fs::write(book.join("cover.jpg"), "jpg").unwrap();
    let notes = dir.join("Unknown/Notes (2)");
    std::fs::create_dir_all(&notes).unwrap();
    std::fs::write(notes.join("Notes.txt"), "some notes").unwrap();
}

/// A tiny Zotero data folder: a paper with a PDF, two highlights, a
/// note and a collection; a book without a file; an item in the trash.
pub fn zotero_fixture(dir: &Path) {
    std::fs::create_dir_all(dir.join("storage/ATTKEY01")).unwrap();
    std::fs::write(
        dir.join("storage/ATTKEY01/Vaswani - 2017 - Attention.pdf"),
        "%PDF",
    )
    .unwrap();
    let conn = Connection::open(dir.join("zotero.sqlite")).unwrap();
    conn.execute_batch(
        "CREATE TABLE items (itemID INTEGER PRIMARY KEY, itemTypeID INT, dateAdded TEXT, dateModified TEXT, libraryID INT, key TEXT);
         CREATE TABLE itemTypes (itemTypeID INTEGER PRIMARY KEY, typeName TEXT);
         CREATE TABLE fields (fieldID INTEGER PRIMARY KEY, fieldName TEXT);
         CREATE TABLE itemData (itemID INT, fieldID INT, valueID INT);
         CREATE TABLE itemDataValues (valueID INTEGER PRIMARY KEY, value);
         CREATE TABLE creators (creatorID INTEGER PRIMARY KEY, firstName TEXT, lastName TEXT, fieldMode INT);
         CREATE TABLE creatorTypes (creatorTypeID INTEGER PRIMARY KEY, creatorType TEXT);
         CREATE TABLE itemCreators (itemID INT, creatorID INT, creatorTypeID INT, orderIndex INT);
         CREATE TABLE tags (tagID INTEGER PRIMARY KEY, name TEXT);
         CREATE TABLE itemTags (itemID INT, tagID INT, type INT);
         CREATE TABLE collections (collectionID INTEGER PRIMARY KEY, collectionName TEXT, parentCollectionID INT, libraryID INT);
         CREATE TABLE collectionItems (collectionID INT, itemID INT);
         CREATE TABLE itemAttachments (itemID INTEGER PRIMARY KEY, parentItemID INT, linkMode INT, contentType TEXT, path TEXT);
         CREATE TABLE itemAnnotations (itemID INTEGER PRIMARY KEY, parentItemID INT, type INT, authorName TEXT, text TEXT, comment TEXT, color TEXT, pageLabel TEXT, sortIndex TEXT, position TEXT, isExternal INT);
         CREATE TABLE itemNotes (itemID INTEGER PRIMARY KEY, parentItemID INT, note TEXT, title TEXT);
         CREATE TABLE deletedItems (itemID INTEGER PRIMARY KEY, dateDeleted TEXT);

         INSERT INTO itemTypes VALUES (1,'conferencePaper'),(2,'attachment'),(3,'annotation'),(4,'note'),(5,'book');
         INSERT INTO fields VALUES (1,'title'),(2,'date'),(3,'proceedingsTitle'),(4,'DOI'),(5,'extra'),(6,'abstractNote'),(7,'ISBN'),(8,'publisher');
         INSERT INTO items VALUES
           (1,1,'2024-01-02 03:04:05','2024-01-02 03:04:05',1,'PAPER001'),
           (2,2,'2024-01-02 03:04:05','2024-01-02 03:04:05',1,'ATTKEY01'),
           (3,3,'2024-01-03 03:04:05','2024-01-04 03:04:05',1,'ANNOT001'),
           (4,3,'2024-01-03 03:04:05','2024-01-03 03:04:05',1,'ANNOT002'),
           (5,4,'2024-01-03 03:04:05','2024-01-03 03:04:05',1,'NOTE0001'),
           (6,5,'2024-01-05 03:04:05','2024-01-05 03:04:05',1,'BOOK0001'),
           (7,5,'2024-01-05 03:04:05','2024-01-05 03:04:05',1,'TRASHED1');
         INSERT INTO itemDataValues VALUES (1,'Attention Is All You Need'),(2,'2017-00-00 2017'),
           (3,'Advances in Neural Information Processing Systems'),(4,'10.48550/arXiv.1706.03762'),
           (5,'arXiv: 1706.03762'),(6,'The dominant sequence transduction models are based on complex recurrent networks.'),
           (7,'Dune'),(8,'978-0-441-01359-3'),(9,'Gone');
         INSERT INTO itemData VALUES (1,1,1),(1,2,2),(1,3,3),(1,4,4),(1,5,5),(1,6,6),(6,1,7),(6,7,8),(7,1,9);
         INSERT INTO creators VALUES (1,'Ashish','Vaswani',0),(2,'','Google Brain',1);
         INSERT INTO creatorTypes VALUES (1,'author'),(2,'editor');
         INSERT INTO itemCreators VALUES (1,1,1,0),(1,2,2,1);
         INSERT INTO tags VALUES (1,'transformers');
         INSERT INTO itemTags VALUES (1,1,0);
         INSERT INTO collections VALUES (1,'ML',NULL,1),(2,'NLP',1,1);
         INSERT INTO collectionItems VALUES (2,1);
         INSERT INTO itemAttachments VALUES (2,1,0,'application/pdf','storage:Vaswani - 2017 - Attention.pdf');
         INSERT INTO itemAnnotations VALUES
           (3,2,1,'','attention function','Key idea','#5fb236','3','','{\"pageIndex\":2,\"rects\":[[100,600,300,612]]}',0),
           (4,2,4,'','','','#ffd400','3','','{\"pageIndex\":2,\"paths\":[]}',0);
         INSERT INTO itemNotes VALUES (5,1,'<p>Read <b>section 3</b> again.</p>','');
         INSERT INTO deletedItems VALUES (7,'2024-02-01');",
    )
    .unwrap();
}
