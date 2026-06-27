# pathlint — プロダクト原則

🌐 [English](PRINCIPLES.md) | **日本語**

以下の 7 つの設計原則が、pathlint が ship するすべての CLI フラグ、
detector の種類、catalog エントリの形を決める。原則は 4 つの役割
（R1 解決順、R2 存在と形状、R3 PATH 衛生 + selfcheck、R4 出自 —
役割定義は [PRD §1](PRD.jp.md#1-概要) 参照）を横断する。提案された
機能がこの原則のどれかと衝突するなら、原則が勝つ。機能は形を変えるか、
延期されるか、却下される。

これらは *不変の* プロダクト原則。0.0.x の最初から効力を持ち、
0.0.x → 0.1.0 の graduation を越えても変わらない。原則を裏付ける
scope の境界 — pathlint が把握するもの、明示的にモデル化しないもの —
は [ADR-0032](decisions/0032-scope-os-knowledge-tool-meta-declaration.md)
に記録されている。

## 原則

1. **宣言的。** pathlint が気にすることはすべて、dotfiles リポに置ける
   `pathlint.toml` で表現できる。実行時フラグだけに隠れる挙動はない。

2. **パスではなく source ラベル。** ユーザーはインストーラ名
   （`cargo`、`mise_shims`、`winget`、`brew_arm`、`apt`）で書く。
   パスパターンはカタログから引かれるので同じ TOML が全マシンで動く。

3. **組み込みカタログ + 上書き。** pathlint がよく使われるインストーラ
   のデフォルトを内蔵。ユーザーは上書きしたい / 新規追加したいときだけ
   `[source.X]` を書く。

4. **1 ファイル、全 OS。** 各 `[[expect]]` に `os = [...]` フィルタ、
   各 `[source.X]` に OS 別パス。同じ `pathlint.toml` が Windows /
   macOS / Linux / Termux を回す。

5. **部分一致 + 大文字小文字無視。** 環境変数展開と slash 正規化の
   あとで、source パスを解決済みパスに対し substring 比較。

6. **正直な exit code。** `0` = クリーン、`1` = 1 つ以上失敗、`2` =
   config / I/O エラー。R3（doctor）と R4（where）も同じスケール。

7. **読み取り専用。** PATH、レジストリ、dotfiles、インストール済み
   パッケージ、いずれも書き換えない。何があるかを伝えるのみ、行動は
   ユーザーが取る。

## 他ドキュメントとの関係

- **PRD §3** は PRD 内における原則の正典記載。本ファイルはそれを
  そのまま抜き出した複製で、contributor が「PRD §3」と書く代わりに
  短い独立ドキュメントを引用できるようにするためのもの。
- **PRD §4（非ゴール）** が *原則の外側* を記録する — パッケージ
  マネージャ問い合わせ、インストール予測、PATH 書き換え、「document
  モデルがないので LSP サーバは持たない」 line。提案された機能が
  原則に合うか判断するときは §4 と本ファイルを並べて読む。
- **ADR-0009**（読み取り専用 stance）が原則 7 の load-bearing ADR。
  ADR-0014 / 0015 / 0023 / 0031 はそれぞれ原則 2-3（catalog が拡張面）
  の一部を anchor する。ADR-0032 が境界全体を 1 本にまとめる。

PR review、issue triage、ADR で原則を引用するときは、本ファイルの
番号で link する。原則は release 間で安定。PRD の section 番号は
将来動くかも知れない。
