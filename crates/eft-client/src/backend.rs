use super::*;

pub(super) fn start_backend_sync() -> BackendSync {
    let base_url =
        env::var("EFT_BACKEND_URL").unwrap_or_else(|_| "http://127.0.0.1:3000".to_owned());
    let inventory_url = format!("{}/inventory", base_url.trim_end_matches('/'));
    let (request_sender, request_receiver) = mpsc::channel();
    let (message_sender, message_receiver) = mpsc::channel();
    let (status, loading, retry_available) = match request_sender.send(BackendRequest::Load) {
        Ok(()) => (
            format!("Connecting to backend at {base_url}..."),
            true,
            false,
        ),
        Err(error) => (
            format!("Could not queue profile load: {error}"),
            false,
            true,
        ),
    };

    thread::Builder::new()
        .name("eft-inventory-sync".to_owned())
        .spawn(move || {
            let client = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(3))
                .build();

            while let Ok(request) = request_receiver.recv() {
                match request {
                    BackendRequest::Load => {
                        if message_sender
                            .send(BackendMessage::Loaded(fetch_inventory(
                                &client,
                                &inventory_url,
                            )))
                            .is_err()
                        {
                            return;
                        }
                    }
                    BackendRequest::Save(layout) => {
                        let result = save_inventory(&client, &inventory_url, &layout);
                        if message_sender.send(BackendMessage::Saved(result)).is_err() {
                            return;
                        }
                    }
                }
            }
        })
        .expect("failed to start inventory backend worker");

    BackendSync {
        requests: request_sender,
        messages: Mutex::new(message_receiver),
        status,
        ready: false,
        loading,
        save_enabled: false,
        retry_available,
        last_queued: None,
    }
}

pub(super) fn fetch_inventory(
    client: &ureq::Agent,
    url: &str,
) -> Result<Option<InventoryLayout>, String> {
    let response = match client.get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(error) => return Err(format!("Could not load inventory: {error}")),
    };

    response
        .into_json()
        .map(Some)
        .map_err(|error| format!("Backend returned invalid inventory JSON: {error}"))
}

pub(super) fn save_inventory(
    client: &ureq::Agent,
    url: &str,
    layout: &InventoryLayout,
) -> Result<(), String> {
    client
        .put(url)
        .send_json(layout)
        .map_err(|error| format!("Could not save inventory: {error}"))?;
    Ok(())
}

pub(super) fn queue_inventory_save(sync: &mut BackendSync, layout: &InventoryLayout) {
    match sync.requests.send(BackendRequest::Save(layout.clone())) {
        Ok(()) => {
            sync.last_queued = Some(layout.clone());
            sync.save_enabled = false;
            sync.retry_available = false;
            sync.status = "Saving inventory to backend...".to_owned();
        }
        Err(error) => {
            sync.save_enabled = false;
            sync.retry_available = true;
            sync.status = format!("Could not queue inventory save: {error}");
        }
    }
}

pub(super) fn poll_backend_sync(
    mut sync: ResMut<BackendSync>,
    mut inventory: ResMut<InventoryState>,
) {
    let messages = match sync.messages.lock() {
        Ok(receiver) => Some(receiver.try_iter().collect::<Vec<_>>()),
        Err(_) => None,
    };
    let Some(messages) = messages else {
        sync.status = "Backend response channel is unavailable.".to_owned();
        return;
    };

    for message in messages {
        match message {
            BackendMessage::Loaded(Err(error)) => {
                sync.loading = false;
                sync.ready = false;
                sync.save_enabled = false;
                sync.retry_available = true;
                sync.status = error;
            }
            BackendMessage::Loaded(Ok(None)) => {
                sync.loading = false;
                sync.ready = true;
                sync.retry_available = false;
                sync.status = "No saved inventory; creating a new backend profile.".to_owned();
                let layout = inventory.layout.clone();
                queue_inventory_save(&mut sync, &layout);
            }
            BackendMessage::Loaded(Ok(Some(layout))) => {
                sync.loading = false;
                let dimensions_match =
                    layout.width == INVENTORY_GRID_WIDTH && layout.height == INVENTORY_GRID_HEIGHT;
                if !dimensions_match {
                    sync.ready = false;
                    sync.save_enabled = false;
                    sync.retry_available = true;
                    sync.status = format!(
                        "Saved inventory has unsupported dimensions {}x{}.",
                        layout.width, layout.height
                    );
                    continue;
                }
                if let Err(error) = validate_inventory(&layout) {
                    sync.ready = false;
                    sync.save_enabled = false;
                    sync.retry_available = true;
                    sync.status = format!("Saved inventory failed validation: {error}");
                    continue;
                }
                if inventory.layout != default_inventory_layout() {
                    sync.ready = true;
                    sync.save_enabled = false;
                    sync.retry_available = false;
                    sync.status = "Kept local edits made during load; server profile was not overwritten. Restart to load it.".to_owned();
                    continue;
                }

                inventory.layout = layout.clone();
                sync.last_queued = Some(layout);
                sync.ready = true;
                sync.save_enabled = true;
                sync.retry_available = false;
                sync.status = "Inventory loaded from backend.".to_owned();
            }
            BackendMessage::Saved(Ok(())) => {
                sync.save_enabled = true;
                sync.retry_available = false;
                sync.status = "Inventory saved on backend.".to_owned();
            }
            BackendMessage::Saved(Err(error)) => {
                sync.save_enabled = false;
                sync.retry_available = true;
                sync.status = error;
            }
        }
    }
}

pub(super) fn sync_inventory_changes(
    inventory: Res<InventoryState>,
    raid: Res<RaidState>,
    mut sync: ResMut<BackendSync>,
) {
    if matches!(raid.phase, RaidPhase::Active | RaidPhase::Paused)
        || !sync.ready
        || !sync.save_enabled
    {
        return;
    }
    if sync.last_queued.as_ref() != Some(&inventory.layout) {
        queue_inventory_save(&mut sync, &inventory.layout);
    }
}
