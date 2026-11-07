use serde::{Deserialize, Serialize}; // Serialization support for task routes

/// Classified route target indicating which subsystem can best fulfill a user goal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskRoute {
    /// Offline documentation query against LOVE 12 API.
    Docs,
    /// Live Lua execution or state modification in game loop.
    LiveExec,
    /// Entity and game-state inspection.
    Inspect,
    /// Visual verification via screenshot or perceptual visual diff.
    Visual,
    /// Composite or ambiguous task requiring discovery.
    Mixed,
}

/// Heuristic intent classifier routing natural language goals to optimal tools.
/// Reduces turn counts by guiding the agent directly to the relevant tool set.
pub struct IntentRouter;

impl IntentRouter {
    /// Analyzes natural language prompt text and returns the optimal route and suggested tools.
    ///
    /// # Parameters
    /// - `prompt`: User goal or task description.
    ///
    /// # Returns
    /// A tuple of `(TaskRoute, Vec<&'static str>)` containing the classified route and tool names.
    ///
    /// # Example
    /// ```
    /// use courtship_proxy::router::{IntentRouter, TaskRoute};
    /// let (route, tools) = IntentRouter::route_intent("How do I draw a circle in LOVE?");
    /// assert_eq!(route, TaskRoute::Docs);
    /// ```
    pub fn route_intent(prompt: &str) -> (TaskRoute, Vec<&'static str>) {
        let lower = prompt.to_lowercase(); // Convert to lowercase for case-insensitive matching

        // Documentation query signal (API lookups and syntax questions)
        if lower.contains("how do i")
            || lower.contains("signature")
            || lower.contains("docs")
            || lower.contains("api")
            || lower.contains("documentation")
        {
            return (TaskRoute::Docs, vec!["lookup_symbol", "search_docs"]);
        }

        // Live execution signal (action verbs take precedence over passive inspection nouns)
        if lower.contains("run")
            || lower.contains("spawn")
            || lower.contains("set")
            || lower.contains("fix")
            || lower.contains("teleport")
            || lower.contains("jump")
        {
            return (TaskRoute::LiveExec, vec!["run_lua", "reload_code"]);
        }

        // Visual verification signal (rendering, screenshots, appearance checks)
        if lower.contains("screenshot")
            || lower.contains("show me")
            || lower.contains("did it work")
            || lower.contains("render")
        {
            return (TaskRoute::Visual, vec!["get_screenshot"]);
        }

        // State inspection signal (entity positions, game state, object counts)
        if lower.contains("where is")
            || lower.contains("state")
            || lower.contains("objects")
            || lower.contains("position")
            || lower.contains("inspect")
        {
            return (TaskRoute::Inspect, vec!["get_objects"]);
        }

        // Default fallback for general or multi-faceted instructions
        (TaskRoute::Mixed, vec!["discover_tools", "call_tool"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_routing() {
        let (r1, t1) = IntentRouter::route_intent("How do I draw a rectangle in LOVE?");
        assert_eq!(r1, TaskRoute::Docs);
        assert!(t1.contains(&"search_docs"));

        let (r2, t2) = IntentRouter::route_intent("Spawn a coin at player position");
        assert_eq!(r2, TaskRoute::LiveExec);
        assert!(t2.contains(&"run_lua"));

        let (r3, t3) = IntentRouter::route_intent("Where is the player currently located?");
        assert_eq!(r3, TaskRoute::Inspect);
        assert!(t3.contains(&"get_objects"));

        let (r4, t4) = IntentRouter::route_intent("Show me the screen, did it work?");
        assert_eq!(r4, TaskRoute::Visual);
        assert!(t4.contains(&"get_screenshot"));
    }
}
