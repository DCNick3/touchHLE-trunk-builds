# touchHLE-trunk-builds

This is a simple proxy server that allows you to download trunk build from
the [touchHLE CI](https://github.com/touchHLE/touchHLE/actions?query=branch%3Atrunk) without a GitHub account.

## API endpoints

### Download latest trunk build

`https://touchhle-trunk-builds.dcnick3.me/api/download_latest/:artifact_name`

where `:artifact_name` is either `touchHLE_Android_AArch64`, `touchHLE_macOS_x86_64` or `touchHLE_Windows_x86_64`

This will download a zip file with the following name format:
`touchHLE_preview_r[run_number]-[commit hash]-[build time]_[platform].zip`

### Get a list of last 30 trunk builds info

`https://touchhle-trunk-builds.dcnick3.me/api/builds`

### Get the latest trunk build info

`https://touchhle-trunk-builds.dcnick3.me/api/builds/latest`

### Download a specific trunk build

`https://touchhle-trunk-builds.dcnick3.me/api/download/:run_id/:artifact_name`

where `:run_id` is the run id of the build (can be taken from build info JSON) and `:artifact_name` is either
`touchHLE_Android_AArch64`, `touchHLE_macOS_x86_64` or `touchHLE_Windows_x86_64`.

The zip file name formatting is the same as for the latest build.

## Deployment

0. Get a GitHub token with access to the CI artifacts.
1. Get a docker container either by building it from the `Dockerfile` or by fetching it
   from [the registry](https://github.com/DCNick3/touchHLE-trunk-builds/pkgs/container/touchhle-trunk-builds).
2. `docker run -p 3000:3000 -e CONFIG_GITHUB__TOKEN=<your_github_token> <container_id>`

^ note that this is just a minimal setup, you might want to put a reverse proxy for HTTPS and other production stuff.
