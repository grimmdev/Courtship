use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskRoute {
    Docs,
    LiveExec,
    Inspect,
    Visual,
    Mixed,
}

pub struct IntentRouter;

impl IntentRouter {
    pub fn route_intent(prompt: &str) -> (TaskRoute, Vec<&'static str>) {
        let lower = prompt.to_lowercase();

        // 1. Docs signal
        if lower.contains("how do i")
            || lower.contains("signature")
            || lower.contains("docs")
            || lower.contains("api")
            || lower.contains("documentation")
        {
            return (TaskRoute::Docs, vec!["lookup_symbol", "search_docs"]);
        }

        // 2. Live exec / fix signal (action verbs take precedence over passive inspection nouns)
        if lower.contains("run")
            || lower.contains("spawn")
            || lower.contains("set")
            || lower.contains("fix")
            || lower.contains("teleport")
            || lower.contains("jump")
        {
            return (TaskRoute::LiveExec, vec!["run_lua", "reload_code"]);
        }

        // 3. Visual signal
        if lower.contains("screenshot")
            || lower.contains("show me")
            || lower.contains("did it work")
            || lower.contains("render")
        {
            return (TaskRoute::Visual, vec!["get_screenshot"]);
        }

        // 4. Inspect signal
        if lower.contains("where is")
            || lower.contains("state")
            || lower.contains("objects")
            || lower.contains("position")
            || lower.contains("inspect")
        {
            return (TaskRoute::Inspect, vec!["get_objects"]);
        }

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
