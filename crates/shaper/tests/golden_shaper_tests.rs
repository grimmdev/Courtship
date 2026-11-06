use courtship_shaper::{
    ResponseShaper, ShaperConfig, TokenCounter, DeltaTracker
};
use serde_json::json;

#[test]
fn test_golden_get_objects_reduction() {
    let counter = TokenCounter::default();
    let shaper = ResponseShaper::new(ShaperConfig::default(), counter.clone());

    // Raw upstream payload as emitted by unshaped bridges / LovePilot
    let raw_upstream = json!([
        {
            "id": "player_main",
            "type": "Player",
            "name": "Hero",
            "x": 142.5,
            "y": 380.0,
            "vx": 0.0,
            "vy": -12.4,
            "internal_lua_ref": "table: 0x01a3f8c0",
            "unrelated_debug_metadata": {
                "heap_allocated_at": "init.lua:42",
                "collision_masks": [1, 2, 4, 8, 16],
                "draw_pass": 1,
                "deep_nested": {
                    "level1": {
                        "level2": {
                            "level3": "excessive"
                        }
                    }
                }
            },
            "empty_cache": {},
            "null_slot": null,
            "empty_str": ""
        },
        {
            "id": "coin_collectible_01",
            "type": "Coin",
            "name": "GoldCoin",
            "x": 220.0,
            "y": 300.0,
            "internal_lua_ref": "table: 0x01a3fa90",
            "empty_state": null
        }
    ]);

    let raw_tokens = counter.count_json_tokens(&raw_upstream);

    let shaped = shaper.shape_tool_result("get_objects", raw_upstream);
    let shaped_tokens = counter.count_json_tokens(&shaped);

    println!("[Golden Test] get_objects: raw={} tokens, shaped={} tokens", raw_tokens, shaped_tokens);

    // Assert exact golden shape
    assert_eq!(
        shaped,
        json!([
            {
                "id": "player_main",
                "type": "Player",
                "name": "Hero",
                "x": 142.5,
                "y": 380.0
            },
            {
                "id": "coin_collectible_01",
                "type": "Coin",
                "name": "GoldCoin",
                "x": 220.0,
                "y": 300.0
            }
        ])
    );

    // Claim 2: Response shaping cuts per-task token spend by >= 40%
    let savings_pct = ((raw_tokens - shaped_tokens) as f64 / raw_tokens as f64) * 100.0;
    println!("[Golden Test] Token savings: {:.1}%", savings_pct);
    assert!(savings_pct >= 40.0, "Savings was only {:.1}%, target >= 40%", savings_pct);
}

#[test]
fn test_golden_delta_stream_suppression() {
    let counter = TokenCounter::default();
    let tracker = DeltaTracker::new();

    let initial_frame = json!({
        "player": { "x": 50, "y": 100, "state": "idle" },
        "enemies": [ { "id": "slime_1", "hp": 20 } ]
    });

    let raw_initial_tokens = counter.count_json_tokens(&initial_frame);
    let initial_delta = tracker.compute_delta("game_loop", initial_frame).unwrap();
    assert!(initial_delta.get("_initial_snapshot").is_some());

    // 10 identical idle frames
    let mut idle_tokens = 0;
    for _ in 0..10 {
        let state = json!({
            "player": { "x": 50, "y": 100, "state": "idle" },
            "enemies": [ { "id": "slime_1", "hp": 20 } ]
        });
        if let Some(delta) = tracker.compute_delta("game_loop", state) {
            idle_tokens += counter.count_json_tokens(&delta);
        }
    }

    // 10 unsuppressed frames would cost 10 * raw_initial_tokens (~300 tokens)
    let unsuppressed_cost = raw_initial_tokens * 10;
    println!("[Golden Test] Delta suppression: unsuppressed={} tokens, delta={} tokens", unsuppressed_cost, idle_tokens);
    assert_eq!(idle_tokens, 0, "Idle frames must cost 0 tokens with delta suppression!");
}
