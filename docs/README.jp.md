# pathlint

[![crates.io](https://img.shields.io/crates/v/pathlint.svg)](https://crates.io/crates/pathlint)
[![CI](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml/badge.svg)](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/pathlint.svg)](#ライセンス)

> 各コマンドが、自分が期待するインストーラから resolve されているかを検証する。

> **⚠ Pre-alpha (0.0.x)。** スキーマと CLI の表面はまだ動きます。
> 本番に組み込むには時期尚早です。現状は **スケルトンのみ** で、動く
> バイナリはまだありません。

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

このクレートには現状プロジェクトのスケルトンしかありません（Cargo
manifest、ライセンス、ドキュメント）。実装は PowerShell プロトタイプ
<https://github.com/ShortArrow/dotfiles/blob/develop/windows/Test-PathOrder.ps1>
から Rust に移植中です。詳細設計は [docs/PRD.jp.md](PRD.jp.md) を参照。

## 想定する使い方

```sh
# 現在のプロセス PATH を ./pathlint.toml で検証
pathlint                          # = pathlint check

# Windows レジストリの User PATH / Machine PATH を直接チェック
pathlint --target user
pathlint --target machine

# 詳細：n/a の expectation や解決後 PATH も表示
pathlint --verbose
```

## 想定する `pathlint.toml`（最小例）

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

## インストール

```sh
# crates.io から（公開後）
cargo install pathlint

# ソースから（最新 main）
cargo install --git https://github.com/ShortArrow/pathlint
```

## ドキュメント

- [PRD（日本語）](PRD.jp.md) — 詳細設計（組み込み source カタログ含む）
- [README（英語）](../README.md)
- [PRD（英語）](PRD.md)
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
