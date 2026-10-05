use dscan::work_stealing::{Steal, deque};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

#[test]
fn test_chase_lev_lifo_local() {
    let (worker, _stealer) = deque::<usize>();
    for i in 0..100 {
        worker.push(i);
    }
    for i in (0..100).rev() {
        assert_eq!(worker.pop(), Some(i));
    }
    assert_eq!(worker.pop(), None);
}

#[test]
fn test_chase_lev_fifo_remote() {
    let (worker, stealer) = deque::<usize>();
    for i in 0..100 {
        worker.push(i);
    }
    for i in 0..100 {
        assert_eq!(stealer.steal(), Steal::Success(i));
    }
    assert_eq!(stealer.steal(), Steal::Empty);
    assert_eq!(worker.pop(), None);
}

#[test]
fn test_chase_lev_growth() {
    let (worker, stealer) = deque::<usize>();
    // Initial capacity is 1024, push 8192 to force multiple growths
    for i in 0..8192 {
        worker.push(i);
    }
    // Steal half, pop half
    for i in 0..4096 {
        assert_eq!(stealer.steal(), Steal::Success(i));
    }
    for i in (4096..8192).rev() {
        assert_eq!(worker.pop(), Some(i));
    }
    assert_eq!(worker.pop(), None);
    assert_eq!(stealer.steal(), Steal::Empty);
}

#[test]
fn test_chase_lev_multi_thief_concurrent() {
    let (worker, stealer) = deque::<usize>();
    let total_items = 20_000;
    let num_thieves = 8;
    let done = Arc::new(AtomicBool::new(false));

    let mut thief_handles = Vec::new();
    for _ in 0..num_thieves {
        let s = stealer.clone();
        let d = Arc::clone(&done);
        thief_handles.push(thread::spawn(move || {
            let mut stolen = Vec::new();
            while !d.load(Ordering::Acquire) || !s.is_empty() {
                match s.steal() {
                    Steal::Success(val) => stolen.push(val),
                    Steal::Empty => thread::yield_now(),
                    Steal::Retry => {}
                }
            }
            stolen
        }));
    }

    let mut worker_items = Vec::new();
    for i in 0..total_items {
        worker.push(i);
        if i % 4 == 0
            && let Some(val) = worker.pop()
        {
            worker_items.push(val);
        }
    }

    while let Some(val) = worker.pop() {
        worker_items.push(val);
    }

    done.store(true, Ordering::Release);

    let mut all_collected = worker_items;
    for h in thief_handles {
        all_collected.extend(h.join().unwrap());
    }

    assert_eq!(all_collected.len(), total_items);
    let set: HashSet<usize> = all_collected.into_iter().collect();
    assert_eq!(set.len(), total_items);
    for i in 0..total_items {
        assert!(set.contains(&i), "missing item {i}");
    }
}
