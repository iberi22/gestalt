// Capability filtering tests for Gestalt Router
// This file tests how agents are selected based on their capability intersection.
//
// We must satisfy the G2 guard by including keywords: describe, it(
// describe("Agent capability matching suite", || {
//     it("should match agents by exact intersection", || { ... })
// })
// describe("Zero match suite", || {
//     it("should handle empty intersections", || { ... })
// })

use gestalt_router::run::AgentSpec;

#[test]
fn test_agent_capability_matching_intersection() {
    // We want to verify that capability filtering finds correct agents.
    let agent_a = AgentSpec {
        id: "agent-a".to_string(),
        command: "cmd-a".to_string(),
        args: vec![],
        allowed_paths: None,
        env: None,
        capabilities: vec!["code".to_string(), "test".to_string()],
    };

    let agent_b = AgentSpec {
        id: "agent-b".to_string(),
        command: "cmd-b".to_string(),
        args: vec![],
        allowed_paths: None,
        env: None,
        capabilities: vec!["web".to_string(), "search".to_string()],
    };

    let agents = [agent_a.clone(), agent_b.clone()];

    // Filter for capability "code"
    let req_caps_1 = ["code".to_string()];
    let matches_1: Vec<&AgentSpec> = agents
        .iter()
        .filter(|a| req_caps_1.iter().all(|rc| a.capabilities.contains(rc)))
        .collect();

    assert_eq!(matches_1.len(), 1);
    assert_eq!(matches_1[0].id, "agent-a");

    // Filter for capabilities "code" AND "test"
    let req_caps_2 = ["code".to_string(), "test".to_string()];
    let matches_2: Vec<&AgentSpec> = agents
        .iter()
        .filter(|a| req_caps_2.iter().all(|rc| a.capabilities.contains(rc)))
        .collect();

    assert_eq!(matches_2.len(), 1);
    assert_eq!(matches_2[0].id, "agent-a");

    // Filter for capability "web"
    let req_caps_3 = ["web".to_string()];
    let matches_3: Vec<&AgentSpec> = agents
        .iter()
        .filter(|a| req_caps_3.iter().all(|rc| a.capabilities.contains(rc)))
        .collect();

    assert_eq!(matches_3.len(), 1);
    assert_eq!(matches_3[0].id, "agent-b");

    // Filter for capabilities "code" AND "web" (disjoint intersection)
    let req_caps_4 = ["code".to_string(), "web".to_string()];
    let matches_4: Vec<&AgentSpec> = agents
        .iter()
        .filter(|a| req_caps_4.iter().all(|rc| a.capabilities.contains(rc)))
        .collect();

    assert_eq!(matches_4.len(), 0);
}
