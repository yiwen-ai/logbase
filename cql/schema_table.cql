CREATE TABLE IF NOT EXISTS log (
    uid      BLOB,     -- user id, 12 bytes, https://docs.rs/xid/latest/xid/
    id       BLOB,     -- log id
    action   TINYINT,  -- log action
    status   TINYINT,  -- log status, -1: failed, 0: processing, 1: success
    gid      BLOB,     -- group id
    ip       TEXT,     -- ip address
    payload  BLOB,     -- a well pruned content in CBOR format
    tokens   INT,
    error    TEXT,     -- error message if failed at end
    PRIMARY KEY (uid, id)
) WITH CLUSTERING ORDER BY (id DESC)
    AND caching = {'enabled': 'true'}
    AND comment = 'content translating'
    AND compaction = {'class': 'SizeTieredCompactionStrategy'}
    AND compression = {'sstable_compression': 'LZ4Compressor'}
    AND default_time_to_live = 0;

CREATE INDEX log_uid_gid ON log ((uid), gid);
CREATE INDEX log_uid_action ON log ((uid), action);
CREATE INDEX log_gid ON log (gid);