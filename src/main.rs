use anyhow::Result;
use chrono::NaiveDateTime;
use log::{info, error};
use regex::Regex;
use reqwest::Client;
use sqlx::{PgPool, Row};
use std::io::Cursor;
use csv::ReaderBuilder;
use chrono_tz::{America};

const PAGE_URL: &str = "https://www.cboe.com/us/options/market_statistics/symbol_data/?mkt=cone";
const URLS: [&str; 4] = [
    "https://www.cboe.com/us/options/market_statistics/symbol_data/csv/?mkt=cone",
    "https://www.cboe.com/us/options/market_statistics/symbol_data/csv/?mkt=opt",
    "https://www.cboe.com/us/options/market_statistics/symbol_data/csv/?mkt=ctwo",
    "https://www.cboe.com/us/options/market_statistics/symbol_data/csv/?mkt=exo"
];

struct OptionRecord {
    symbol: String,
    call_put: String,
    expiration: String,
    strike_price: f64,
    volume: i64,
    matched: i64,
    routed: i64,
    bid_size: i64,
    bid_price: f64,
    ask_size: i64,
    ask_price: f64,
    last_price: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    env_logger::init();

    let db_url = std::env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&db_url).await?;

    let client = Client::new();

    let last_update_time = get_page_content_last_update_time(&client).await?;
    let max_updated_time = get_max_updated_date(&pool).await?;

    if Some(last_update_time) == max_updated_time {
        info!("Already updated, no need to update.");
        return Ok(());
    }

    for url in URLS {
        let records = get_csv_content(&client, url).await?;
        insert_records(&pool, &records, last_update_time).await?;
    }

    clean_duplicate_data(&pool).await?;

    Ok(())
}

async fn get_page_content_last_update_time(client: &Client) -> Result<NaiveDateTime> {
    let resp = client.get(PAGE_URL).send().await?.text().await?;
    let re = Regex::new(r"last updated (\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})")?;
    if let Some(caps) = re.captures(&resp) {
        let dt = NaiveDateTime::parse_from_str(&caps[1], "%Y-%m-%d %H:%M:%S")?;
        info!("Last update time: {}", dt);
        Ok(dt)
    } else {
        error!("Failed to parse last update time.");
        Err(anyhow::anyhow!("Parse error"))
    }
}

async fn get_max_updated_date(pool: &PgPool) -> Result<Option<NaiveDateTime>> {
    let row = sqlx::query("SELECT max(last_updated_time) FROM t_options_cboe_snapshot")
        .fetch_one(pool)
        .await?;
    let max_time: Option<NaiveDateTime> = row.try_get(0)?;
    Ok(max_time)
}
async fn get_csv_content(client: &Client, url: &str) -> Result<Vec<OptionRecord>> {
    info!("Fetching CSV from {}", url);
    let resp = client.get(url).send().await?.bytes().await?;
    let cursor = Cursor::new(resp);

    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(cursor);

    let mut records = Vec::new();
    for result in rdr.records() {
        let record = result?;
        if record.len() < 12 {
            continue;
        }

        let option_record = OptionRecord {
            symbol: record[0].to_string(),
            call_put: record[1].to_string(),
            expiration: record[2].to_string(),
            strike_price: record[3].parse().unwrap_or(0.0),
            volume: record[4].parse().unwrap_or(0),
            matched: record[5].parse().unwrap_or(0),
            routed: record[6].parse().unwrap_or(0),
            bid_size: record[7].parse().unwrap_or(0),
            bid_price: record[8].parse().unwrap_or(0.0),
            ask_size: record[9].parse().unwrap_or(0),
            ask_price: record[10].parse().unwrap_or(0.0),
            last_price: record[11].parse().unwrap_or(0.0),
        };

        records.push(option_record);
    }

    Ok(records)
}
async fn insert_records(pool: &PgPool, records: &[OptionRecord], last_updated_time: NaiveDateTime) -> Result<()> {
    let mut tx = pool.begin().await?;
    let utc_now = chrono::Utc::now();
    let etl_in_dt = utc_now.with_timezone(&America::New_York);

    for rec in records {
        sqlx::query(r#"
            INSERT INTO t_options_cboe_snapshot
            (symbol, call_put, expiration, strike_price, volume, matched, routed, bid_size, bid_price, ask_size, ask_price, last_price, last_updated_time, etl_in_dt)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (symbol, call_put, expiration, strike_price, last_updated_time)
            DO UPDATE SET
                volume = EXCLUDED.volume,
                matched = EXCLUDED.matched,
                routed = EXCLUDED.routed,
                bid_size = EXCLUDED.bid_size,
                bid_price = EXCLUDED.bid_price,
                ask_size = EXCLUDED.ask_size,
                ask_price = EXCLUDED.ask_price,
                last_price = EXCLUDED.last_price,
                etl_in_dt = EXCLUDED.etl_in_dt
        "#)
            .bind(&rec.symbol)
            .bind(&rec.call_put)
            .bind(&rec.expiration)
            .bind(rec.strike_price)
            .bind(rec.volume)
            .bind(rec.matched)
            .bind(rec.routed)
            .bind(rec.bid_size)
            .bind(rec.bid_price)
            .bind(rec.ask_size)
            .bind(rec.ask_price)
            .bind(rec.last_price)
            .bind(last_updated_time)
            .bind(etl_in_dt)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    info!("Inserted {} records.", records.len());
    Ok(())
}


async fn clean_duplicate_data(pool: &PgPool) -> Result<()> {
    let sql = r#"
        DELETE FROM t_options_cboe_snapshot a
        USING (
            SELECT ctid FROM (
                SELECT
                    ctid,
                    ROW_NUMBER() OVER (PARTITION BY symbol, expiration,call_put,strike_price, last_updated_time ORDER BY etl_in_dt DESC) AS rn
                FROM t_options_cboe_snapshot
            ) t
            WHERE t.rn > 1
        ) b
        WHERE a.ctid = b.ctid;
    "#;

    sqlx::query(sql)
        .execute(pool)
        .await?;

    info!("Duplicate data cleaned.");
    Ok(())
}
