use std::{
    env,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use eft_core::{InventoryLayout, InventorySave};
use http_body_util::BodyExt;
use tower::ServiceExt;

use super::{
    api::app,
    storage::{atomic_write, migrate_legacy_inventory, BackendState},
};
fn test_state() -> (BackendState, PathBuf) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let file = env::temp_dir().join(format!(
        "eft-inventory-test-{}-{}.json",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    (BackendState::new(file.clone()), file)
}

#[tokio::test]
async fn validation_endpoint_returns_weight_for_valid_layout() {
    let body = r#"{"width":2,"height":1,"items":[{"id":"item-a","x":0,"y":0,"width":1,"height":1,"weight_grams":300}]}"#;
    let (state, _) = test_state();
    let response = app(state)
        .oneshot(
            Request::post("/inventory/validate")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        serde_json::json!({"valid": true, "total_weight_grams": 300})
    );
}

#[tokio::test]
async fn validation_endpoint_returns_unprocessable_for_overlap() {
    let body = r#"{"width":2,"height":1,"items":[{"id":"a","x":0,"y":0,"width":1,"height":1,"weight_grams":10},{"id":"b","x":0,"y":0,"width":1,"height":1,"weight_grams":20}]}"#;
    let (state, _) = test_state();
    let response = app(state)
        .oneshot(
            Request::post("/inventory/validate")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        axum::http::StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test]
async fn inventory_endpoint_persists_and_loads_layout() {
    let (state, file) = test_state();
    let layout = InventoryLayout {
        width: 2,
        height: 1,
        items: vec![eft_core::ItemPlacement {
            id: "sample".to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            weight_grams: 300,
            durability: None,
        }],
    };
    let body = serde_json::to_vec(&layout).unwrap();

    let save_response = app(state.clone())
        .oneshot(
            Request::put("/inventory")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save_response.status(), StatusCode::OK);

    let load_response = app(state)
        .oneshot(Request::get("/inventory").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(load_response.status(), StatusCode::OK);
    let body = load_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(
        serde_json::from_slice::<InventoryLayout>(&body).unwrap(),
        layout
    );
    let persisted = tokio::fs::read(&file).await.unwrap();
    let save = InventorySave::decode(&persisted).unwrap();
    assert_eq!(save.format_version, eft_core::INVENTORY_SAVE_FORMAT_VERSION);
    assert_eq!(save.inventory, layout);

    tokio::fs::remove_file(file).await.unwrap();
}

#[tokio::test]
async fn invalid_inventory_does_not_overwrite_the_saved_profile() {
    let (state, file) = test_state();
    let saved_layout = InventoryLayout {
        width: 2,
        height: 1,
        items: vec![eft_core::ItemPlacement {
            id: "saved-item".to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            weight_grams: 100,
            durability: None,
        }],
    };
    let invalid_layout = InventoryLayout {
        items: vec![
            eft_core::ItemPlacement {
                id: "first".to_owned(),
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                weight_grams: 100,
                durability: None,
            },
            eft_core::ItemPlacement {
                id: "overlap".to_owned(),
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                weight_grams: 100,
                durability: None,
            },
        ],
        ..saved_layout.clone()
    };
    tokio::fs::write(
        &file,
        InventorySave::new(saved_layout.clone()).encode().unwrap(),
    )
    .await
    .unwrap();

    let response = app(state)
        .oneshot(
            Request::put("/inventory")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&invalid_layout).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let stored = InventorySave::decode(&tokio::fs::read(&file).await.unwrap()).unwrap();
    assert_eq!(stored.inventory, saved_layout);
    tokio::fs::remove_file(file).await.unwrap();
}

#[tokio::test]
async fn legacy_inventory_is_copied_without_removing_the_original() {
    let (_, file) = test_state();
    let root = file.with_extension("migration-test");
    let legacy = root.join("data").join("inventory.json");
    let target = root.join("profile").join("inventory.json");
    let layout = InventoryLayout {
        width: 2,
        height: 1,
        items: vec![eft_core::ItemPlacement {
            id: "legacy-item".to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            weight_grams: 100,
            durability: None,
        }],
    };
    tokio::fs::create_dir_all(legacy.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(
        &legacy,
        InventorySave::new(layout.clone()).encode().unwrap(),
    )
    .await
    .unwrap();

    migrate_legacy_inventory(&target, &legacy).await.unwrap();

    let migrated = tokio::fs::read(&target).await.unwrap();
    assert_eq!(InventorySave::decode(&migrated).unwrap().inventory, layout);
    assert!(tokio::fs::try_exists(&legacy).await.unwrap());
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn existing_user_profile_is_not_overwritten_by_legacy_inventory() {
    let (_, file) = test_state();
    let root = file.with_extension("profile-precedence-test");
    let legacy = root.join("data").join("inventory.json");
    let target = root.join("profile").join("inventory.json");
    let user_layout = InventoryLayout {
        width: 2,
        height: 1,
        items: vec![eft_core::ItemPlacement {
            id: "user-item".to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            weight_grams: 200,
            durability: None,
        }],
    };
    let legacy_layout = InventoryLayout {
        items: vec![],
        ..user_layout.clone()
    };
    tokio::fs::create_dir_all(target.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::create_dir_all(legacy.parent().unwrap())
        .await
        .unwrap();
    tokio::fs::write(
        &target,
        InventorySave::new(user_layout.clone()).encode().unwrap(),
    )
    .await
    .unwrap();
    tokio::fs::write(&legacy, InventorySave::new(legacy_layout).encode().unwrap())
        .await
        .unwrap();

    migrate_legacy_inventory(&target, &legacy).await.unwrap();

    let profile = InventorySave::decode(&tokio::fs::read(&target).await.unwrap()).unwrap();
    assert_eq!(profile.inventory, user_layout);
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn atomic_write_replaces_an_existing_profile() {
    let (_, file) = test_state();
    atomic_write(&file, b"old profile").await.unwrap();
    atomic_write(&file, b"new profile").await.unwrap();

    assert_eq!(tokio::fs::read(&file).await.unwrap(), b"new profile");

    tokio::fs::remove_file(file).await.unwrap();
}

#[tokio::test]
async fn inventory_endpoint_loads_legacy_unversioned_saves() {
    let (state, file) = test_state();
    let layout = InventoryLayout {
        width: 2,
        height: 1,
        items: vec![eft_core::ItemPlacement {
            id: "legacy-item".to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            weight_grams: 100,
            durability: None,
        }],
    };
    tokio::fs::write(&file, serde_json::to_vec(&layout).unwrap())
        .await
        .unwrap();

    let response = app(state)
        .oneshot(Request::get("/inventory").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<InventoryLayout>(&body).unwrap(),
        layout
    );

    tokio::fs::remove_file(file).await.unwrap();
}

#[tokio::test]
async fn inventory_endpoint_reports_missing_data() {
    let (state, _) = test_state();
    let response = app(state)
        .oneshot(Request::get("/inventory").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
