# pathlint のリリース手順

リリースは `main` ブランチに対して GitHub Actions の `release`
ワークフローを実行することで行う。version bump、tag、build、
GitHub Release はワークフローが行う。crates.io への publish は
opt-in。

## 手順

ワークフロー起動前のチェックリスト：

- `README.md` の schema-pin 例を更新する。`<TAG>` の placeholder
  例が **直前のリリース版** を指すようにする (copy-paste した user
  が known-good URL を得るため、未公開バージョンを指さないため)。
  `README.md` 内で
  `https://github.com/ShortArrow/pathlint/releases/download/` を
  検索。
- (任意、推奨) `scripts/bench.sh` を走らせて hyperfine の表を
  release notes 草案に貼る。host の情報 (CPU モデル、OS) も書いて
  おくと後から数字を比較しやすい。PRD §12 の `<50 ms startup`
  claim の検証手段。
- **英日 parity check。** 以下 3 ペアそれぞれについて、前回
  リリース以降の差分を diff し、両ファイルが同時に更新されたか
  確認する：
    - `README.md` ↔ `docs/README.jp.md`
    - `docs/RELEASE.md` ↔ `docs/RELEASE.jp.md`
    - `docs/PRD.md` ↔ `docs/PRD.jp.md`
  例えば 0.0.14 で導入された `os_baseline_linux_sbin` が JP
  README には反映されないまま 0.0.18 まで残っていた、ような
  drift をこの checklist で防ぐ。

  注: `docs/ARCHITECTURE.md` は意図的に英語のみ。 user feedback
  次第で将来 JP 版を別 release で追加する余地はあるが、 現時点では
  この parity check の対象外。

それから：

1. GitHub のリポジトリで **Actions** → **release** → **Run
   workflow** を開く。
2. 新しいバージョン番号を入力する（例：`0.0.10`）。`v` は付け
   ない。tag には自動で付く。
3. crates.io にも公開するかどうか決める。チェックはデフォルト
   off。Trusted Publishing の設定が済んだら on にする（後述）。
4. **Run workflow** をクリック。

ワークフローは以下を順に行う：

1. `Cargo.toml` を bump し、`Cargo.lock` を更新する。
2. fmt / clippy / test / package を走らせる。
3. `chore: release X.Y.Z` をコミットし、`vX.Y.Z` を tag、
   `main` に push する。
4. Linux / macOS / Windows 向けにクロスビルドする。
5. リリースノートを自動生成して GitHub Release を作る。
6. （指定された場合のみ）crates.io に公開する。

## ブランチと merge ポリシー

長く維持するブランチは `main` 1 本だけ。

- 日常作業は feature ブランチ（`feat/...`、`fix/...` など）で
  行い、PR の squash merge で `main` に入れる。
- PR タイトルは Conventional Commits に従う（`feat:`、`fix:`、
  `refactor:`、`chore:`、`docs:`、`test:`、`ci:` ほか）。squash
  後の commit subject は PR タイトルそのものになり、リリース
  ノートの自動生成はこの行を拾う。
- PR レビューを通らずに `main` に入る唯一の例外は、リリース
  ワークフローの `prepare` ジョブが `github-actions[bot]` と
  して打つ `chore: release X.Y.Z` コミット。

リポ設定の推奨：

- Pull Requests: squash merge のみ許可。squash の subject に PR
  タイトルを使う設定を on に。
- `main` の branch protection: PR + status checks（`ci`、
  `pr-title-check`）必須、linear history 必須、リリースコミット
  のために `github-actions` のみ push を許可。

## バージョン

バージョンが `0.` で始まる間は、minor / patch 双方で TOML schema
や CLI を壊しうる。`0.1.0` 以降は通常の semver に従う。

## crates.io への publish

最初の 1 回は手動で：

```sh
cargo publish
```

そのあと crates.io のクレート設定画面で Trusted Publishing を
設定すれば、`release` ワークフローからも公開できるようになる。
ワークフロー実行時に **Also publish to crates.io** にチェックを
入れる。

## 失敗時の対応

- **prepare が失敗。** 何も push されていない。`main` で直して
  ワークフローを再実行。
- **prepare 成功後 build が失敗。** bump コミットと tag は既に
  `main` に乗っている。fix-forward で次バージョンに進むか、tag
  を消して（`git push origin :refs/tags/vX.Y.Z`）同じバージョン
  で再実行する。
- **publish-github が失敗。** そのジョブだけ再実行する。アー
  ティファクトは build ジョブに残っている。
- **publish-crates が失敗。** crates.io は同じバージョンの再
  公開を受け付けないので、次のバージョンに上げる必要がある。

リリース全体を取り消す場合：

```sh
git switch main
git pull --ff-only
git reset --hard HEAD~1
git push --force-with-lease origin main
git push origin :refs/tags/vX.Y.Z
```

## 手動 fallback

ワークフロー自体が壊れているとき：

```sh
./scripts/release-check.sh X.Y.Z   # ローカルで fmt/clippy/test/package
cargo install cargo-edit --locked  # `cargo set-version` を提供
cargo set-version X.Y.Z
git commit -am "chore: release X.Y.Z"
git tag -a vX.Y.Z -m "pathlint X.Y.Z"
git push origin main vX.Y.Z
gh release create vX.Y.Z --generate-notes ...
cargo publish      # crates.io にも出す場合
```
