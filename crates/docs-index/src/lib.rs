use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DocsError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocEntry {
    pub symbol: String,
    pub module: String,
    pub signature: String,
    pub description: String,
    pub example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub symbol: String,
    pub module: String,
    pub signature: String,
    pub snippet: String,
    pub score: f64,
}

pub struct DocsIndex {
    conn: Connection,
}

impl DocsIndex {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, DocsError> {
        let conn = Connection::open(path)?;
        let index = Self { conn };
        index.init_schema()?;
        Ok(index)
    }

    pub fn in_memory() -> Result<Self, DocsError> {
        let conn = Connection::open_in_memory()?;
        let index = Self { conn };
        index.init_schema()?;
        Ok(index)
    }

    fn init_schema(&self) -> Result<(), DocsError> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS docs (
                id INTEGER PRIMARY KEY,
                symbol TEXT UNIQUE NOT NULL,
                module TEXT NOT NULL,
                signature TEXT NOT NULL,
                description TEXT NOT NULL,
                example TEXT
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS docs_fts USING fts5(
                symbol,
                module,
                signature,
                description,
                content='docs',
                content_rowid='id'
            );

            CREATE TRIGGER IF NOT EXISTS docs_ai AFTER INSERT ON docs BEGIN
                INSERT INTO docs_fts(rowid, symbol, module, signature, description)
                VALUES (new.id, new.symbol, new.module, new.signature, new.description);
            END;

            CREATE TRIGGER IF NOT EXISTS docs_ad AFTER DELETE ON docs BEGIN
                INSERT INTO docs_fts(docs_fts, rowid, symbol, module, signature, description)
                VALUES('delete', old.id, old.symbol, old.module, old.signature, old.description);
            END;
            ",
        )?;
        Ok(())
    }

    pub fn insert_entry(&self, entry: &DocEntry) -> Result<(), DocsError> {
        self.conn.execute(
            "INSERT OR REPLACE INTO docs (symbol, module, signature, description, example)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                entry.symbol,
                entry.module,
                entry.signature,
                entry.description,
                entry.example
            ],
        )?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, DocsError> {
        let sanitized_query = query
            .replace('"', "")
            .replace('*', "")
            .trim()
            .to_string();

        if sanitized_query.is_empty() {
            return Ok(Vec::new());
        }

        let fts_query = format!("{}*", sanitized_query);
        let mut stmt = self.conn.prepare(
            "SELECT d.symbol, d.module, d.signature, snippet(docs_fts, 3, '<b>', '</b>', '...', 16), bm25(docs_fts) as rank
             FROM docs_fts
             JOIN docs d ON d.id = docs_fts.rowid
             WHERE docs_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![fts_query, limit as i64], |row| {
            Ok(SearchResult {
                symbol: row.get(0)?,
                module: row.get(1)?,
                signature: row.get(2)?,
                snippet: row.get(3)?,
                score: row.get(4)?,
            })
        })?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn seed_love12_essentials(&self) -> Result<usize, DocsError> {
        let seeds = vec![
            DocEntry {
                symbol: "love.graphics.rectangle".to_string(),
                module: "graphics".to_string(),
                signature: "love.graphics.rectangle(mode, x, y, width, height)".to_string(),
                description: "Draws a rectangle on the screen with the active color.".to_string(),
                example: Some("love.graphics.rectangle('fill', 10, 10, 100, 50)".to_string()),
            },
            DocEntry {
                symbol: "love.graphics.draw".to_string(),
                module: "graphics".to_string(),
                signature: "love.graphics.draw(drawable, x, y, r, sx, sy, ox, oy)".to_string(),
                description: "Draws a Drawable object (Image, Canvas, Text) to the screen.".to_string(),
                example: Some("love.graphics.draw(my_image, 100, 100)".to_string()),
            },
            DocEntry {
                symbol: "love.physics.newWorld".to_string(),
                module: "physics".to_string(),
                signature: "love.physics.newWorld(xg, yg, sleep)".to_string(),
                description: "Creates a new Box2D physics simulation world with gravity vectors.".to_string(),
                example: Some("world = love.physics.newWorld(0, 9.81 * 64, true)".to_string()),
            },
            DocEntry {
                symbol: "love.filesystem.mount".to_string(),
                module: "filesystem".to_string(),
                signature: "love.filesystem.mount(archive, mountpoint, appendToPath)".to_string(),
                description: "Mounts a zip file or folder in the game's virtual save/search directory.".to_string(),
                example: Some("love.filesystem.mount('mods/pack1', 'content', true)".to_string()),
            },
            DocEntry {
                symbol: "love.audio.play".to_string(),
                module: "audio".to_string(),
                signature: "love.audio.play(source)".to_string(),
                description: "Plays the specified Source audio playback object.".to_string(),
                example: Some("love.audio.play(sfx_jump)".to_string()),
            },
            DocEntry {
                symbol: "love.window.setMode".to_string(),
                module: "window".to_string(),
                signature: "love.window.setMode(width, height, flags)".to_string(),
                description: "Sets the display mode and properties of the game window.".to_string(),
                example: Some("love.window.setMode(1280, 720, { resizable = true })".to_string()),
            },
            DocEntry {
                symbol: "love.keyboard.isDown".to_string(),
                module: "keyboard".to_string(),
                signature: "love.keyboard.isDown(key, ...)".to_string(),
                description: "Checks whether one or more specific keys are currently pressed.".to_string(),
                example: Some("if love.keyboard.isDown('space') then player:jump() end".to_string()),
            },
            DocEntry {
                symbol: "love.timer.getFPS".to_string(),
                module: "timer".to_string(),
                signature: "love.timer.getFPS()".to_string(),
                description: "Returns the current frames per second (FPS) of the game loop.".to_string(),
                example: Some("print('FPS: ' .. love.timer.getFPS())".to_string()),
            },
        ];

        let count = seeds.len();
        for entry in seeds {
            self.insert_entry(&entry)?;
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fts5_indexing_and_search() {
        let index = DocsIndex::in_memory().unwrap();
        index.seed_love12_essentials().unwrap();

        let results = index.search("rectangle", 5).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].symbol, "love.graphics.rectangle");

        let physics_res = index.search("physics gravity", 5).unwrap();
        assert!(!physics_res.is_empty());
        assert_eq!(physics_res[0].symbol, "love.physics.newWorld");

        let audio_res = index.search("audio playback", 5).unwrap();
        assert!(!audio_res.is_empty());
        assert_eq!(audio_res[0].symbol, "love.audio.play");
    }
}

