# Releasing the Trigix node SDKs

Both SDKs publish automatically from CI when you push a version tag. The actual
publish requires registry accounts you own; the steps below are one-time setup
plus a tag push per release.

## Package names (decide first)

| SDK | Package | Notes |
|-----|---------|-------|
| Python | `trigix-node-sdk` (PyPI) | Confirm the name is free at https://pypi.org/project/trigix-node-sdk/ |
| TypeScript | `trigix-node-sdk` (npm) | Unscoped; confirm it's free at https://www.npmjs.com/package/trigix-node-sdk |

## Python → PyPI (Trusted Publishing, no token)

1. On PyPI → *Your projects* → *Publishing*, add a **pending publisher**:
   - PyPI project name: `trigix-node-sdk`
   - Owner: `bj-qizhi`, Repository: `trigix`
   - Workflow: `publish-python-sdk.yml`, Environment: `pypi`
2. In GitHub repo settings → Environments, create an environment named `pypi`.
3. Bump `version` in `sdk/python/pyproject.toml`, commit.
4. Tag and push:
   ```bash
   git tag py-sdk-v0.1.0
   git push github py-sdk-v0.1.0
   ```
   CI tests, builds, and publishes. Verify at https://pypi.org/project/trigix-node-sdk/.

## TypeScript → npm (token)

1. Create an npm **automation** access token (npmjs.com → Access Tokens).
2. Add it as a repo secret `NPM_TOKEN` (Settings → Secrets → Actions).
3. Bump `version` in `sdk/typescript/package.json`, commit.
4. Tag and push:
   ```bash
   git tag ts-sdk-v0.1.0
   git push github ts-sdk-v0.1.0
   ```
   CI tests and publishes. Verify at https://www.npmjs.com/package/trigix-node-sdk.

## Manual publish (fallback)

```bash
# Python
cd sdk/python && python -m build && twine upload dist/*

# npm
cd sdk/typescript && npm publish
```

> Publishing is irreversible: a published version can be yanked/deprecated but
> not truly removed, and the name is claimed. Double-check the version and
> package name before tagging.
