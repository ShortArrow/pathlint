# pathlint

[![crates.io](https://img.shields.io/crates/v/pathlint.svg)](https://crates.io/crates/pathlint)
[![CI](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml/badge.svg)](https://github.com/ShortArrow/pathlint/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/pathlint.svg)](#ライセンス)

> 宣言的な順序ルールに照らして `PATH` 環境変数を lint する。

> **⚠ Pre-alpha (0.0.x)。** スキーマと CLI の表面はまだ動きます。
> 本番に組み込むには時期尚早です。現状は **スケルトンのみ** で、動くバイナリは
> まだありません。

---

## なぜ必要か

「PATH 関連の不具合」のほとんどは結局 **間違った実体のコマンドが先に
解決される** ことに帰着します。OS ごとに見た目は違いますが本質は同じ：

- **Windows.** Microsoft Store の `python.exe` スタブが mise / conda
  / 手動インストールを覆う。Strawberry Perl の `gcc` が MSYS / Rust
  のツールチェインを覆う。
- **macOS.** `/usr/bin/python3` が Homebrew や pyenv を覆う。Intel の
  `/usr/local/bin` と arm の `/opt/homebrew/bin` の順序が問題に。
- **Linux.** distro の `node` が nvm / mise を覆う。`/snap/bin` が
  `~/.cargo/bin` を覆う。`/usr/games` が `~/bin` より先でローカル
  スクリプトを覆う。
- **Termux.** `~/bin` が `$PREFIX/bin` の後ろに来ると、ユーザー作成
  スクリプトで `pkg install` 提供のツールを上書きできない。

`which python` は何が勝つかを教えてくれますが、それが**勝つべきもの
なのか**は教えてくれません。`pathlint` はその意図を明示します：
「**A は B より先に来るべき**」のルールを TOML に書き、各ルールに
適用 OS タグを付け、実 PATH に対してチェックします。

## ステータス

このクレートには現状プロジェクトのスケルトンしかありません（Cargo
manifest、ライセンス、ドキュメント）。実装は PowerShell プロトタイプ
<https://github.com/ShortArrow/dotfiles/blob/develop/windows/Test-PathOrder.ps1>
から Rust に移植中です。スコープは [docs/PRD.jp.md](PRD.jp.md) を参照。

## 想定する使い方

```sh
# 現在のプロセス PATH を ./pathlint.toml で検証
pathlint check

# Windows レジストリの User PATH / Machine PATH を直接チェック
pathlint check --target user
pathlint check --target machine

# あるコマンドがどこから resolve されるか、覆われている同名 exe を含めて表示
pathlint which python

# (planned) 全ルールを満たすように PATH を並べ替えた案を提示
pathlint sort --target user --dry-run
```

## 想定する `pathlint.toml` スキーマ

```toml
# タグ無し: 全 OS で適用。
[[rule]]
name   = "PowerShell 7 precedes legacy WindowsPowerShell"
before = "PowerShell\\7"
after  = ["WindowsPowerShell\\v1.0"]

# Windows 専用。
[[rule]]
name   = "mise shims override system tools"
os     = ["windows"]
before = "mise\\shims"
after  = ["chocolatey\\bin", "Strawberry\\c\\bin"]

# 複数 OS（Termux は除外）。
[[rule]]
name   = "user cargo bin precedes distro tools"
os     = ["windows", "macos", "linux"]
before = ".cargo/bin"
after  = ["/usr/bin", "Strawberry"]

# Termux 専用。
[[rule]]
name   = "user bin precedes pkg-installed binaries"
os     = ["termux"]
before = "/data/data/com.termux/files/home/bin"
after  = ["/data/data/com.termux/files/usr/bin"]
```

マッチは部分一致 + 大文字小文字無視、環境変数展開後に評価されます。
slash と backslash（`/` と `\`）は正規化されるので、同じルールが OS
横断で動きます。`os` は `windows | macos | linux | termux | unix` を
受け付けます。

## インストール

```sh
# crates.io から（公開後）
cargo install pathlint

# ソースから（最新 main）
cargo install --git https://github.com/ShortArrow/pathlint
```

## ドキュメント

- [PRD（日本語）](PRD.jp.md)
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
