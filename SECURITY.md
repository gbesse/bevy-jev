# Security

This file describes the security boundary of this early release.

Do not post secrets, personal data or exploit details in public issues. Use GitHub's
private vulnerability reporting when enabled.

The server holds one secret, `TYPESAFE_API_KEY`, and never writes it to responses or logs;
Jev error text is redacted before it is returned. Bind to `127.0.0.1` (the default) or put
the server behind your own TLS and authentication; `RERANK_SERVER_TOKEN` is a shared bearer
token, not an access-control system. Queries and documents are sent to `api.typesafe.ai`
when the real client is configured; do not send data you are not allowed to share with that
provider. Relevance scores are model judgments and never grant authorization. Errors
propagate as structured HTTP error bodies and stderr lines; no automatic emails or telemetry
are sent.
