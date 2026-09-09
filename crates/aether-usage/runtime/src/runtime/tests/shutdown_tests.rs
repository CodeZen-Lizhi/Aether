use super::*;

fn completed_event(request_id: &str) -> UsageEvent {
    UsageEvent::new(
        UsageEventType::Completed,
        request_id,
        UsageEventData {
            provider_name: "openai".to_string(),
            model: "test-model".to_string(),
            status_code: Some(200),
            ..UsageEventData::default()
        },
    )
}

#[tokio::test]
async fn idle_wait_tracks_detached_producer_before_event_submission() {
    let runtime = UsageRuntime::new(UsageRuntimeConfig {
        enabled: true,
        ..UsageRuntimeConfig::default()
    })
    .unwrap();
    let store = CloneQueueConfiguredUsageStore {
        records: Arc::new(Mutex::new(Vec::new())),
        queue: Arc::new(RuntimeState::memory(MemoryRuntimeStateConfig::default())),
    };
    let release = Arc::new(tokio::sync::Notify::new());
    let producer_release = Arc::clone(&release);
    let producer_runtime = runtime.clone();
    let producer_store = store.clone();
    let handoff = runtime.track_persistence_handoff();
    let producer = tokio::spawn(async move {
        let _handoff = handoff;
        producer_release.notified().await;
        producer_runtime
            .record_terminal_event_direct(&producer_store, completed_event("shutdown-handoff"))
            .await;
    });
    assert_eq!(runtime.metrics_snapshot().terminal_submission_pending, 0);
    assert!(!runtime
        .wait_for_idle(&store, Duration::from_millis(60))
        .await
        .unwrap());
    release.notify_one();
    assert!(runtime
        .wait_for_idle(&store, Duration::from_secs(1))
        .await
        .unwrap());
    producer.await.unwrap();
    assert_eq!(
        store.records.lock().unwrap()[0].request_id,
        "shutdown-handoff"
    );
}

#[tokio::test]
async fn cancelled_handoff_does_not_keep_shutdown_waiting() {
    let runtime = UsageRuntime::new(UsageRuntimeConfig {
        enabled: true,
        ..UsageRuntimeConfig::default()
    })
    .unwrap();
    let handoff = runtime.track_persistence_handoff();
    let producer = tokio::spawn(async move {
        let _handoff = handoff;
        std::future::pending::<()>().await;
    });
    producer.abort();
    assert!(producer.await.unwrap_err().is_cancelled());
    assert!(runtime
        .wait_for_idle(&NoRedisUsageStore::default(), Duration::from_secs(1))
        .await
        .unwrap());
}

#[tokio::test]
async fn idle_wait_tracks_direct_database_write_until_it_finishes() {
    let runtime = UsageRuntime::new(UsageRuntimeConfig {
        enabled: true,
        ..UsageRuntimeConfig::default()
    })
    .unwrap();
    let store = BlockingWriteQueueConfiguredUsageStore {
        records: Arc::new(Mutex::new(Vec::new())),
        queue: Arc::new(RuntimeState::memory(MemoryRuntimeStateConfig::default())),
        write_started: Arc::new(tokio::sync::Notify::new()),
        release_writes: Arc::new(tokio::sync::Notify::new()),
        writes_completed: Arc::new(AtomicUsize::new(0)),
    };
    let started = store.write_started.notified();
    let write_runtime = runtime.clone();
    let write_store = store.clone();
    let write = tokio::spawn(async move {
        write_runtime
            .record_terminal_event_direct(&write_store, completed_event("shutdown-direct"))
            .await;
    });
    timeout(Duration::from_secs(1), started).await.unwrap();
    assert!(!runtime
        .wait_for_idle(&store, Duration::from_millis(60))
        .await
        .unwrap());
    assert_eq!(store.writes_completed.load(Ordering::Acquire), 0);

    store.release_writes.notify_one();
    timeout(Duration::from_secs(1), write)
        .await
        .unwrap()
        .unwrap();
    assert!(runtime
        .wait_for_idle(&store, Duration::from_secs(1))
        .await
        .unwrap());
    assert_eq!(store.writes_completed.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn idle_wait_requires_queued_usage_to_be_persisted_and_acked() {
    let config = UsageRuntimeConfig {
        enabled: true,
        queue_terminal_events: true,
        worker_count: 1,
        worker_autoscale_enabled: false,
        consumer_block_ms: 1,
        stream_key: "usage:events:test:shutdown-queue".to_string(),
        consumer_group: "usage_consumers_test_shutdown_queue".to_string(),
        ..UsageRuntimeConfig::default()
    };
    let queue_runner: Arc<dyn RuntimeQueueStore> =
        Arc::new(RuntimeState::memory(MemoryRuntimeStateConfig::default()));
    UsageQueue::new(Arc::clone(&queue_runner), config.clone())
        .unwrap()
        .ensure_consumer_group()
        .await
        .unwrap();
    let store = Arc::new(CloneQueueConfiguredUsageStore {
        records: Arc::new(Mutex::new(Vec::new())),
        queue: queue_runner,
    });
    let runtime = UsageRuntime::new(config).unwrap();
    runtime
        .record_terminal_event(store.as_ref(), completed_event("shutdown-queued"))
        .await;
    assert!(!runtime
        .wait_for_idle(store.as_ref(), Duration::from_millis(60))
        .await
        .unwrap());
    assert!(store.records.lock().unwrap().is_empty());

    let worker = runtime.spawn_worker_supervisor(Arc::clone(&store)).unwrap();
    let drained = runtime
        .wait_for_idle(store.as_ref(), Duration::from_secs(2))
        .await
        .unwrap();
    worker.abort();
    let _ = worker.await;
    assert!(drained);
    let records = store.records.lock().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].request_id, "shutdown-queued");
    assert_eq!(records[0].status, "completed");
}

#[tokio::test]
async fn idle_wait_includes_delayed_lifecycle_items_outside_the_queue() {
    let config = UsageRuntimeConfig {
        enabled: true,
        queue_lifecycle_events: true,
        lifecycle_enqueue_delay_ms: 150,
        worker_count: 1,
        worker_autoscale_enabled: false,
        consumer_block_ms: 1,
        stream_key: "usage:events:test:shutdown-delayed".to_string(),
        consumer_group: "usage_consumers_test_shutdown_delayed".to_string(),
        ..UsageRuntimeConfig::default()
    };
    let queue_runner: Arc<dyn RuntimeQueueStore> =
        Arc::new(RuntimeState::memory(MemoryRuntimeStateConfig::default()));
    UsageQueue::new(Arc::clone(&queue_runner), config.clone())
        .unwrap()
        .ensure_consumer_group()
        .await
        .unwrap();
    let store = Arc::new(CloneQueueConfiguredUsageStore {
        records: Arc::new(Mutex::new(Vec::new())),
        queue: queue_runner,
    });
    let runtime = UsageRuntime::new(config).unwrap();
    runtime
        .enqueue_or_write_lifecycle(
            store.as_ref(),
            UsageEvent::new(
                UsageEventType::Pending,
                "shutdown-delayed",
                UsageEventData {
                    provider_name: "openai".to_string(),
                    model: "test-model".to_string(),
                    ..UsageEventData::default()
                },
            ),
        )
        .await;
    let queue = runtime.queue_health_snapshot(store.as_ref()).await.unwrap();
    assert_eq!(queue.stream_length, 0);
    assert!(!runtime
        .wait_for_idle(store.as_ref(), Duration::from_millis(60))
        .await
        .unwrap());

    let worker = runtime.spawn_worker_supervisor(Arc::clone(&store)).unwrap();
    let drained = runtime
        .wait_for_idle(store.as_ref(), Duration::from_secs(2))
        .await
        .unwrap();
    worker.abort();
    let _ = worker.await;
    assert!(drained);
    assert_eq!(
        store.records.lock().unwrap()[0].request_id,
        "shutdown-delayed"
    );
}
