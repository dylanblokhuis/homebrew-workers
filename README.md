## Setup

1. Copy `.env.example` to `.env`

2. Fill `JWT_SECRET`, `ADMIN_CLIENT_ID` and `ADMIN_CLIENT_SECRET`

3. Startup the services `docker-compose up -d`

4. Login to minio at `http://localhost:9000`

5. Create a user with readwrite (copy the `S3_ACCESS_KEY` and `S3_SECRET_KEY` to .env) and create a bucket (set the `S3_BUCKET` in `.env`)


## Developing apps
To develop apps you can use the CLI to watch the script and restart instantly.
```sh
cargo install --path ./cli
```

```sh
hbw help
```

```sh
DATABASE_URL=postgres://workers:password@localhost/workers hbw run /path/to/folder --watch
```