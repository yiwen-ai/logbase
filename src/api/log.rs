use axum::{
    extract::{Query, State},
    Extension,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use validator::Validate;

use axum_web::context::ReqContext;
use axum_web::erring::{HTTPError, SuccessResponse};
use axum_web::object::PackObject;
use scylla_orm::ColumnsMap;

use crate::db;

use crate::api::{action, get_fields, AppState};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct LogOutput {
    pub uid: PackObject<xid::Id>,
    pub id: PackObject<xid::Id>,
    pub action: String,
    pub status: i8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gid: Option<PackObject<xid::Id>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<PackObject<Vec<u8>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl LogOutput {
    pub fn from<T>(val: db::Log, to: &PackObject<T>) -> Self {
        let mut rt = Self {
            uid: to.with(val.uid),
            id: to.with(val.id),
            action: action::from_action(val.action),
            status: val.status,
            ..Default::default()
        };

        for v in val._fields {
            match v.as_str() {
                "gid" => rt.gid = Some(to.with(val.gid)),
                "ip" => rt.ip = Some(val.ip.to_owned()),
                "payload" => rt.payload = Some(to.with(val.payload.to_owned())),
                "tokens" => rt.tokens = Some(val.tokens as u32),
                "error" => {
                    rt.error = if val.error.is_empty() {
                        None
                    } else {
                        Some(val.error.to_owned())
                    }
                }
                _ => {}
            }
        }

        rt
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct QueryLog {
    pub uid: PackObject<xid::Id>,
    pub id: PackObject<xid::Id>,
    pub fields: Option<String>,
}

pub async fn get(
    State(app): State<Arc<AppState>>,
    Extension(ctx): Extension<Arc<ReqContext>>,
    to: PackObject<()>,
    Query(input): Query<QueryLog>,
) -> Result<PackObject<SuccessResponse<LogOutput>>, HTTPError> {
    input.validate()?;

    ctx.set_kvs(vec![("action", "get_log".into())]).await;

    let mut doc = db::Log::with_pk(input.uid.unwrap(), input.id.unwrap());
    doc.get_one(&app.scylla, get_fields(input.fields)).await?;

    Ok(to.with(SuccessResponse::new(LogOutput::from(doc, &to))))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateLogInput {
    pub uid: PackObject<xid::Id>,
    pub gid: PackObject<xid::Id>,
    pub action: String,
    #[validate(range(min = -1, max = 1))]
    pub status: i8,
    pub ip: String,
    pub payload: PackObject<Vec<u8>>,
    #[validate(range(min = 0))]
    pub tokens: i32,
}

pub async fn create(
    State(app): State<Arc<AppState>>,
    Extension(ctx): Extension<Arc<ReqContext>>,
    to: PackObject<CreateLogInput>,
) -> Result<PackObject<SuccessResponse<LogOutput>>, HTTPError> {
    let (to, input) = to.unpack();
    input.validate()?;

    let i = action::to_action(&input.action)
        .ok_or_else(|| HTTPError::new(400, format!("invalid action {}", input.action)))?;

    ctx.set_kvs(vec![("action", "create_log".into())]).await;

    let mut doc = db::Log::with_pk(input.uid.unwrap(), xid::new());
    let mut cols: ColumnsMap = ColumnsMap::with_capacity(5);
    doc.action = i;
    cols.set_as("action", &i);
    cols.set_as("gid", &input.gid.unwrap());
    cols.set_as("ip", &input.ip);
    cols.set_as("payload", &input.payload.unwrap());
    cols.set_as("tokens", &input.tokens);

    doc.upsert_fields(&app.scylla, cols).await?;
    Ok(to.with(SuccessResponse::new(LogOutput::from(doc, &to))))
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateLogInput {
    pub uid: PackObject<xid::Id>,
    pub id: PackObject<xid::Id>,
    pub status: i8,
    pub payload: Option<PackObject<Vec<u8>>>,
    #[validate(range(min = 0))]
    pub tokens: Option<i32>,
    pub error: Option<String>,
}

pub async fn update(
    State(app): State<Arc<AppState>>,
    Extension(ctx): Extension<Arc<ReqContext>>,
    to: PackObject<UpdateLogInput>,
) -> Result<PackObject<SuccessResponse<LogOutput>>, HTTPError> {
    let (to, input) = to.unpack();
    input.validate()?;

    if input.status != -1 && input.status != 1 {
        return Err(HTTPError::new(
            400,
            format!("invalid status, expected -1 or 1, got {}", input.status),
        ));
    }

    ctx.set_kvs(vec![("action", "update_log".into())]).await;
    let mut doc = db::Log::with_pk(input.uid.unwrap(), input.id.unwrap());
    let mut cols: ColumnsMap = ColumnsMap::with_capacity(3);
    cols.set_as("status", &input.status);
    if input.payload.is_some() {
        cols.set_as("payload", &input.payload.unwrap().unwrap());
    }
    if input.tokens.is_some() {
        cols.set_as("tokens", &input.tokens.unwrap());
    }
    if input.error.is_some() {
        cols.set_as("error", &input.error.unwrap());
    }

    doc.upsert_fields(&app.scylla, cols).await?;
    Ok(to.with(SuccessResponse::new(LogOutput::from(doc, &to))))
}

#[derive(Debug, Deserialize, Validate)]
pub struct ListRecentlyInput {
    pub uid: PackObject<xid::Id>,
    #[validate(length(min = 1, max = 10))]
    pub actions: Vec<String>,
    pub fields: Option<Vec<String>>,
}

pub async fn list_recently(
    State(app): State<Arc<AppState>>,
    Extension(ctx): Extension<Arc<ReqContext>>,
    to: PackObject<ListRecentlyInput>,
) -> Result<PackObject<SuccessResponse<Vec<LogOutput>>>, HTTPError> {
    let (to, input) = to.unpack();
    input.validate()?;

    let mut actions: Vec<i8> = Vec::with_capacity(input.actions.len());
    for a in input.actions.iter() {
        let i = action::to_action(a)
            .ok_or_else(|| HTTPError::new(400, format!("invalid action {}", a)))?;
        actions.push(i);
    }

    ctx.set_kvs(vec![("action", "list_recently".into())]).await;
    let res = db::Log::list_recently(
        &app.scylla,
        input.uid.unwrap(),
        input.fields.unwrap_or_default(),
        actions,
    )
    .await?;
    Ok(to.with(SuccessResponse::new(
        res.iter()
            .map(|r| LogOutput::from(r.to_owned(), &to))
            .collect(),
    )))
}
