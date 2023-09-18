use axum_web::{context::unix_ms, erring::HTTPError};
use scylla_orm::{ColumnsMap, CqlValue, ToCqlVal};
use scylla_orm_macros::CqlOrm;

use crate::db::{scylladb, MAX_ID};

#[derive(Debug, Default, Clone, CqlOrm)]
pub struct Log {
    pub uid: xid::Id,
    pub id: xid::Id,
    pub action: i8,
    pub status: i8,
    pub gid: xid::Id,
    pub ip: String,
    pub payload: Vec<u8>,
    pub tokens: i32,
    pub error: String,

    pub _fields: Vec<String>, // selected fields，`_` 前缀字段会被 CqlOrm 忽略
}

impl Log {
    pub fn with_pk(uid: xid::Id, id: xid::Id) -> Self {
        Self {
            uid,
            id,
            ..Default::default()
        }
    }

    pub fn select_fields(select_fields: Vec<String>, with_pk: bool) -> anyhow::Result<Vec<String>> {
        if select_fields.is_empty() {
            return Ok(Self::fields());
        }

        let fields = Self::fields();
        for field in &select_fields {
            if !fields.contains(field) {
                return Err(HTTPError::new(400, format!("Invalid field: {}", field)).into());
            }
        }

        let mut select_fields = select_fields;
        let field = "action".to_string();
        if !select_fields.contains(&field) {
            select_fields.push(field);
        }
        let field = "status".to_string();
        if !select_fields.contains(&field) {
            select_fields.push(field);
        }
        if with_pk {
            let field = "uid".to_string();
            if !select_fields.contains(&field) {
                select_fields.push(field);
            }
            let field = "id".to_string();
            if !select_fields.contains(&field) {
                select_fields.push(field);
            }
        }

        Ok(select_fields)
    }

    pub async fn get_one(
        &mut self,
        db: &scylladb::ScyllaDB,
        select_fields: Vec<String>,
    ) -> anyhow::Result<()> {
        let fields = Self::select_fields(select_fields, false)?;
        self._fields = fields.clone();

        let query = format!(
            "SELECT {} FROM log WHERE uid=? AND id=? LIMIT 1",
            fields.join(",")
        );
        let params = (self.uid.to_cql(), self.id.to_cql());
        let res = db.execute(query, params).await?.single_row()?;

        let mut cols = ColumnsMap::with_capacity(fields.len());
        cols.fill(res, &fields)?;
        self.fill(&cols);

        Ok(())
    }

    pub async fn upsert_fields(
        &mut self,
        db: &scylladb::ScyllaDB,
        cols: ColumnsMap,
    ) -> anyhow::Result<bool> {
        let valid_fields = vec![
            "status", "gid", "action", "ip", "payload", "tokens", "error",
        ];

        let res = self.get_one(db, vec!["status".to_string()]).await;
        if res.is_ok() && self.status != 0 {
            return Err(HTTPError::new(400, "log is frozen".to_string()).into());
        }

        let mut set_fields: Vec<String> = Vec::with_capacity(cols.len());
        let mut params: Vec<CqlValue> = Vec::with_capacity(cols.len() + 4);
        for (k, v) in cols.iter() {
            if !valid_fields.contains(&k.as_str()) {
                return Err(HTTPError::new(400, format!("Invalid field: {}", k)).into());
            }
            set_fields.push(format!("{}=?", k));
            params.push(v.to_owned());
        }

        let query = format!(
            "UPDATE log SET {} WHERE uid=? AND id=?",
            set_fields.join(",")
        );
        params.push(self.uid.to_cql());
        params.push(self.id.to_cql());

        let _ = db.execute(query, params).await?;
        Ok(true)
    }

    pub async fn list(
        db: &scylladb::ScyllaDB,
        uid: xid::Id,
        select_fields: Vec<String>,
        page_size: u16,
        page_token: Option<xid::Id>,
        action: Option<i8>,
    ) -> anyhow::Result<Vec<Log>> {
        let fields = Self::select_fields(select_fields, true)?;
        let token = if page_token.is_none() {
            MAX_ID
        } else {
            page_token.unwrap()
        };

        let rows = if action.is_none() {
            let query = format!(
                "SELECT {} FROM log WHERE uid=? AND id<? LIMIT ? USING TIMEOUT 3s",
                fields.clone().join(",")
            );
            let params = (uid.to_cql(), token.to_cql(), page_size as i32);
            db.execute_iter(query, params).await?
        } else {
            let query = format!(
                "SELECT {} FROM log WHERE uid=? AND action=? AND id<? LIMIT ? USING TIMEOUT 3s",
                fields.clone().join(",")
            );
            let params = (
                uid.to_cql(),
                token.to_cql(),
                action.unwrap(),
                page_size as i32,
            );
            db.execute_iter(query, params).await?
        };

        let mut res: Vec<Log> = Vec::with_capacity(rows.len());
        for row in rows {
            let mut doc = Log::default();
            let mut cols = ColumnsMap::with_capacity(fields.len());
            cols.fill(row, &fields)?;
            doc.fill(&cols);
            doc._fields = fields.clone();
            res.push(doc);
        }

        Ok(res)
    }

    pub async fn list_recently(
        db: &scylladb::ScyllaDB,
        uid: xid::Id,
        select_fields: Vec<String>,
        actions: Vec<i8>,
    ) -> anyhow::Result<Vec<Log>> {
        let fields = Self::select_fields(select_fields, true)?;

        // from 3 days ago
        let unix_ts = (unix_ms() / 1000 - 3600 * 24 * 3) as u32;
        let mut id = xid::Id::default();
        id.0[0..=3].copy_from_slice(&unix_ts.to_be_bytes());

        let rows = if actions.is_empty() {
            let query = format!(
                "SELECT {} FROM log WHERE uid=? AND id>? LIMIT ? USING TIMEOUT 3s",
                fields.clone().join(","),
            );

            let mut params: Vec<CqlValue> = Vec::with_capacity(3);
            params.push(uid.to_cql());
            params.push(id.to_cql());
            params.push(1000_i32.to_cql());
            db.execute_iter(query, params).await?
        } else {
            let query = format!(
                "SELECT {} FROM log WHERE uid=? AND id>? AND action IN ({}) LIMIT ? ALLOW FILTERING USING TIMEOUT 3s",
                fields.clone().join(","),
                actions.iter().map(|_| "?").collect::<Vec<&str>>().join(",")
            );

            let mut params: Vec<CqlValue> = Vec::with_capacity(actions.len() + 3);
            params.push(uid.to_cql());
            params.push(id.to_cql());
            for a in &actions {
                params.push(a.to_cql());
            }
            params.push(1000_i32.to_cql());
            db.execute_iter(query, params).await?
        };

        let mut res: Vec<Log> = Vec::with_capacity(rows.len());
        for row in rows {
            let mut doc = Log::default();
            let mut cols = ColumnsMap::with_capacity(fields.len());
            cols.fill(row, &fields)?;
            doc.fill(&cols);
            doc._fields = fields.clone();
            res.push(doc);
        }

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::OnceCell;

    use crate::conf;

    use super::*;

    static DB: OnceCell<scylladb::ScyllaDB> = OnceCell::const_new();

    async fn get_db() -> scylladb::ScyllaDB {
        let cfg = conf::Conf::new().unwrap_or_else(|err| panic!("config error: {}", err));
        let res = scylladb::ScyllaDB::new(cfg.scylla, "logbase_test").await;
        res.unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore]
    async fn log_model_works() {
        let db = DB.get_or_init(get_db).await;
        let uid = xid::new();
        let id = xid::new();
        let mut doc = Log::with_pk(uid, id);

        let res = doc.get_one(db, vec![]).await;
        assert!(res.is_err());
        let err: HTTPError = res.unwrap_err().into();
        assert_eq!(err.code, 404);

        let content: Vec<u8> = vec![0x80];

        let mut cols = ColumnsMap::with_capacity(4);
        cols.set_as("action", &1i8);
        cols.set_as("ip", &"1.2.3.4".to_string());
        cols.set_as("tokens", &(1000i32));
        cols.set_as("payload", &content);

        doc.upsert_fields(db, cols).await.unwrap();

        let mut doc2 = Log::with_pk(uid, id);
        doc2.get_one(db, vec![]).await.unwrap();

        assert_eq!(doc2.action, 1i8);
        assert_eq!(doc2.gid, xid::Id::default());
        assert_eq!(doc2.ip, "1.2.3.4".to_string());
        assert_eq!(doc2.tokens, 1000i32);
        assert_eq!(doc2.payload, content);
        assert_eq!(doc2.error, "".to_string());

        let mut doc3 = Log::with_pk(uid, id);
        doc3.get_one(db, vec!["error".to_string()]).await.unwrap();
        assert_eq!(doc3.tokens, 0i32);
        assert_eq!(doc3.payload.len(), 0);
        assert_eq!(doc3.error, "".to_string());

        let mut cols = ColumnsMap::with_capacity(1);
        cols.set_as("error", &"some error".to_string());
        doc.upsert_fields(db, cols).await.unwrap();

        let mut doc3 = Log::with_pk(uid, id);
        doc3.get_one(db, vec![]).await.unwrap();
        assert_eq!(doc3.tokens, 1000i32);
        assert_eq!(doc3.payload, content);
        assert_eq!(doc3.error, "some error".to_string());

        let mut doc = Log::with_pk(uid, xid::new());
        let mut cols = ColumnsMap::with_capacity(1);
        cols.set_as("action", &2i8);
        cols.set_as("error", &"some error".to_string());
        doc.upsert_fields(db, cols).await.unwrap();
        doc.get_one(db, vec![]).await.unwrap();
        assert_eq!(doc.tokens, 0i32);
        assert_eq!(doc.payload.len(), 0);
        assert_eq!(doc.error, "some error".to_string());

        let docs = Log::list_recently(db, uid, vec![], vec![1i8, 2i8])
            .await
            .unwrap();
        assert_eq!(2, docs.len());
        assert_eq!(docs[0].action, 2i8);
        assert_eq!(docs[1].action, 1i8);
    }
}
