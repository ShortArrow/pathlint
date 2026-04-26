# pathlint — プロダクト要件定義書（PRD）

**ステータス:** ドラフト（実装前）。
**対象リリース:** 0.0.1 MVP。

---

## 1. 概要

`pathlint` は、TOML で書かれた **宣言的な順序ルールに照らして `PATH`
環境変数を検証する** CLI。次の問いに答える：

> 「このコマンドは、本来勝つべき実体から resolve されているか？」

ルールは「X は PATH の中で Y より先に来なければならない」という形。
各ルールについて **OK / NG / skip**（skip = 双方とも PATH に存在しな
い）を表示し、失敗時には該当エントリの index を含めて理由を示す。

MVP は読み取り専用（lint モード）。後続バージョンで並べ替え案の提示・
適用（sort / fix モード）を予定。

## 2. 課題定義

PATH の順序バグはありがちで、静かに痛い：

- `python.exe` の Microsoft Store スタブが本物のインストール（mise、
  conda、asdf、手動など）を覆い隠す。
- Strawberry Perl の `gcc` が Rust／MSYS のツールチェインを覆い隠す。
- 取り残された `WindowsPowerShell\v1.0` のエントリが PowerShell 7 で
  はなく `pwsh.exe` を resolve する。
- `cargo install` で配置された `%UserProfile%\.cargo\bin` のバイナリが、
  Machine PATH 側にある同名ツールに負ける。

`which X` は何が勝つかは見せてくれるが、何が勝つべきかは見せない。
「勝つべきもの」を宣言的に書く手段が存在しなければ、CI / dotfiles /
doctor スクリプトから検証できない。

`pathlint` がそのギャップを埋める。

## 3. ゴール

- **宣言的なルール。** `pathlint.toml` に `[[rule]]` エントリで「X は
  Y より先」を素朴な TOML で書ける。
- **部分一致 + 大文字小文字無視。** ルールキーは厳密なパスでなくて
  良い：`"mise\\shims"` は環境変数展開後に同文字列を含む全エントリに
  マッチ。マシンによって `%UserProfile%` が違っても同じルールが効く
  ようにするため意図的にこうしている。
- **OS を意識した参照元。** `--target process|user|machine` で読み取る
  PATH を切り替え。Windows では `user` / `machine` はレジストリから読
  む。Linux では `process` のみが意味を持つ。
- **正直な exit code。** `0` = 全ルール OK もしくは skip、`1` = 1 つでも
  失敗。
- **役に立つ失敗出力。** 失敗ルールごとに、衝突エントリの index と短い
  理由を表示（例：`'chocolatey\\bin' at #42 precedes 'mise\\shims' at #49`）。
- **MVP では非変更。** 読み取りのみ。`--apply` / `sort` は後回し。

## 4. 非ゴール（MVP）

- **PATH の書き換え／永続化はしない。** sort/fix は後回し。
- **PATH 全体の整形出力はしない。** 失敗の文脈付けに必要な範囲だけ。
- **シェル補完のインストールはしない。** `pathlint completions <shell>`
  は将来追加するかも。
- **パッケージ管理はしない。** ルールを満たすために不足ツールを入れる
  ような副作用は持たない。

## 5. ターゲットユーザー

- 自分の `doctor` ステップで PATH ドリフトを検出したい dotfiles 利用者。
- 「なぜこの Python が動いて、あちらの Python が動かないのか」を
  再起動後にも残る形で追いかけたい開発者。
- 開発環境を bootstrap する CI で、PATH の順序リグレッションを
  はっきり失敗させたい運用者。

## 6. ユーザーストーリー

- 自分が本当に気にする数本のルールだけ `pathlint.toml` に書いて
  dotfiles に commit、`pathlint check` を全マシンで走らせる。
- linter run は全ルールとそのステータスを表示。失敗したらどのエン
  トリがどのエントリに勝っているかが見える。
- `setx PATH ...` する前に `pathlint check --target user` で User
  PATH だけ検証する。
- （MVP 後）`pathlint sort --target user --dry-run` で何が変わるか
  diff として見る。

## 7. 機能要件（MVP）

### 7.1 `pathlint check [--target <process|user|machine>] [--rules <path>]`

- `--target` のデフォルトは `process`（`$env:PATH` / `$PATH` の展開後）。
- `--rules` のデフォルト解決順：
  1. 明示の `--rules <path>`
  2. `./pathlint.toml`
  3. `$XDG_CONFIG_HOME/pathlint/pathlint.toml`（または
     `$HOME/.config/pathlint/pathlint.toml`）
- ルールを読み、各ルールを評価し、ルール 1 つにつき 1 行表示、失敗時に
  詳細を追加表示。
- exit code: 失敗したルールが 0 なら `0`、それ以外 `1`。

### 7.2 `pathlint which <command>`（MVP）

- OS のルール（Windows なら `PATHEXT`、Unix なら `+x`）に従って PATH
  からコマンドを resolve。
- 勝者のパスを最初に表示し、続いて PATH の後方にある shadow されている
  同名 exe を `[shadowed]` 注記つきで表示。「最初に勝つ、後ろは到達可能
  だが使われない」関係を見える化するのが目的。
- exit code: 1 つ以上マッチで `0`、それ以外 `1`。

### 7.3 `pathlint.toml` スキーマ

```toml
# 各 [[rule]] が宣言する：`before` を含むエントリの少なくとも 1 つが、
# `after` のいずれかを含む全エントリより PATH 上で前にあること。

[[rule]]
name   = "mise shims override system tools"
before = "mise\\shims"
after  = ["chocolatey\\bin", "Strawberry\\c\\bin"]

[[rule]]
name   = "user cargo bin precedes Strawberry's gcc/perl"
before = ".cargo\\bin"
after  = ["Strawberry"]
```

マッチセマンティクス：

- 部分一致、大文字小文字無視、各 PATH エントリに対し **環境変数展開後** に
  評価。
- ルールが **OK**: `after` のすべてのマッチが、最初の `before` マッチ
  より後ろにある。
- ルールが **fail**: いずれかの `after` マッチが、すべての `before` マッ
  チより前に来る。または `after` にマッチがあり `before` には無い場合。
- ルールが **skip**: PATH に双方とも存在しない場合。

### 7.4 PATH ソースの解決

- `process`: `$env:PATH`（Unix では `$PATH`）を読む。
- `user`（Windows のみ）: レジストリ `HKCU\Environment\Path` を読む。
- `machine`（Windows のみ）: レジストリ
  `HKLM\System\CurrentControlSet\Control\Session Manager\Environment\Path` を読む。
- Unix では `--target user|machine` は警告を出して `process` にフォール
  バック。

### 7.5 出力

- デフォルト: ルール 1 つにつき 1 行（`OK` / `NG` / `skip`）、失敗時のみ
  インデントされた詳細行を続ける。
- `--verbose`: 展開後の PATH エントリも併せて出す。
- `--quiet`: 失敗のみ出力。

## 8. 非機能要件

- **単一の Rust バイナリ。** OS 以外の runtime 依存無し。
- **クロスプラットフォーム。** Windows（最優先）、macOS、Linux。Termux
  はソースビルドのみ（`dotfm` と同じ方針）。
- **起動時間。** PATH 約 100 件、ルール約 20 件で `pathlint check` が warm
  cache 50 ms 以内。
- **安定した exit code。** `0` クリーン、`1` ルール失敗、`2` 設定パース
  / I/O エラー。

## 9. 配布

- 0.0.1 ship 後に crates.io publish。
- GitHub Releases workflow で `x86_64-{linux,windows,darwin}` と
  `aarch64-darwin` のアーカイブを配布。`dotfm` と同じ流れ。
- (post-MVP) Homebrew / scoop / winget formulae。

## 10. スコープ外

- PATH の編集／永続化（後の `sort` モードに先送り）。
- 関数／エイリアス resolve は対象外。PATH 上のファイル探索のみ。
- シェル設定パッチ（`.bashrc`、`$PROFILE` の書き換え）。
- ルール評価の副作用以上の「ツールが見つからない」検出。

## 11. 成功指標

- リファレンス dotfiles（`ShortArrow/dotfiles`）が
  `windows/Test-PathOrder.ps1` を `pathlint check` 呼び出しに置き換え、
  ルールファイルが同じリポに置かれる。
- README をコピペ・編集する形で 1 分以内に 5 ルールの `pathlint.toml`
  を書ける。
- 失敗実行が、追加のデバッグツール無しで直せる程度に明確にすべての
  衝突ペアを示す。

## 12. 未解決事項

- **負マッチ（`before_not`）**：「ユーザーの `go\\bin`、システムの
  `go\\bin` ではなく」を表現したい。MVP 外。実際に 2 つ目の衝突が
  起きたら考える。
- **Windows 限定の env 展開。** 現状 `%VAR%`（Windows 形式）と
  `$VAR` / `${VAR}`（POSIX 形式）を一律展開する想定。OS 別動作にする
  かは要検討。
- **シェル補完。** `clap_complete` で安く足せるが MVP 外。
- **macOS launchd / Linux PAM の PATH ソース。** プロセスレベル PATH は
  これらの合成。表面化させるかは要検討。

## 13. 他ツールとの関係

- **`which` / `where.exe`**: 同じドメイン（コマンドの解決位置）だが
  「べき」の概念は無い。`pathlint which` は補完であって置き換えでは
  ない。
- **`dotfm doctor`**: `pathlint check` は `dotfm.toml` の
  `[tools.windows.doctor]` スクリプト（または後継）から呼ばれる想定。
  `dotfm` を置き換えるものではない。
- **`PATH.txt` / `DiffPath.ps1`（`ShortArrow/dotfiles` 内）**: これらは
  「期待エントリが存在するか」を見る。`pathlint` は「順序が正しいか」を
  見る。両者は補完関係。
