# pathlint

[![crates.io](https://img.shields.io/crates/v/pathlint.svg)](https://crates.io/crates/pathlint)
[![CI](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml/badge.svg)](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/pathlint.svg)](#ライセンス)

> 各コマンドが、自分が期待するインストーラから resolve されているかを検証する。

> **⚠ Pre-alpha (0.0.x)。** スキーマと CLI 表面はまだ動きます。
> 0.1.0 が出るまで、minor / patch 双方が schema や CLI の互換を
> 壊しうる前提でお使いください。現行の 0.0.x バイナリは動作します
> （load-bearing な仕組みに組み込むのはまだ時期尚早）。

---

## 何をするツールか

「PATH 関連の不具合」のほとんどは結局 **間違った実体のコマンドが先に
解決される** ことに帰着します。`which python` は何が勝つかを教えて
くれますが、それが**勝つべきものなのか**を、dotfiles リポにコミット
して全マシンでチェックできる形では教えてくれません。

`pathlint` がその意図を明示します。「**`runex` は `cargo` から、
`winget` ではなく**」と一度 `pathlint.toml` に書けば、自分の所有
する全マシンで検証できます。

## インストール

```sh
# crates.io から
cargo install pathlint

# ソースから（最新 main）
cargo install --git https://github.com/ShortArrow/pathlint

# ビルド済みバイナリ
# https://github.com/ShortArrow/pathlint/releases
# Linux x86_64 / Windows x86_64 / macOS x86_64 / macOS aarch64
```

## 60 秒で試す

```sh
# starter pathlint.toml をカレントに作る
pathlint init

# pathlint.toml を編集して、自分が気にするツール 1 件の [[expect]]
# を書く。例: 「rg は cargo 由来であってほしい、winget からは嫌」:
#
#   [[expect]]
#   command = "rg"
#   prefer  = ["cargo"]
#   avoid   = ["winget"]

# チェック実行
pathlint                          # = pathlint check

# 失敗したら理由を聞く
pathlint check --explain
```

これがループ。`[[expect]]` がユーザー視点の概念で、それ以外は
すべて補助的な仕組み。

## `pathlint.toml`（最小例）

```toml
[[expect]]
command = "runex"
prefer  = ["cargo"]
avoid   = ["winget"]

[[expect]]
command = "python"
prefer  = ["mise"]
avoid   = ["windows_apps", "choco"]

[[expect]]
command = "node"
prefer  = ["mise", "volta"]

[[expect]]
command = "gcc"
prefer  = ["mingw", "msys"]
avoid   = ["strawberry"]
os      = ["windows"]
```

上の例で参照している source 名はすべて組み込みカタログにあるので、
`[source.*]` セクションは 1 つも書かない (`cargo`、`mise`、`volta`、
`aqua`、`winget`、`choco`、`scoop`、`brew_arm`、`brew_intel`、
`apt`、`pacman`、`dnf`、`pkg`、`flatpak`、`snap`、`windows_apps`
等)。ファイル全体がユーザーの意図そのもの。

組み込みを上書きしたいとき（例：mise を非標準パスに置いている）：

```toml
[source.mise]
windows = "D:/tools/mise"
```

新しい source を追加したいとき：

```toml
[source.my_dotfiles_bin]
unix = "$HOME/dotfiles/bin"
```

`os = [...]` は `windows | macos | linux | termux | unix` を受け付け
る。マッチは部分一致 + 大文字小文字無視、環境変数展開
（`%VAR%` も `$VAR` もどの OS でも） + slash 正規化のあとで評価。

`severity = "warn"` をルールに付ければ、NG を表示するが exit 0
を保つ（CI を止めずに気付きだけ得る用途、0.0.7+）：

```toml
[[expect]]
command  = "rg"
prefer   = ["cargo"]
severity = "warn"   # 軽い警告、強制ではない
```

`kind = "executable"` を足せば、resolve したパスが実際に実行可能
ファイルかも検証する — 同名のディレクトリがバイナリを覆い隠した
場合や、symlink の先が消えた場合などを捕まえる：

```toml
[[expect]]
command = "rustc"
prefer  = ["cargo"]
kind    = "executable"
```

## 次に試すもの

- `pathlint check --explain` — expectation が失敗したときに
  resolved / matched / prefer / avoid / diagnosis / hint を多行表示。
- `pathlint check --json` — CI パイプライン向け機械可読出力。
- `pathlint doctor` — PATH 自体の衛生チェック（重複、不在
  ディレクトリ、8.3 短縮、env-var 短縮候補、形式破損エントリ、
  同名コマンドが別 dir に実体を持つ shadow）。 `[[expect]]` から
  独立。
- `pathlint trace <command>` — コマンドの解決元、マッチ source、
  最も妥当な uninstall コマンドを表示。mise については plugin-aware
  ([mise を使うとき](#mise-を使うとき) 参照)。
- `pathlint sort --dry-run` — 全 `[[expect]]` を満たす PATH 順を
  提案。読み取り専用 (PRD §4 で PATH 書き換え禁止)。
- `pathlint catalog list` — 認識できる source 一覧。`--names-only`
  で名前だけ、`--all` で全 OS フィールド表示。
- `pathlint catalog relations` — source 間の宣言関係 (alias /
  conflict / served-by-via / depends-on / prefer-order-over)。
- `pathlint --target user` / `--target machine` — Windows のみ。
  プロセス env ではなくレジストリの per-user / per-machine PATH を
  読む。他 subcommand も同じ flag。
- `pathlint check --json | jq '.[] | select(.kind != "ok")'` の
  ような機械的パイプライン — 全 JSON 出力 subcommand に安定した
  `kind` discriminator (0.0.15+)。

詳細設計と背景は [docs/PRD.jp.md](PRD.jp.md) (日本語版)。

## 仕組み

TOML の 2 つの概念：

1. **`[[expect]]`** — コマンドごとの期待。「コマンド X は source S
   から解決されるべき」。ユーザーが実際に書くのはこれ。
2. **`[source.<name>]`** — ディスク上のインストーラの見分け方
   （「`cargo` は `~/.cargo/bin` にいる」）。pathlint がよく使われる
   インストーラ全般について組み込みデフォルトを持つ。ユーザーは標準
   と違うレイアウトのときだけ上書きする。

各 `[[expect]]` について、pathlint はコマンドを実 PATH から resolve
し、勝者バイナリの場所を見て、それを source ラベルにマッチさせる。

## pathlint が **教えてくれない** こと

`pathlint` は **パスの prefix ベース**のツールです：コマンドを resolve
して、勝者バイナリのフルパスを見て、定義済み source の OS ごとのパスが
**substring として含まれているか**だけを判定します。これによって速く
（パッケージマネージャ呼び出しなし、ネット不要）動きますが、知って
おくべき盲点があります：

- **AUR / Homebrew tap / `make install` / 任意の prefix。** 定義済みの
  `[source.<name>]` のいずれにも含まれない場所に install されたバイナリ
  は、たとえ正規 install であっても `NG (unknown source)` と報告され
  ます。`[source.my_prefix]` を追加するか、pathlint がその違いを区別
  できないことを受け入れてください。
- **シンボリックリンクされたシステムディレクトリ。** Arch / openSUSE
  TW / Solus などでは `/usr/sbin → /usr/bin` です。`which ls` は
  `/usr/sbin/ls` を返すので、組み込みの `apt` / `pacman` / `dnf`
  source（`/usr/bin`）には部分一致しません。組み込みの
  `os_baseline_linux_sbin` source を `prefer` に並べてください：

  ```toml
  [[expect]]
  command = "ls"
  prefer = ["pacman", "os_baseline_linux_sbin"]
  ```
- **どのパッケージがそのバイナリを所有しているか。** `pathlint` は
  `dpkg -S` / `rpm -qf` / `pacman -Qo` / `brew which-formula` を呼び
  ません。0.0.x では速度とオフライン正しさを優先しての判断で、再考
  は 0.2 議題です。

既知の制約と将来のトレードオフは [docs/PRD.jp.md §14, §16](PRD.jp.md)
にすべて書いてあります。

## mise を使うとき

mise はバイナリを 2 つの異なる場所から提供するので、pathlint は
それぞれを別ソースとして提供している。ルールを精密に書ける：

- **`mise_shims`** — Unix で `$HOME/.local/share/mise/shims/<bin>`、
  Windows で `$LocalAppData/mise/shims/<bin>`。`mise activate` が
  シェルから PATH 先頭に付ける層。多くのルールではこちらを
  `prefer` に書くのが推奨。
- **`mise_installs`** — `$HOME/.local/share/mise/installs/<tool>/<ver>/bin/<bin>`。
  `mise activate` が PATH 書き換え方式（shim ではない）で動くとき
  にここがマッチ。プラグイン (`cargo-*`、`npm-*`...) が
  `installs/<plugin>/<ver>/bin` 配下にバイナリを置く場合も同様。
- **`mise`** — 両者をまとめて引っかけるキャッチオール。「mise が
  どのモードかは気にしない」ルール向け。0.0.3 以前に書かれた
  ルールはこのまま動く。

```toml
# 厳しめ: mise の shim 層からだけ来てほしい
[[expect]]
command = "python"
prefer  = ["mise_shims"]

# 緩め: mise が出すものなら何でも OK
[[expect]]
command = "node"
prefer  = ["mise"]
```

`pathlint trace <command>` は plugin-aware：解決済みパスが
`mise/installs/<segment>/...` の下にあり、`<segment>` が
`cargo-` / `npm-` / `pipx-` / `go-` / `aqua-` で始まるとき、
出力に `provenance:` 行と `mise uninstall ...` ヒントが追加される
（どのプラグインで入れたか思い出さなくて済む）：

```
$ pathlint trace lazygit
lazygit
  resolved: ~/.local/share/mise/installs/cargo-jesseduffield-lazygit/0.61/bin/lazygit
  sources:  mise_installs, mise
  provenance: cargo (via mise plugin `cargo-jesseduffield-lazygit`)
  hint:     mise uninstall cargo:jesseduffield-lazygit  (best-guess; verify with `mise plugins ls`)
```

provenance はパス上の heuristic で source match では**ない**。
`prefer = ["cargo"]` が `mise/installs/cargo-foo/...` のバイナリに
マッチすることはない。source ラベルはカタログ駆動のまま、
provenance は `trace` の表示専用。

`MISE_DATA_DIR` や `XDG_DATA_HOME` で mise を非標準パスに置いて
いる場合は、3 つのソースをまとめて `pathlint.toml` で上書きする：

```toml
[source.mise]
unix = "/data/tools/mise"

[source.mise_shims]
unix = "/data/tools/mise/shims"

[source.mise_installs]
unix = "/data/tools/mise/installs"
```

## 運用情報

0.0.x ラインで 6 サブコマンドが動きます: `check`（デフォルト）/
`doctor` / `trace` / `sort` / `init` / `catalog`（`list` と
`relations`）。`pathlint where` は `pathlint trace` の visible
alias、`--rules` は `--config` の visible alias として 0.0.x
線では残します。両 alias は将来のリリースで削除予定（時期未定、
破壊リリース前に告知）。TOML スキーマと CLI 表面は引き続き動き
ますが、解決 / マッチ / レポートの一連は実装済みでテストもあり
ます。

`pathlint --version` は modern host で 50 ms を切ります。自分の
ハードウェアで検証するには `scripts/bench.sh` (`hyperfine` で
`--version` / `--help` / `catalog list --names-only` を計測) を
使ってください。

### エディタ対応 (JSON Schema)

`pathlint.toml` は実装の Rust 型から生成された JSON Schema を
配布しています。設定ファイルの先頭に 1 行加えるだけで、
[Taplo] (TOML LSP の主流) と [Even Better TOML][ebt] VS Code
拡張で補完とインライン検証が効きます：

```toml
#:schema https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json
```

特定リリースに固定したい場合は (`<TAG>` を使いたいバージョン
に置換、例: `v0.0.21`)：

```toml
#:schema https://github.com/ShortArrow/pathlint/releases/download/<TAG>/pathlint.schema.json
```

schema は GitHub Release ごとに `pathlint.schema.json` として
バイナリと並んで配布。0.0.15 以降は `check.schema.json`
(`pathlint check --json` 出力 shape) も同梱しています。

[Taplo]: https://taplo.tamasfe.dev/
[ebt]: https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml

### カタログバージョンを固定する

組み込みソースカタログは進化する。新しい pathlint がソースの
OS 別パスを変更することもある（例：winget がレイアウトを変えた）。
自分の `pathlint.toml` が十分に新しいカタログで実行されていることを
保証したいなら、最低バージョンを書く：

```toml
require_catalog = 1
```

実行中のバイナリが古いカタログを埋め込んでいたら、pathlint は
exit 2 とバージョン差を案内するメッセージで止まる。古いルールに
黙ってマッチさせ続けるのを防げる。`pathlint catalog list` の
1 行目に組み込みバージョンが出るので、それを参考に値を決める。

逆方向（新しすぎるカタログ）は強制されない。`catalog_version` の
bump は path / 意味の変更があったときに限られ、新規 source 追加
では bump しないので、古いルールが壊れることはない。

## ドキュメント

- [PRD（日本語）](PRD.jp.md) — 詳細設計（組み込み source カタログ含む）
- [リリース手順（日本語）](RELEASE.jp.md)
- [README（英語）](../README.md)
- [PRD（英語）](PRD.md)
- [Architecture（英語）](ARCHITECTURE.md) — 5 分で把握する repo map
- [リリース手順（英語）](RELEASE.md)
- [Changelog（英語）](../CHANGELOG.md) — Keep a Changelog 形式、 breaking change の migration note 入り (英語のみ)
- [Releases](https://github.com/ShortArrow/pathlint/releases) — 自動生成のリリースノート付き履歴

## ライセンス

以下のいずれかを選択可能なデュアルライセンス：

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) または <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) または <http://opensource.org/licenses/MIT>)

利用者が選択できます。

### コントリビュート

明示的に別段の指定をしない限り、あなたが本プロジェクトに意図的に提出
したコントリビュートは、Apache-2.0 の定義に従い、追加条項なしで上記の
デュアルライセンスの下で提供されたものとみなされます。
