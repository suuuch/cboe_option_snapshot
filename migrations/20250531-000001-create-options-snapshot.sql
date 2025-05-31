-- 创建 t_options_cboe_snapshot 表
CREATE TABLE IF NOT EXISTS t_options_cboe_snapshot
(
    id                SERIAL PRIMARY KEY,
    symbol            TEXT             NOT NULL,
    call_put          TEXT             NOT NULL,
    expiration        TEXT             NOT NULL,
    strike_price      DOUBLE PRECISION NOT NULL,
    volume            BIGINT           NOT NULL,
    matched           BIGINT           NOT NULL,
    routed            BIGINT           NOT NULL,
    bid_size          BIGINT           NOT NULL,
    bid_price         DOUBLE PRECISION NOT NULL,
    ask_size          BIGINT           NOT NULL,
    ask_price         DOUBLE PRECISION NOT NULL,
    last_price        DOUBLE PRECISION NOT NULL,
    last_updated_time TIMESTAMP        NOT NULL,
    etl_in_dt         TIMESTAMP        NOT NULL,
    constraint t_options_cboe_snapshot_pk
        primary key (symbol, call_put, strike_price, expiration, last_updated_time)
);

-- 创建索引提升查询速度
CREATE INDEX IF NOT EXISTS idx_options_symbol ON t_options_cboe_snapshot (symbol);
CREATE INDEX IF NOT EXISTS idx_options_expiration ON t_options_cboe_snapshot (expiration);
CREATE INDEX IF NOT EXISTS idx_options_last_updated ON t_options_cboe_snapshot (last_updated_time);
