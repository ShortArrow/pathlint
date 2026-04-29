# リリース手順

新しい pathlint バージョンを切り出すためのチェックリスト。
0.0.x / 0.1.x の小刻みなリリースサイクル（時々 schema を壊す）に
最適化されている。

## 前提条件

- 新バージョンに入れる作業が `develop` 上にあり、CI が green。
- `develop` と `main` の作業ツリーがクリーン。
- `origin` への push 権がある。
- crates.io publish したい場合は `cargo login` 設定済み（バイナリ
  は GitHub Release pipeline 側でやるので、これは optional）。

## バージョン方針

- `0.0.x` と `0.1.x` ではどちらも TOML schema や CLI 表面の互換を
  壊しうる（`CHANGELOG.md` に明記）。
- 1.0 前は **patch bump（`0.0.A` → `0.0.A+1`）** がデフォルト。出荷
  したい振る舞い変更があれば bump する。
- **minor bump（`0.0.x` → `0.1.0`）** は schema/CLI を「通常の semver
  契約に乗せる」と宣言できる段階に予約する。

## 手順

番号は 0.0.2 で実際に走らせた手順の通り。

### 1. `develop` の sanity check

```sh
git switch develop
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

どれか赤ければ先に `develop` で直す。赤いままマージしない。

### 2. `develop` を `--no-ff` で `main` へマージ

```sh
git switch main
git pull --ff-only           # まず origin に追いつかせる
git merge --no-ff develop -m "Merge branch 'develop' for X.Y.Z

<このリリースに何が入っているかの短いサマリ>"
```

`--no-ff` が肝。`main` のヒストリにリリースごとに 1 つ merge コミ
ットが残るので、`git log --first-parent main` が「リリース単位の
タイムライン」として読める。squash や fast-forward だとこの形が
失われる。

### 3. `main` 上で 1 コミットで bump

編集する：

- `Cargo.toml` — `version = "X.Y.Z"`
- `CHANGELOG.md`:
  - 先頭の `## [Unreleased]` を `## [X.Y.Z] - YYYY-MM-DD`（今日の
    日付、ISO-8601）に置き換え。
  - その上に新しい空の `## [Unreleased]` を追加。
  - 末尾の比較リンク：
    - `[Unreleased]: .../compare/vX.Y.Z...HEAD`
    - `[X.Y.Z]: .../releases/tag/vX.Y.Z`

その後 `Cargo.lock` を同期：

```sh
cargo build         # 新しい version を Cargo.lock に取り込む
cargo test          # 念押しで sanity
./target/debug/pathlint --version   # 新しい version を表示するはず
```

コミット：

```sh
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release X.Y.Z

<1 段落: このバージョンで何が注目すべきか、なぜ bump するのか、
ユーザーが気をつけるべきことがあれば>"
```

### 4. `main` を `develop` に forward-merge

`develop` が常に「`main` に入ってるすべて + 次バージョンの作業中」
という状態を保つため。

```sh
git switch develop
git merge --ff-only main
```

fast-forward にならない（リリース中に `develop` に commit が乗った）
場合は通常の `git merge main` でよい — でもリリースは serialize
させた方がシンプル。

### 5. ブランチと tag を push

tag が `release.yml` を起動して `x86_64-{linux,windows,darwin}` +
`aarch64-darwin` のバイナリをビルドし、アーカイブと checksum を
固めて GitHub Release を作る。

```sh
git push origin main develop
git tag -a vX.Y.Z -m "pathlint X.Y.Z"
git push origin vX.Y.Z
```

Actions タブを監視。`release.yml` が数分以内に green になるはず。
バージョンが `v0.` で始まる間は `prerelease: true` で公開され、
`v1.0.0` で通常リリースに切り替わる。

### 6. crates.io へ publish（任意、準備が整ったとき）

`0.0.x` は **自動 publish しない**。publish したいときに：

```sh
cargo publish --dry-run     # まずパッケージレイアウトを確認
cargo publish
```

`release.yml` が green になるまで `cargo publish` しないこと。
crates.io は **取り消し不可** なので、まずバイナリを sanity check
の意味で先に出す。

## 検証

ステップ 5 後、別のクリーンマシンで成果物を取得：

```sh
# GitHub Releases から：
curl -L -o pathlint.tar.gz \
  "https://github.com/ShortArrow/pathlint/releases/download/vX.Y.Z/pathlint-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz"
tar -xzf pathlint.tar.gz
./pathlint-vX.Y.Z-x86_64-unknown-linux-gnu/pathlint --version
```

表示される version が tag と一致すること。

## ロールバック

tag を push する **前** に問題が見つかったら：`main` 上の
`chore: release` コミットを消し、すでに push されていれば force
push（同じ ref を引いてる人と調整しつつ）して、やり直す。

tag は push 済みだが `release.yml` が途中で失敗した、または壊れた
成果物が出来たら：

```sh
# GitHub Release と tag をローカル + リモート両方から消す
gh release delete vX.Y.Z --yes
git push origin :refs/tags/vX.Y.Z
git tag -d vX.Y.Z
```

そのあと `develop` で問題を直し、**X.Y.Z+1** に bump して
（同じ番号は再利用しない — たとえ誰もダウンロードしていなくても、
crates.io と各人の toolchain キャッシュは「上書き」を認識しない）、
プロセスをやり直す。

## チートシート

```sh
# develop 上、X.Y.Z を切る準備が整っている：
git switch main && git pull --ff-only
git merge --no-ff develop -m "Merge branch 'develop' for X.Y.Z"

# Cargo.toml + CHANGELOG.md を編集してから：
cargo build && cargo test
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore: release X.Y.Z"

# develop に forward-merge：
git switch develop && git merge --ff-only main

# tag + push：
git push origin main develop
git tag -a vX.Y.Z -m "pathlint X.Y.Z" && git push origin vX.Y.Z

# 任意、GitHub Release が無事に出てから：
cargo publish --dry-run && cargo publish
```
