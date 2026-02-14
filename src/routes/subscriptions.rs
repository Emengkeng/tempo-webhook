use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    CreateSubscriptionRequest, Filter, FilterInput, Subscription, SubscriptionResponse,
};
use crate::state::AppState;
use crate::utils::auth::AuthenticatedUser;
use crate::utils::validation::*;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    data: Vec<T>,
    total: usize,
    limit: i64,
    offset: i64,
    has_more: bool,
}

pub async fn create_subscription(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> AppResult<(StatusCode, Json<SubscriptionResponse>)> {
    // Validate inputs
    validate_network(&payload.network)?;
    validate_event_type(&payload.event_type)?;
    validate_ethereum_address(&payload.address)?;
    validate_url(&payload.webhook_url)?;

    // Check subscription quota BEFORE creating
    crate::services::quota::check_subscription_quota(
        &state.db,
        auth.api_key.organization_id,
        &payload.network,
    )
    .await?;

    // Validate filters and check filter quota
    if let Some(ref filters) = payload.filters {
        // Check filter count against plan limits
        crate::services::quota::check_filter_quota(
            filters.len(),
            &state.db,
            auth.api_key.organization_id,
        )
        .await?;

        for filter in filters {
            validate_filter_type(&filter.filter_type)?;
            
            // Additional validation based on filter type
            match filter.filter_type.as_str() {
                "from_address" | "to_address" => {
                    validate_ethereum_address(&filter.value)?;
                }
                "amount_min" | "amount_max" => {
                    filter.value.parse::<f64>()
                        .map_err(|_| AppError::BadRequest("Invalid amount value".to_string()))?;
                }
                _ => {}
            }
        }
    }

    let confirmation_blocks = payload.confirmation_blocks.unwrap_or(1);

    // Create subscription
    let subscription = Subscription::create(
        &state.db,
        auth.api_key.organization_id,
        auth.user.id,
        payload.network,
        payload.event_type,
        payload.address,
        payload.webhook_url,
        confirmation_blocks,
    )
    .await?;

    // Create filters if provided
    let filters = if let Some(filter_inputs) = payload.filters {
        Filter::create_batch(&state.db, subscription.id, filter_inputs).await?
    } else {
        Vec::new()
    };

    let response = SubscriptionResponse {
        id: subscription.id,
        event_type: subscription.event_type,
        network: subscription.network,
        address: subscription.address,
        webhook_url: subscription.webhook_url,
        active: subscription.active,
        confirmation_blocks: subscription.confirmation_blocks,
        created_at: subscription.created_at,
        filters,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn list_subscriptions(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<ListResponse<SubscriptionResponse>>> {
    let limit = query.limit.min(100);
    let offset = query.offset;

    let subscriptions = Subscription::list_by_organization(
        &state.db,
        auth.api_key.organization_id,
        limit,
        offset,
    )
    .await?;

    let total = subscriptions.len();
    let has_more = total as i64 == limit;

    let mut responses = Vec::new();
    for sub in subscriptions {
        let filters = Filter::get_by_subscription(&state.db, sub.id).await?;
        responses.push(SubscriptionResponse {
            id: sub.id,
            event_type: sub.event_type,
            network: sub.network,
            address: sub.address,
            webhook_url: sub.webhook_url,
            active: sub.active,
            confirmation_blocks: sub.confirmation_blocks,
            created_at: sub.created_at,
            filters,
        });
    }

    Ok(Json(ListResponse {
        data: responses,
        total,
        limit,
        offset,
        has_more,
    }))
}

pub async fn get_subscription(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SubscriptionResponse>> {
    let subscription = Subscription::get_by_id(&state.db, id, auth.api_key.organization_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Subscription not found".to_string()))?;

    let filters = Filter::get_by_subscription(&state.db, subscription.id).await?;

    Ok(Json(SubscriptionResponse {
        id: subscription.id,
        event_type: subscription.event_type,
        network: subscription.network,
        address: subscription.address,
        webhook_url: subscription.webhook_url,
        active: subscription.active,
        confirmation_blocks: subscription.confirmation_blocks,
        created_at: subscription.created_at,
        filters,
    }))
}

pub async fn update_subscription(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSubscriptionRequest>,
) -> AppResult<Json<SubscriptionResponse>> {
    // Verify subscription exists and belongs to organization
    let subscription = Subscription::get_by_id(&state.db, id, auth.api_key.organization_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Subscription not found".to_string()))?;

    // Update active status if provided
    if let Some(active) = payload.active {
        Subscription::update_status(&state.db, id, auth.api_key.organization_id, active).await?;
    }

    // Fetch updated subscription
    let updated = Subscription::get_by_id(&state.db, id, auth.api_key.organization_id)
        .await?
        .unwrap();

    let filters = Filter::get_by_subscription(&state.db, updated.id).await?;

    Ok(Json(SubscriptionResponse {
        id: updated.id,
        event_type: updated.event_type,
        network: updated.network,
        address: updated.address,
        webhook_url: updated.webhook_url,
        active: updated.active,
        confirmation_blocks: updated.confirmation_blocks,
        created_at: updated.created_at,
        filters,
    }))
}

pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Extension(auth): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    // Verify subscription exists
    Subscription::get_by_id(&state.db, id, auth.api_key.organization_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Subscription not found".to_string()))?;

    Subscription::delete(&state.db, id, auth.api_key.organization_id).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubscriptionRequest {
    active: Option<bool>,
}
