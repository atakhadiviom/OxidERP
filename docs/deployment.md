# Deployment

OxidERP can run directly with Cargo for development or with Docker Compose for a production-like setup.

## Local server already used on this VPS

```bash
DATABASE_URL=postgres://oxiderp:oxiderp_dev_password@127.0.0.1:5432/oxiderp cargo run -p oxiderp-core
```

Open:

```text
http://SERVER_IP:8080
```

## Docker Compose

1. Copy the example environment:

```bash
cp .env.example .env
```

2. Set a strong database password in `.env`:

```env
POSTGRES_PASSWORD=change_this_password
```

3. Start services:

```bash
docker compose up -d --build
```

4. Open:

```text
http://localhost:8080
```

## Demo login

```text
Email: admin@demo.com
Password: admin123
```

## Health checks

HTTP:

```bash
curl http://localhost:8080/api/health
```

Container command:

```bash
oxiderp-core --healthcheck
```

## Production notes

Before real production use:

- Replace demo password seeding
- Put the app behind HTTPS reverse proxy
- Use managed PostgreSQL or scheduled backups
- Set strong secrets/passwords
- Configure firewall to expose only needed ports
