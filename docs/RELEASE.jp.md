# pathlint のリリース手順

🌐 [English](RELEASE.md) | **日本語**

0.0.36 以降のリリースは、`main` ブランチに **`vX.Y.Z` 形式の
tag を push する** ことで実行する。 tag push が workflow を
trigger し、 ビルド、 schema 生成、 GitHub Release 作成、
crates.io への publish までが自動で走る。 Actions タブの dispatch
は使わず、 version 入力もない。 tag そのものがリリース指示。
この形は以前の dispatch 駆動 workflow を置き換えたもの — CI が
tag を管理する旧 flow は partial release を同一 version で安全に
retry できなかった。 人間が push する tag を唯一の trigger に
することで、 その recovery 経路ごと不要にした。

## 手順

### 1. 事前チェックリスト

- `README.md` の schema-pin 例を更新する。 `<TAG>` の placeholder
  例が **直前のリリース版** を指すようにする (copy-paste した user
  が known-good URL を得るため、未公開バージョンを指さないため)。
  `README.md` 内で
  `https://github.com/ShortArrow/pathlint/releases/download/` を
  検索。
- (任意、 推奨) `scripts/bench.sh` を走らせて hyperfine の表を
  release notes 草案に貼る。 host の情報 (CPU モデル、 OS) も書いて
  おくと後から数字を比較しやすい。 PRD §12 の `<50 ms startup`
  claim の検証手段。
- (release が `doctor` selfcheck / `lint` 検出器 / 組み込み catalog
  / `/etc/os-release` 読み取り / `expand_env` のどれかに触る場合)
  `scripts/e2e/run.sh` を走らせて Ubuntu / Arch / Fedora container
  で smoke する。 意図的に local-only — CI runner 上の container
  は遅く flaky で、 この harness が gate するのは merge ではなく
  release だから。
- **英日 parity check。** 以下 3 ペアそれぞれについて、 前回
  リリース以降の差分を diff し、 両ファイルが同時に更新されたか
  確認する：
    - `README.md` ↔ `docs/README.jp.md`
    - `docs/RELEASE.md` ↔ `docs/RELEASE.jp.md`
    - `docs/PRD.md` ↔ `docs/PRD.jp.md`
  例えば 0.0.14 で導入された `os_baseline_linux_sbin` が JP
  README には反映されないまま 0.0.18 まで残っていた、 ような
  drift をこの checklist で防ぐ。

  注: `docs/ARCHITECTURE.md` と `CHANGELOG.md` は意図的に英語のみ。
  user feedback 次第で将来 JP 版を別 release で追加する余地は
  あるが、 現時点ではこの parity check の対象外。

### 2. Bump PR

以下を行う PR を 1 本立てる：

- `Cargo.toml` を新バージョンに bump (例: `version = "0.0.36"`)、
  `Cargo.lock` も更新。
- `CHANGELOG.md` に `[X.Y.Z]` entry を追加。
- ユーザ向け変更を含むなら `docs/PRD.md` / `docs/README.jp.md` /
  `docs/PRD.jp.md` も同 PR で更新。

squash merge する。 このリリースで crates.io publish を **skip**
したい場合は、 squash 後の commit message に `[skip publish]`
(半角スペース 1 つ、 角括弧、 小文字、 exact spelling) を含める。
デフォルトは publish する。

```text
chore: release 0.0.36

リリースノート本文。

[skip publish]   ← crates.io を skip したい release だけこの行を入れる
```

### 3. main 上で tag を切って push

Bump PR が merge されたら、 きれいな `main` で：

```sh
git switch main
git pull --ff-only origin main
git tag -a vX.Y.Z -m "pathlint X.Y.Z"
git push origin vX.Y.Z
```

`git push origin vX.Y.Z` が workflow を trigger する。 workflow
は以下を順に実行：

1. tag 先の commit の `Cargo.toml` version が `X.Y.Z` と一致する
   ことを check。 不一致なら即 fail。
2. Linux / macOS / Windows 向けにクロスビルド。
3. tag された commit から JSON schema を再生成。
4. リリースノートを自動生成して GitHub Release を作成。
5. (tag commit の message に `[skip publish]` が含まれない限り)
   Trusted Publishing 経由で OIDC token を交換し `cargo publish`
   を実行。

### tag-on-`main` ルール

tag は `main` でのみ切る。 workflow 側での enforcement は今は
していない (runex も同じ運用、 かつ pathlint は releaser が 1 人
なので)。 規律で守る。 hotfix branch からリリースしたい場合は、
先に hotfix を `main` に merge する。

### `[skip publish]` token

正確な spelling は `[skip publish]` — 角括弧、 小文字、 `skip`
と `publish` の間は半角スペース 1 つ。 `[skip-publish]` /
`[skip_publish]` / `[ skip publish ]` といった変形は workflow の
標準行 check に **match しない** ので、 結果的に crates.io に
publish される。 skip させたい場合は bump PR の squash commit
message を merge 前に再確認する。

check は token が **単独の行として** 現れる場合のみ skip と判定
する (前後の whitespace は trim される)。 文中での言及 (例:
「bump commit に `[skip publish]` を書く」) は **count しない** —
これは意図的で、 CHANGELOG entry や PR description で token を
言及しても 本物の publish を 誤って 止めない ため。 0.0.36 は
ちょうど この pitfall を踏んだ (release commit body が token を
prose の中で説明していて、 publish が skip された)。 0.0.37 で
standalone-line gate を追加した。

skip したい commit body の例：

```text
chore: release 0.0.40

リリースノート本文。

[skip publish]
```

`[skip publish]` が単独行にある — skip される。

publish したいが token を prose で言及したい commit body の例：

```text
chore: release 0.0.40

リリースノート本文。`[skip publish]` を release commit に追加
すると crates.io publish が抑制される。
```

token が文の一部なので standalone-line check に match せず、
crates.io publish が走る。

## ブランチと merge ポリシー

長く維持するブランチは `main` 1 本だけ。

- 日常作業は feature ブランチ (`feat/...`、 `fix/...` など) で
  行い、 PR の squash merge で `main` に入れる。
- PR タイトルは Conventional Commits に従う (`feat:`、 `fix:`、
  `refactor:`、 `chore:`、 `docs:`、 `test:`、 `ci:` ほか)。 squash
  後の commit subject は PR タイトルそのものになり、 リリース
  ノートの自動生成はこの行を拾う。
- PR レビューを通らずに `main` に入る commit は **ない**。 0.0.36
  以降は workflow が `main` に push しない (version bump は通常の
  PR に同梱)。

リポ設定の推奨 (現状値もこの通り)：

- Pull Requests: squash merge のみ許可。 **squash commit subject =
  PR title** (API: `squash_merge_commit_title=PR_TITLE`、 つまり
  `COMMIT_OR_PR_TITLE` ではない。 後者は PR が 1 commit だけのとき
  個別 commit の subject を採用するため、 PR title を edit しても
  古い / scope 違反の subject が main に滑り込みうる。 0.0.37 の
  `fix(release):` slip がそれ。 0.0.38 で `PR_TITLE` に flip した)。
- Squash commit body は squash 対象 commit の message を結合
  (`squash_merge_commit_message=COMMIT_MESSAGES`)。 commit body
  はそのまま残る。
- `main` の branch protection: PR + status checks (`ci`、
  `pr-title-check`) 必須、 linear history 必須。 旧 release flow
  で必要だった `github-actions[bot]` の push 例外は不要。

## バージョン

バージョンが `0.` で始まる間は、 minor / patch 双方で TOML schema
や CLI を壊しうる。 `0.1.0` 以降は通常の semver に従う。

## crates.io への publish

最初の 1 回は手動で：

```sh
cargo publish
```

そのあと crates.io のクレート設定画面で Trusted Publishing を
設定済み。 0.0.36 以降は tag push のたびに デフォルトで publish
される。 特定リリースで skip したい場合は bump 時の commit
message に `[skip publish]` を含める。

## 失敗時の対応

新 flow は、 旧 `workflow_dispatch` flow が必要としていた
partial-release recovery 経路を構造的に取り除いた。 同じ tag を
再 push しても `git push` が non-fast-forward で reject、 仮に
tag を force-delete して再 push しても crates.io が同一 version
の publish を reject する。

**recovery は次の patch release を切ること**。 これは 0.0.34 →
0.0.35 で pathlint が苦労して学んだ規律と同じ：途中で release が
失敗したら、 同じ version で retry せず、 次の patch を bump して
新しく release を切る。

具体的な失敗モード：

- **Version mismatch guard が fail**: tag 先の `Cargo.toml` が
  別 version。 tag を削除 (`git push origin :refs/tags/vX.Y.Z`)、
  `Cargo.toml` を正しく bump し直す PR を立て、 再度 tag を切る。
- **特定 OS の build matrix が fail**: 一時的なら Actions タブで
  そのジョブだけ再実行する。 本質的な失敗なら fix forward で
  次 patch release を切る。
- **publish-github が fail**: そのジョブだけ再実行する。 build
  artifact は build job に残っており、 tag は変わらない。
- **publish-crates が fail**: crates.io は同 version の再 publish
  を受け付けない。 fix forward で次 patch release を切る。 再 tag
  しない。

リリース全体を取り消したい場合：

```sh
git switch main
git pull --ff-only
git push origin :refs/tags/vX.Y.Z
gh release delete vX.Y.Z --yes
```

Bump PR の commit は `main` に残る。 それを消すには `main` への
force push が必要で、 branch protection で拒否される想定。 代わり
に bump を revert する follow-up PR を立てる。
