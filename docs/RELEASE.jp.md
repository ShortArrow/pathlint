# リリース手順

新しい pathlint バージョンを切り出す手順は **ボタン 1 つ**：
**Actions → release → Run workflow → 新バージョンを入力 → Run**。

それだけ。bump も tag もビルドも GitHub Release 作成も
crates.io publish も `.github/workflows/release.yml` が全部やる。

`develop` ブランチ無し。手で書く `chore: release` commit 無し。
`CHANGELOG.md` 無し（リリースノートは PR タイトルから自動生成）。

## TL;DR

1. Actions → **release** → **Run workflow** を開く。
2. 新バージョンを入力（例: `0.0.8`）。`v` は付けない（workflow が
   tag に付ける）。
3. **Run workflow** を押す。

ユーザーがやることはこれだけ。残りは workflow が何をするか、
失敗したらどうするか、新規リポでの 1 回設定だけ。

## バージョン方針

- `0.0.x` と `0.1.x` ではどちらも TOML schema や CLI 表面の互換を
  壊しうる（GitHub Release の本文に明記）。
- 1.0 前は **patch bump（`0.0.A` → `0.0.A+1`）** がデフォルト。
  出荷したい振る舞い変更があれば bump する。
- **minor bump（`0.0.x` → `0.1.0`）** は schema/CLI を「通常の
  semver 契約に乗せる」と宣言できる段階に予約する。

## workflow が何をするか

`release.yml` は 4 ジョブを順に走らせる：

1. **prepare**: `cargo set-version` で `Cargo.toml`（と
   `Cargo.lock`）を入力 version に bump、標準ゲート（`fmt
   --check` / `clippy -D warnings` / `cargo test` / `cargo
   package --allow-dirty`）を走らせ、`chore: release X.Y.Z` で
   commit、`vX.Y.Z` を tag、自動付与の `GITHUB_TOKEN` で `main`
   に push。
2. **build**: ubuntu-latest / windows-latest / macos-latest で
   `x86_64-unknown-linux-gnu` / `x86_64-pc-windows-msvc` /
   `x86_64-apple-darwin` / `aarch64-apple-darwin` のリリースバ
   イナリをクロスビルド。Termux はソースから build。
3. **publish-github**: `SHA256SUMS` を作り、GitHub Release を
   作成、全アーカイブと checksum を添付、前の tag から今回の
   tag までの PR タイトルを使ってリリースノートを自動生成
   （`generate_release_notes: true`）。tag が `v0.*` なら
   prerelease としてマーク。
4. **publish-crates**: `rust-lang/crates-io-auth-action@v1` で
   workflow の OIDC アイデンティティを crates.io の短期トークンに
   交換し、`cargo publish` を走らせる。長期保存される
   `CARGO_REGISTRY_TOKEN` は使わない。

## 良いリリースノートを出すには

自動生成ノートは PR タイトルの質次第。読みやすく保つため、
**全 PR タイトルが Conventional Commits 形式**であることを
`.github/workflows/pr-title-check.yml` で強制する。許可される
type：

```
feat fix refactor perf test docs build ci chore revert
```

例：

```
feat: pathlint sort --dry-run (R5 read-only PATH repair)
fix(catalog): correct unix fallback for termux
refactor!: drop bump-on-main flow
chore(deps): bump clap to 4.6
```

GitHub はこれを `### Features` / `### Bug Fixes` /
`### Other Changes` のセクションに自動分類する。

## 1 回だけ設定するもの

リポ外で 1 回設定すれば終わるもの 2 つ：

1. **crates.io Trusted Publishing**。crates.io のクレート設定 →
   「Trusted Publishers」 → 「Add publisher」を開いて入力：
   - Repository owner / name: `ShortArrow/pathlint`
   - Workflow filename: `release.yml`
   - Environment: 空欄（GitHub environment では gate しない）。

   最初の publish は手動 token で済ませる必要がある。
   そのあと 0.0.8 以降は workflow 駆動になる。
2. **`main` の branch protection**。`ci` と `pr-title-check` を
   merge 前 status check として必須にし、force push を不許可。
   `release.yml` の `prepare` ジョブは `github-actions[bot]` と
   して `main` に直接 push するが、デフォルトの保護ルールは
   `GITHUB_TOKEN` 経由なら許可している。

## 失敗したとき

4 ジョブが順序立っているので、どこで失敗したかが診断しやすい：

- **prepare 失敗**: 何も push されていない。`main` で直して、
  同じバージョンで workflow を再実行。
- **prepare 成功後、build 失敗**: tag は既に `main` に乗って push
  されている。fix-forward して `X.Y.Z+1` に bump するか、tag を
  消す（`git push origin :refs/tags/vX.Y.Z`、ローカルは
  `git tag -d`）して同じ version で再実行。`chore: release`
  commit は main に残しても害はない。
- **publish-github 失敗**: GitHub Release が見えない。アーティ
  ファクトは build ジョブの workflow artifact に残っているので、
  publish-github ジョブだけ再実行できる。
- **publish-crates 失敗**: GitHub Release はあるが crates.io に
  この version が無い状態。publish-crates ジョブだけ再実行する。
  crates.io は同じ version の再 publish を拒否するので、ネット
  ワーク的に crates.io 側に記録された失敗だった場合は
  `X.Y.Z+1` に bump する必要がある。

リリース全体を取り消したい場合：

```sh
# ローカルで prepare commit + tag を取り消して remote に反映。
git switch main && git pull --ff-only
git reset --hard HEAD~1                   # chore: release X.Y.Z を消す
git push --force-with-lease origin main
git push origin :refs/tags/vX.Y.Z
```

`--force` ではなく `--force-with-lease` を使うこと。

## 検証

別マシンで実物を取得：

```sh
curl -L -o pathlint.tar.gz \
  "https://github.com/ShortArrow/pathlint/releases/download/vX.Y.Z/pathlint-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz"
tar -xzf pathlint.tar.gz
./pathlint-vX.Y.Z-x86_64-unknown-linux-gnu/pathlint --version
```

表示される version が tag と一致すること。同じ Release の
`SHA256SUMS` で checksum を検証する。

## release.yml が壊れているときの手動 fallback

workflow 自体に問題があってどうしても出さなければならないとき、
`scripts/release-check.sh X.Y.Z` がローカルで同じゲート
（`fmt --check` / `clippy -D warnings` / `cargo test` /
`cargo package`）を走らせる。pass したら：

```sh
cargo set-version X.Y.Z
git commit -am "chore: release X.Y.Z"
git tag -a vX.Y.Z -m "pathlint X.Y.Z"
git push origin main vX.Y.Z
gh release create vX.Y.Z --generate-notes ...
cargo publish
```

意図的に面倒にしてある。`release.yml` の存在意義は「これを
手動でやらなくて済む」こと。手動 fallback は workflow が
壊れているときだけ使い、可能なら先に workflow を直すこと。
