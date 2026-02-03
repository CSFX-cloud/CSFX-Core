use crate::{auth::middleware::AuthenticatedUser, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use entity::{subscription, Subscription};
use sea_orm::prelude::Decimal;
use sea_orm::{ActiveModelTrait, ColumnTrait, DeleteResult, EntityTrait, QueryFilter, Set};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(serde::Deserialize, ToSchema)]
pub struct CreateSubscriptionRequest {
    pub name: String,
    pub amount: f64,
    pub billing_cycle: String,
    pub next_billing_date: String,
    pub category: String,
}

#[derive(serde::Deserialize, ToSchema)]
pub struct UpdateSubscriptionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_cycle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_billing_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_active: Option<bool>,
}

/// Get all subscriptions for the authenticated user
#[utoipa::path(
    get,
    path = "/api/subscriptions",
    responses(
        (status = 200, description = "List of user's subscriptions", body = [subscription::Model]),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Subscriptions",
    security(
        ("jwt_token" = [])
    )
)]
pub async fn get_subscriptions(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<subscription::Model>>, StatusCode> {
    let subscriptions = Subscription::find()
        .filter(subscription::Column::UserId.eq(user.0.user_id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!("Database error getting subscriptions: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!("Found {} subscriptions for user", subscriptions.len());

    match serde_json::to_string(&subscriptions) {
        Ok(json) => tracing::info!("Serialized subscriptions: {}", json),
        Err(e) => {
            tracing::error!("Failed to serialize subscriptions: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    Ok(Json(subscriptions))
}

/// Get subscription by ID for the authenticated user
#[utoipa::path(
    get,
    path = "/api/subscriptions/{id}",
    params(
        ("id" = Uuid, Path, description = "Subscription ID")
    ),
    responses(
        (status = 200, description = "Subscription found", body = subscription::Model),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Subscription not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Subscriptions",
    security(
        ("jwt_token" = [])
    )
)]
pub async fn get_subscription(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<Json<subscription::Model>, StatusCode> {
    let subscription = Subscription::find_by_id(id)
        .filter(subscription::Column::UserId.eq(user.0.user_id))
        .one(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(subscription))
}

/// Create a new subscription for the authenticated user
#[utoipa::path(
    post,
    path = "/api/subscriptions",
    request_body = CreateSubscriptionRequest,
    responses(
        (status = 200, description = "Subscription created successfully", body = subscription::Model),
        (status = 400, description = "Bad request - invalid data"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Subscriptions",
    security(
        ("jwt_token" = [])
    )
)]
pub async fn create_subscription(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> Result<Json<subscription::Model>, StatusCode> {
    let subscription_id = Uuid::new_v4();

    let new_subscription = subscription::ActiveModel {
        id: Set(subscription_id),
        user_id: Set(user.0.user_id),
        name: Set(payload.name),
        amount: Set(Decimal::try_from(payload.amount).map_err(|_| StatusCode::BAD_REQUEST)?),
        billing_cycle: Set(payload.billing_cycle),
        next_billing_date: Set(chrono::NaiveDateTime::parse_from_str(
            &payload.next_billing_date,
            "%Y-%m-%d %H:%M:%S",
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?),
        category: Set(payload.category),
        is_active: Set(true),
    };

    Subscription::insert(new_subscription)
        .exec_without_returning(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let created = Subscription::find_by_id(subscription_id)
        .one(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(created))
}

/// Update a subscription
#[utoipa::path(
    put,
    path = "/api/subscriptions/{id}",
    params(
        ("id" = Uuid, Path, description = "Subscription ID")
    ),
    request_body = UpdateSubscriptionRequest,
    responses(
        (status = 200, description = "Subscription updated successfully", body = subscription::Model),
        (status = 400, description = "Bad request - invalid data"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Subscription not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Subscriptions",
    security(
        ("jwt_token" = [])
    )
)]
pub async fn update_subscription(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSubscriptionRequest>,
) -> Result<Json<subscription::Model>, StatusCode> {
    let mut subscription: subscription::ActiveModel = Subscription::find_by_id(id)
        .filter(subscription::Column::UserId.eq(user.0.user_id))
        .one(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?
        .into();

    if let Some(name) = payload.name {
        subscription.name = Set(name);
    }
    if let Some(amount) = payload.amount {
        if amount <= 0.0 {
            return Err(StatusCode::BAD_REQUEST);
        }
        subscription.amount = Set(Decimal::try_from(amount).map_err(|_| StatusCode::BAD_REQUEST)?);
    }
    if let Some(billing_cycle) = payload.billing_cycle {
        subscription.billing_cycle = Set(billing_cycle);
    }
    if let Some(next_billing_date) = payload.next_billing_date {
        subscription.next_billing_date = Set(chrono::NaiveDateTime::parse_from_str(
            &next_billing_date,
            "%Y-%m-%d %H:%M:%S",
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?);
    }
    if let Some(category) = payload.category {
        subscription.category = Set(category);
    }
    if let Some(is_active) = payload.is_active {
        subscription.is_active = Set(is_active);
    }

    let result = subscription
        .update(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result))
}

/// Delete a subscription
#[utoipa::path(
    delete,
    path = "/api/subscriptions/{id}",
    params(
        ("id" = Uuid, Path, description = "Subscription ID")
    ),
    responses(
        (status = 204, description = "Subscription deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Subscription not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Subscriptions",
    security(
        ("jwt_token" = [])
    )
)]
pub async fn delete_subscription(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let subscription_exists = Subscription::find_by_id(id)
        .filter(subscription::Column::UserId.eq(user.0.user_id))
        .one(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if subscription_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    let result: DeleteResult = Subscription::delete_by_id(id)
        .exec(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
}

pub fn subscriptions_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/subscriptions",
            get(get_subscriptions).post(create_subscription),
        )
        .route(
            "/subscriptions/:id",
            get(get_subscription)
                .put(update_subscription)
                .delete(delete_subscription),
        )
}
