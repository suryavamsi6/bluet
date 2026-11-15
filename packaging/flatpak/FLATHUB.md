# Publishing Bluet on Flathub

CI builds `bluet-x86_64.flatpak` on every push and pull request and attaches it
to each `v*` release. Getting Bluet onto Flathub itself is a one-time manual
submission that Flathub reviewers approve; after that, updates are pull
requests against Flathub's own repository for the app.

## Blocker: a project license

Flathub does not accept an app without a declared license. Bluet has none yet
(see `packaging/linux/PROJECT-LICENSE-NOTICE`). Before submitting:

1. Choose a license and add a `LICENSE` file to this repository.
2. Add `<project_license>SPDX-ID</project_license>` to
   `io.github.suryavamsi6.bluet.metainfo.xml`, after `<metadata_license>`.
3. Install the license in the manifest as
   `/app/share/licenses/io.github.suryavamsi6.bluet/LICENSE`.

## Before submitting

- The app id `io.github.suryavamsi6.bluet` is verified through the GitHub
  account `suryavamsi6`, so no domain is needed. Keep the repository public.
- Tag a release that contains the license and this Flatpak packaging, and make
  sure its `<release>` entry exists in the metainfo.
- Lint the manifest and the built repo with Flathub's linter:

  ```sh
  flatpak install -y flathub org.flatpak.Builder
  packaging/flatpak/update-cargo-sources.sh
  flatpak run --command=flatpak-builder-lint org.flatpak.Builder \
    manifest packaging/flatpak/io.github.suryavamsi6.bluet.yml
  flatpak run org.flatpak.Builder --force-clean --sandbox --user --install \
    --install-deps-from=flathub --ccache --mirror-screenshots-url=https://dl.flathub.org/media/ \
    --repo=repo builddir packaging/flatpak/io.github.suryavamsi6.bluet.yml
  flatpak run --command=flatpak-builder-lint org.flatpak.Builder repo repo
  ```

## One-time submission

1. Fork <https://github.com/flathub/flathub> **with all branches** (untick
   "Copy the master branch only"), then clone the `new-pr` branch:

   ```sh
   git clone --branch=new-pr git@github.com:suryavamsi6/flathub.git
   cd flathub
   git checkout -b io.github.suryavamsi6.bluet new-pr
   ```

2. Copy in the manifest and the generated cargo sources:

   ```sh
   cp /path/to/bluet/packaging/flatpak/io.github.suryavamsi6.bluet.yml .
   cp /path/to/bluet/packaging/flatpak/cargo-sources.json .
   ```

3. In the copied manifest, replace the `type: dir` source with the tagged
   release, pinned to its commit:

   ```yaml
       sources:
         - type: git
           url: https://github.com/suryavamsi6/bluet.git
           tag: v1.2.0
           commit: <full commit sha of v1.2.0>
         - cargo-sources.json
   ```

4. Bluet is only built and tested on x86_64, so add `flathub.json`:

   ```json
   { "only-arches": ["x86_64"] }
   ```

   Drop this file once aarch64 is tested.

5. Commit, push, and open a pull request against the **`new-pr`** base branch
   (not `master`) titled `Add io.github.suryavamsi6.bluet`. Fill in the
   checklist in the template. Expect a reviewer to ask why the app needs
   `--system-talk-name=org.bluez` (it is a Bluetooth manager talking to BlueZ)
   and `--socket=pulseaudio` (audio controls run `pactl`).

6. Comment `bot, build` on the pull request to get a test build, and address
   review comments. Once merged, Flathub creates
   `github.com/flathub/io.github.suryavamsi6.bluet` and invites you as a
   maintainer; the app appears on flathub.org after the first official build.

## Updates after acceptance

Every new Bluet tag needs a pull request to
`flathub/io.github.suryavamsi6.bluet` that bumps `tag` and `commit` and
regenerates `cargo-sources.json`. To have that pull request opened
automatically, add `x-checker-data` to the git source there so Flathub's
external data checker watches new tags:

```yaml
        - type: git
          url: https://github.com/suryavamsi6/bluet.git
          tag: v1.2.0
          commit: <sha>
          x-checker-data:
            type: git
            tag-pattern: ^v([\d.]+)$
```

The checker updates `tag` and `commit` but not `cargo-sources.json`, so when
`Cargo.lock` changed in a release, regenerate it with
`packaging/flatpak/update-cargo-sources.sh` and push it to the checker's
pull request before merging. Add a `<release>` entry to the metainfo here for
each version.
