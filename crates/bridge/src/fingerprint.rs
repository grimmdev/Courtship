use std::path::{Path, PathBuf}; // Filesystem path representations
use std::fs; // Filesystem inspection functions
use serde::{Deserialize, Serialize}; // Serialization support for fingerprint reports

/// Detected architectural patterns and entity conventions for a LOVE game codebase.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArchitectureFingerprint {
    /// Detected OOP or Entity-Component-System framework (e.g. Classic, Middleclass, Concord, Tiny-ECS).
    pub oop_system: Option<String>,
    /// Detected collision or physics library (e.g. Bump.lua, HC, love.physics).
    pub collision_physics: Option<String>,
    /// Detected scene or state management library (e.g. HUMP.gamestate, Roomy).
    pub state_management: Option<String>,
    /// Detected tilemap or map engine (e.g. STI, Cartographer).
    pub map_engine: Option<String>,
    /// Detected graphical user interface library (e.g. Gspot, SUIT).
    pub ui_library: Option<String>,
    /// High-level guidance string injected into agent prompts to direct tool usage.
    pub agent_hint: String,
}

/// Static analyzer scanning Lua source files to fingerprint game engine architecture.
pub struct ProjectFingerprinter;

impl ProjectFingerprinter {
    /// Scans a game project directory to detect engine architecture conventions.
    pub fn scan<P: AsRef<Path>>(dir: P) -> ArchitectureFingerprint {
        let mut fp = ArchitectureFingerprint::default();
        let mut scanned_sources = Vec::new();

        Self::collect_lua_files(dir.as_ref(), &mut scanned_sources, 3);

        let mut combined_content = String::new();
        for file in scanned_sources {
            if let Ok(text) = fs::read_to_string(file) {
                combined_content.push_str(&text);
                combined_content.push('\n');
            }
        }

        let lower = combined_content.to_lowercase();

        // Detect OOP / Entity Framework
        if lower.contains("concord.entity") || lower.contains("concord.component") {
            fp.oop_system = Some("Concord (ECS)".to_string());
        } else if lower.contains("tiny.system") || lower.contains("tiny.processingSystem") {
            fp.oop_system = Some("Tiny-ECS".to_string());
        } else if lower.contains("classic:extend") || lower.contains("require(\"classic\")") || lower.contains("require('classic')") {
            fp.oop_system = Some("Classic OOP (Class:extend)".to_string());
        } else if lower.contains("middleclass") || lower.contains("class('") {
            fp.oop_system = Some("Middleclass OOP".to_string());
        } else if lower.contains("hump.class") || lower.contains("class{") {
            fp.oop_system = Some("HUMP.class".to_string());
        }

        // Detect Physics / Collision
        if lower.contains("bump.newworld") || lower.contains("require(\"bump\")") || lower.contains("require('bump')") {
            fp.collision_physics = Some("Bump.lua (AABB / tile collision)".to_string());
        } else if lower.contains("love.physics.newworld") {
            fp.collision_physics = Some("Box2D (love.physics)".to_string());
        } else if lower.contains("hardoncollider") {
            fp.collision_physics = Some("HardonCollider".to_string());
        }

        // Detect Gamestate / Scene
        if lower.contains("gamestate.switch") || lower.contains("hump.gamestate") {
            fp.state_management = Some("HUMP Gamestate".to_string());
        } else if lower.contains("roomy.new") {
            fp.state_management = Some("Roomy Scene Manager".to_string());
        }

        // Detect Tilemap Engine
        if lower.contains("sti(") || lower.contains("require(\"sti\")") || lower.contains("require('sti')") {
            fp.map_engine = Some("STI (Simple Tiled Implementation)".to_string());
        }

        // Generate concise, token-efficient agent guidance hint
        let mut hint_parts = Vec::new();
        if let Some(ref oop) = fp.oop_system {
            hint_parts.push(format!("OOP: {}", oop));
        }
        if let Some(ref col) = fp.collision_physics {
            hint_parts.push(format!("Physics: {}", col));
        }
        if let Some(ref sm) = fp.state_management {
            hint_parts.push(format!("Scenes: {}", sm));
        }

        fp.agent_hint = if hint_parts.is_empty() {
            "Standard LOVE game loop; no third-party ECS or scene libraries detected.".to_string()
        } else {
            format!("Detected stack: {}. Target these APIs when inspecting or extending game entities.", hint_parts.join(" | "))
        };

        fp
    }

    fn collect_lua_files(dir: &Path, out: &mut Vec<PathBuf>, max_depth: usize) {
        if max_depth == 0 { return; }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    Self::collect_lua_files(&p, out, max_depth - 1);
                } else if p.extension().and_then(|e| e.to_str()) == Some("lua") {
                    out.push(p);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprinting() {
        let temp = tempfile::tempdir().unwrap();
        let main_path = temp.path().join("main.lua");
        fs::write(
            &main_path,
            r#"
            local bump = require("bump")
            local world = bump.newWorld(64)
            local Player = require("classic"):extend()
            function love.load() end
        "#,
        ).unwrap();

        let fp = ProjectFingerprinter::scan(temp.path());
        assert!(fp.oop_system.unwrap().contains("Classic"));
        assert!(fp.collision_physics.unwrap().contains("Bump"));
        assert!(fp.agent_hint.contains("Bump.lua"));
    }
}

