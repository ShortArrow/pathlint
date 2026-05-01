# pathlint

[![crates.io](https://img.shields.io/crates/v/pathlint.svg)](https://crates.io/crates/pathlint)
[![CI](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml/badge.svg)](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/pathlint.svg)](#ライセンス)

> 各コマンドが、自分が期待するインストーラから resolve されているかを検証する。

> **⚠ Pre-alpha (0.0.x)。** スキーマと CLI 表面はまだ動きます。
> 0.1.0 が出るまで、minor / patch 双方が schema や CLI の互換を
> 壊しうる前提でお使いください。0.0.2 のバイナリは動作します
> （load-bearing な仕組みに組み込むのはまだ時期尚早）。

---

## なぜ必要か

「PATH 関連の不具合」のほとんどは結局 **間違った実体のコマンドが先に
解決される** ことに帰着します：

- このマシンで `cargo install runex` したのに、走るのは `winget` の
  古いほう。同名・別ファイル。
- `python` は `mise` 由来であってほしい、Microsoft Store の
  `WindowsApps` スタブからではなく。
- `node` は `volta` 由来がいい、システムの `apt` インストールでは
  なく。
- macOS の `gcc` は Homebrew 由来であってほしい、`/usr/bin/gcc` から
  ではなく。

`which python` は何が勝つかを教えてくれますが、それが**勝つべきもの
なのか**を、dotfiles リポにコミットして全マシンでチェックできる形で
は教えてくれません。

`pathlint` がその意図を明示します：「**`runex` は `cargo` から、
`winget` ではなく**」と一度書けば、自分の所有する全マシンで検証
できます。

## 仕組み

TOML の 2 つの概念：

1. **`[[expect]]`** — コマンドごとの期待。「コマンド X は source S
   から解決されるべき」。ユーザーが実際に書くのはこれ。
2. **`[source.<name>]`** — ディスク上のインストーラの見分け方
   （「`cargo` は `~/.cargo/bin` にいる」）。pathlint が `cargo`、
   `mise`、`volta`、`aqua`、`winget`、`choco`、`scoop`、`brew_arm`、
   `brew_intel`、`apt`、`pacman`、`dnf`、`pkg`、`flatpak`、`snap`、
   `WindowsApps` などの組み込みデフォルトを持つ。ユーザーは標準と違う
   レイアウトのときだけ上書きする。

各 `[[expect]]` について、pathlint はコマンドを実 PATH から resolve
し、勝者バイナリの場所を見て、それを source ラベルにマッチさせる。

## ステータス

0.0.x ラインで `pathlint` / `pathlint init` / `pathlint catalog list`
/ `pathlint doctor` が動きます。TOML スキーマと CLI 表面は引き続き
動きますが、解決 / マッチ / レポートの一連は実装済みでテストもあります。
詳細設計は [docs/PRD.jp.md](PRD.jp.md) を参照。

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
  source（`/usr/bin`）には部分一致しません。`[source.usr_sbin]
  linux = "/usr/sbin"` を `pathlint.toml` に足してください。
- **どのパッケージがそのバイナリを所有しているか。** `pathlint` は
  `dpkg -S` / `rpm -qf` / `pacman -Qo` / `brew which-formula` を呼び
  ません。0.0.x では速度とオフライン正しさを優先しての判断で、再考
  は 0.2 議題です。

既知の制約と将来のトレードオフは [docs/PRD.jp.md §14, §16](PRD.jp.md)
にすべて書いてあります。

## 使い方

```sh
# 現在のプロセス PATH を ./pathlint.toml で検証
pathlint                          # = pathlint check

# Windows レジストリの User PATH / Machine PATH を直接チェック
pathlint --target user
pathlint --target machine

# 詳細：n/a の expectation や解決後 PATH も表示
pathlint --verbose

# NG ごとに resolved / matched / prefer / avoid / diagnosis / hint
# を多行表示（0.0.7+）
pathlint check --explain

# starter pathlint.toml をカレントに作る
pathlint init
pathlint init --emit-defaults     # 組み込みカタログ全体も書き出す

# 認識できる source 一覧を表示
pathlint catalog list             # 現在 OS の path のみ
pathlint catalog list --all       # 全 OS のフィールドを縦展開
pathlint catalog list --names-only

# PATH 自体の衛生チェック（重複、不在ディレクトリ、env-var 短縮候補、
# Windows 8.3 短縮、形式破損エントリなど）
pathlint doctor

# コマンドがどこから来たか + uninstall コマンドのヒント
pathlint where lazygit
pathlint where lazygit --json     # 0.0.6+: 機械可読出力

# CI 用に doctor の診断を絞る
pathlint doctor --exclude shortenable,missing
pathlint doctor --include duplicate,malformed
```

## `pathlint.toml`（最小例）

```toml
[[expect]]
command = "runex"
prefer  = ["cargo"]
avoid   = ["winget"]

[[expect]]
command = "python"
prefer  = ["mise"]
avoid   = ["WindowsApps", "choco"]

[[expect]]
command = "node"
prefer  = ["mise", "volta"]

[[expect]]
command = "gcc"
prefer  = ["mingw", "msys"]
avoid   = ["strawberry"]
os      = ["windows"]
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

上の例で参照している source 名はすべて組み込みカタログにあるので、
`[source.*]` セクションは 1 つも書かない。ファイル全体がユーザーの
意図そのもの。

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

`pathlint where <command>` は plugin-aware：解決済みパスが
`mise/installs/<segment>/...` の下にあり、`<segment>` が
`cargo-` / `npm-` / `pipx-` / `go-` / `aqua-` で始まるとき、
出力に `provenance:` 行と `mise uninstall ...` ヒントが追加される
（どのプラグインで入れたか思い出さなくて済む）：

```
$ pathlint where lazygit
lazygit
  resolved: ~/.local/share/mise/installs/cargo-jesseduffield-lazygit/0.61/bin/lazygit
  sources:  mise_installs, mise
  provenance: cargo (via mise plugin `cargo-jesseduffield-lazygit`)
  hint:     mise uninstall cargo:jesseduffield-lazygit  (best-guess; verify with `mise plugins ls`)
```

provenance はパス上の heuristic で source match では**ない**。
`prefer = ["cargo"]` が `mise/installs/cargo-foo/...` のバイナリに
マッチすることはない。source ラベルはカタログ駆動のまま、
provenance は `where` の表示専用。

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

## カタログバージョンを固定する

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

## ドキュメント

- [PRD（日本語）](PRD.jp.md) — 詳細設計（組み込み source カタログ含む）
- [リリース手順（日本語）](RELEASE.jp.md)
- [README（英語）](../README.md)
- [PRD（英語）](PRD.md)
- [リリース手順（英語）](RELEASE.md)
- [Changelog](../CHANGELOG.md)

## ライセンス

以下のいずれかを選択可能なデュアルライセンス：

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE) または <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) または <http://opensource.org/licenses/MIT>)

利用者が選択できます。

### コントリビュート

明示的に別段の指定をしない限り、あなたが本プロジェクトに意図的に提出
したコントリビュートは、Apache-2.0 の定義に従い、追加条項なしで上記の
デュアルライセンスの下で提供されたものとみなされます。
