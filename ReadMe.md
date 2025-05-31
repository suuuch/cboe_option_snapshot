📦 使用方式：
如果你已经装好了 sqlx-cli（没有的话）

```bash
cargo install sqlx-cli --no-default-features --features postgres
```
然后执行：

```bash
sqlx migrate run --database-url "postgresql://user:password@localhost:5432/db_name"
```

就会自动执行这个 migration，建好表和索引。


查看 migration 状态
```bash
sqlx migrate info --database-url "postgresql://user:password@localhost:5432/db_name"
```

能看到每个 migration 执行情况。