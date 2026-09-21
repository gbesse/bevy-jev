# Contributing

This guide explains how to contribute code, tests and integration notes.

Use Node.js 22 or newer. Runtime tests and the demo need no dependency installation or build.
For declaration checks, run `npm ci --ignore-scripts` and `npm run typecheck`.
Run `npm run check`, `npm test`, and `npm run demo` before submitting a pull request.
Tests talk to a fake Jev server on loopback; never add a test that calls `api.typesafe.ai`.
Add a regression case when fixing nontrivial behavior. Keep sample data synthetic,
mark fixture-based demonstrations explicitly, and never commit credentials or customer records.
When adding an integration snippet to `docs/integrations.md`, say whether you executed it.
Describe the problem, resulting behavior, and validation in your pull request.
