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
解決される** ことに帰着します。例：

- Microsoft Store 経由の `python.exe` スタブが mise でインストールした
  `python` を覆い隠す。
- Strawberry Perl の `gcc` が、本来使いたいツールチェインを覆い隠す。
- 残骸の `WindowsPowerShell\v1.0` エントリが PowerShell 7 ではなく
  古い `pwsh.exe` を走らせる。

`which python` は何が勝つかを教えてくれますが、それが**勝つべきもの
なのか**は教えてくれません。`pathlint` はその意図を明示します：
「**A は B より先に来るべき**」のルールを TOML に書き、実 PATH に対して
チェックします。

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
[[rule]]
name   = "mise shims override system tools"
before = "mise\\shims"
after  = ["chocolatey\\bin", "Strawberry\\c\\bin"]

[[rule]]
name   = "PowerShell 7 precedes legacy WindowsPowerShell"
before = "PowerShell\\7"
after  = ["WindowsPowerShell\\v1.0"]
```

マッチは部分一致 + 大文字小文字無視、環境変数展開後に評価されます。

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
