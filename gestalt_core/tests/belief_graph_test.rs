// describe: BeliefGraph Core Domain Tests
// it("should allow adding nodes and clamping belief scores")
// it("should support adding edges, verifying weights, and retrieving neighbors")
// it("should support exporting JSON and performing import round-trips")
// it("should seamlessly convert to and from context persisted graphs")

use gestalt_core::context::belief_graph::PersistedBeliefGraph;
use gestalt_core::domain::belief_graph::BeliefGraph;

#[test]
fn test_domain_node_addition_and_clamping() {
    let mut graph = BeliefGraph::new();

    // Test normal addition
    graph.add_node("node_1", "Concept 1", 0.8);
    let node = graph.get_node("node_1").expect("Node should exist");
    assert_eq!(node.id, "node_1");
    assert_eq!(node.label, "Concept 1");
    assert!((node.belief - 0.8).abs() < 1e-9);

    // Test belief upper clamping
    graph.add_node("node_2", "Concept 2", 1.5);
    let node_2 = graph.get_node("node_2").unwrap();
    assert!((node_2.belief - 1.0).abs() < 1e-9);

    // Test belief lower clamping
    graph.add_node("node_3", "Concept 3", -0.5);
    let node_3 = graph.get_node("node_3").unwrap();
    assert!((node_3.belief - 0.0).abs() < 1e-9);

    // Test update belief
    let success = graph.update_belief("node_1", 0.4);
    assert!(success);
    let updated_node = graph.get_node("node_1").unwrap();
    assert!((updated_node.belief - 0.4).abs() < 1e-9);

    // Test update non-existent node
    let fail = graph.update_belief("non_existent", 0.5);
    assert!(!fail);
}

#[test]
fn test_domain_edge_and_neighbors() {
    let mut graph = BeliefGraph::new();

    graph.add_node("A", "Alpha", 1.0);
    graph.add_node("B", "Beta", 0.9);
    graph.add_node("C", "Gamma", 0.8);

    graph.add_edge("A", "B", 0.75, "leads_to");
    graph.add_edge("A", "C", 0.5, "supports");

    let neighbors_a = graph.neighbors("A");
    assert_eq!(neighbors_a.len(), 2);
    assert!(neighbors_a.contains(&"B".to_string()));
    assert!(neighbors_a.contains(&"C".to_string()));

    let edge_1 = &graph.edges[0];
    assert_eq!(edge_1.from, "A");
    assert_eq!(edge_1.to, "B");
    assert!((edge_1.weight - 0.75).abs() < 1e-9);
    assert_eq!(edge_1.relation, "leads_to");
}

#[test]
fn test_domain_json_export_roundtrip() {
    let mut graph = BeliefGraph::new();
    graph.add_node("node_a", "Node A", 0.9);
    graph.add_node("node_b", "Node B", 0.7);
    graph.add_edge("node_a", "node_b", 0.85, "depends_on");

    let exported_val = graph.export_json();
    assert!(exported_val.is_object());

    let json_str = graph.save_to_string().expect("Serialization failed");
    let reconstructed = BeliefGraph::load_from_string(&json_str).expect("Deserialization failed");

    assert_eq!(reconstructed.nodes.len(), 2);
    assert_eq!(reconstructed.edges.len(), 1);

    let node_a = reconstructed.get_node("node_a").unwrap();
    assert_eq!(node_a.label, "Node A");
    assert!((node_a.belief - 0.9).abs() < 1e-9);

    let edge = &reconstructed.edges[0];
    assert_eq!(edge.from, "node_a");
    assert_eq!(edge.to, "node_b");
    assert!((edge.weight - 0.85).abs() < 1e-9);
    assert_eq!(edge.relation, "depends_on");
}

#[test]
fn test_interop_conversions() {
    let mut domain_graph = BeliefGraph::new();
    domain_graph.add_node("n1", "Concept One", 0.9);
    domain_graph.add_edge("n1", "n2", 0.8, "related_to");

    // Convert to context persisted graph
    let persisted: PersistedBeliefGraph = domain_graph.clone().into();
    assert_eq!(persisted.nodes.len(), 1);
    assert_eq!(persisted.edges.len(), 1);

    let context_node = persisted.nodes.get("n1").unwrap();
    assert_eq!(context_node.concept, "Concept One");
    assert!((context_node.confidence - 0.9).abs() < 1e-6);

    let context_edge = &persisted.edges[0];
    assert_eq!(context_edge.source, "n1");
    assert_eq!(context_edge.target, "n2");
    assert_eq!(context_edge.relation_type, "related_to");
    assert!((context_edge.weight - 0.8).abs() < 1e-6);

    // Convert back from context persisted graph
    let back_graph = BeliefGraph::from(persisted);
    assert_eq!(back_graph.nodes.len(), 1);
    assert_eq!(back_graph.edges.len(), 1);

    let back_node = back_graph.get_node("n1").unwrap();
    assert_eq!(back_node.label, "Concept One");
    assert!((back_node.belief - 0.9).abs() < 1e-6);

    let back_edge = &back_graph.edges[0];
    assert_eq!(back_edge.from, "n1");
    assert_eq!(back_edge.to, "n2");
    assert_eq!(back_edge.relation, "related_to");
    assert!((back_edge.weight - 0.8).abs() < 1e-6);
}
