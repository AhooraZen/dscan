use dscan_core::work_stealing::{Steal, deque};
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
    let num_thieves = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(2, 8);
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
                    Steal::Retry => std::hint::spin_loop(),
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

#[test]
fn test_chase_lev_steal_batch() {
    let (worker0, stealer0) = deque::<usize>();
    let (worker1, _stealer1) = deque::<usize>();

    for i in 0..64 {
        worker0.push(i);
    }

    let stolen_count = stealer0.steal_batch(&worker1, 32);
    assert_eq!(stolen_count, 32);

    for i in 0..32 {
        assert_eq!(worker1.pop(), Some(i));
    }
    assert_eq!(worker1.pop(), None);

    let mut remaining = Vec::new();
    while let Some(item) = worker0.pop() {
        remaining.push(item);
    }
    assert_eq!(remaining.len(), 32);
    for i in 32..64 {
        assert!(remaining.contains(&i));
    }
}

#[test]
fn test_chase_lev_bulk_steal_half() {
    let (worker0, stealer0) = deque::<usize>();
    let (worker1, _stealer1) = deque::<usize>();

    for i in 0..100 {
        worker0.push(i);
    }

    let stolen = stealer0.steal_batch(&worker1, 32);
    assert_eq!(stolen, 32);

    let mut collected = Vec::new();
    while let Some(val) = worker1.pop() {
        collected.push(val);
    }
    while let Some(val) = worker0.pop() {
        collected.push(val);
    }

    assert_eq!(collected.len(), 100);
    let set: HashSet<usize> = collected.into_iter().collect();
    assert_eq!(set.len(), 100);
}

#[test]
fn test_chase_lev_steal_batch_concurrent() {
    let (worker, stealer) = deque::<usize>();
    let total_items = 20_000;
    let num_thieves = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(2, 8);
    let done = Arc::new(AtomicBool::new(false));

    let mut thief_handles = Vec::new();
    for _ in 0..num_thieves {
        let s = stealer.clone();
        let d = Arc::clone(&done);
        thief_handles.push(thread::spawn(move || {
            let (thief_worker, _thief_stealer) = deque::<usize>();
            let mut stolen = Vec::new();
            while !d.load(Ordering::Acquire) || !s.is_empty() {
                if s.steal_batch(&thief_worker, 16) > 0 {
                    while let Some(val) = thief_worker.pop() {
                        stolen.push(val);
                    }
                } else {
                    thread::yield_now();
                }
            }
            while let Some(val) = thief_worker.pop() {
                stolen.push(val);
            }
            stolen
        }));
    }

    let mut worker_items = Vec::new();
    for i in 0..total_items {
        worker.push(i);
        if i % 8 == 0
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
