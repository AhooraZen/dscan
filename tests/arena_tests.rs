use dscan::arena::DirArena;

#[test]
fn test_deep_nested_tree_rollup_depth_10() {
    let mut arena = DirArena::new();
    let root = arena.add_root(0, b"/root");
    arena.add_direct_bytes(root, 100);

    let mut current = root;

    // Linear chain of 10 levels
    let mut chain = vec![root];
    for d in 1..=10 {
        let name = format!("level_{d}");
        let child = arena.add_node(current, d as u16, name.as_bytes());
        let bytes = (d as u64) * 50;
        arena.add_direct_bytes(child, bytes);
        chain.push(child);
        current = child;
    }

    // Add branch at depth 5
    let branch_parent = chain[5];
    let branch_child = arena.add_node(branch_parent, 6, b"branch_child");
    arena.add_direct_bytes(branch_child, 1234);

    arena.rollup();

    // Verify leaf at depth 10
    assert_eq!(arena.nodes[chain[10] as usize].total_bytes, 500);

    // Verify branch child
    assert_eq!(arena.nodes[branch_child as usize].total_bytes, 1234);

    // Verify level 5 includes chain[6..10] + branch_child
    let subchain_sum: u64 = (6..=10).map(|d| (d as u64) * 50).sum();
    let expected_lvl5 = (5 * 50) + subchain_sum + 1234;
    assert_eq!(arena.nodes[chain[5] as usize].total_bytes, expected_lvl5);

    // Verify root total
    let total_chain: u64 = (1..=10).map(|d| (d as u64) * 50).sum();
    let expected_root = 100 + total_chain + 1234;
    assert_eq!(arena.nodes[root as usize].total_bytes, expected_root);

    // Verify path reconstruction for deepest node
    let mut path = Vec::new();
    arena.reconstruct_path(chain[10], &mut path);
    assert_eq!(
        path,
        b"/root/level_1/level_2/level_3/level_4/level_5/level_6/level_7/level_8/level_9/level_10"
    );

    // Verify path reconstruction for branch child
    arena.reconstruct_path(branch_child, &mut path);
    assert_eq!(
        path,
        b"/root/level_1/level_2/level_3/level_4/level_5/branch_child"
    );
}

#[test]
fn test_multi_branch_random_rollup_equivalence() {
    let mut arena = DirArena::new();
    let root = arena.add_root(0, b"base");
    arena.add_direct_bytes(root, 10);

    let mut direct_sizes = Vec::new();
    direct_sizes.push(10u64);

    let mut parent_map = Vec::new();
    parent_map.push(0usize);

    // Create a 5-ary tree with 3 levels (1 + 5 + 25 = 31 nodes)
    for i in 1..=5 {
        let child = arena.add_node(root, 1, format!("c_{i}").as_bytes());
        arena.add_direct_bytes(child, i as u64 * 10);
        direct_sizes.push(i as u64 * 10);
        parent_map.push(root as usize);

        for j in 1..=5 {
            let grandchild = arena.add_node(child, 2, format!("gc_{j}").as_bytes());
            let sz = (i * 100 + j * 10) as u64;
            arena.add_direct_bytes(grandchild, sz);
            direct_sizes.push(sz);
            parent_map.push(child as usize);
        }
    }

    arena.rollup();

    // Verify against independent recursive/reverse pass
    let mut expected = direct_sizes.clone();
    for i in (1..expected.len()).rev() {
        let p = parent_map[i];
        expected[p] += expected[i];
    }

    for (i, node) in arena.nodes.iter().enumerate() {
        assert_eq!(node.total_bytes, expected[i], "Mismatch at node {i}");
    }
}
