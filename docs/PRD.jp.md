# pathlint — プロダクト要件定義書（PRD）

**ステータス:** ドラフト（実装前）。
**対象リリース:** 0.0.1 MVP。

---

## 1. 概要

`pathlint` は、TOML で書かれた **宣言的な順序ルールに照らして `PATH`
環境変数を検証する** CLI。次の問いに答える：

> 「このコマンドは、本来勝つべき実体から resolve されているか？」

ルールは「X は PATH の中で Y より先に来なければならない」という形。
各ルールについて **OK / NG / skip**（skip = 双方とも PATH に存在し
ない）を表示し、失敗時には該当エントリの index を含めて理由を示す。

MVP は読み取り専用（lint モード）。後続バージョンで並べ替え案の提示・
適用（sort / fix モード）を予定。

1 つの `pathlint.toml` が **Windows、macOS、Linux、Termux** すべてで
動くことを意図している。各ルールに `os = [...]` タグを付けられるので、
Windows 固有ルール（WindowsApps スタブ、chocolatey、Strawberry）と
macOS 固有ルール（Homebrew vs system）と Linux/Termux 固有ルール
（mise vs distro pkg）を 1 つのファイルに同居できる。

## 2. 課題定義

PATH の順序バグはどの OS でも形が違って見えるが、本質は同じ — 「同じ
名前の異なる実体のうち、間違った方が勝っている」：

- **Windows.** `python.exe` の Microsoft Store スタブが本物のインス
  トール（mise、conda、asdf、手動）を覆い隠す。Strawberry Perl の
  `gcc` が Rust／MSYS のツールチェインを覆い隠す。残骸の
  `WindowsPowerShell\v1.0` エントリが PowerShell 7 ではなく
  `pwsh.exe` を resolve する。
- **macOS.** `/usr/bin/python3`（Apple 提供のシステム Python）が
  Homebrew や pyenv を覆う。Intel と arm の brew が両方ある場合の
  `/usr/local/bin` vs `/opt/homebrew/bin` の順序が問題になる。
- **Linux.** distro パッケージの `node` が nvm や mise を覆う。
  `/snap/bin` が `~/.cargo/bin` を覆う。`/usr/games` が `~/bin` より
  先でローカルスクリプトを覆う。
- **Termux.** `~/bin` が `$PREFIX/bin` の後ろに来ると、ユーザー作成
  スクリプトで `pkg install` 提供のツールを上書きできない。

`which X` は何が勝つかを見せてくれるが、何が勝つべきかは見せない。
「勝つべきもの」を宣言的に書く手段が無いため、CI / dotfiles /
doctor スクリプトから検証できない。`pathlint` がそのギャップを埋める。

## 3. ゴール

- **宣言的なルール。** `pathlint.toml` に `[[rule]]` エントリで
  「X は Y より先」を素朴な TOML で書ける。
- **1 ファイルで全 OS。** `os = [...]` フィルタを付けられるので、
  1 つの `pathlint.toml` が Windows、macOS、Linux、Termux で動く。
  タグ無しのルールは全 OS で適用される。
- **部分一致 + 大文字小文字無視。** ルールキーは厳密なパスでなくて
  良い。case-insensitive を OS 横断で統一する（Windows は元々無視、
  Unix では厳密性を犠牲に同じファイルの可搬性を取る）。
- **OS を意識した PATH ソース。** `--target process|user|machine` で
  読み取る PATH を切り替え。Windows では `user` / `machine` はレジ
  ストリ。Unix では警告を出して `process` にフォールバック。
- **正直な exit code。** `0` = クリーン、`1` = ルール失敗、`2` =
  config / I/O エラー。
- **役に立つ失敗出力。** 失敗ルールごとに、衝突エントリの index と
  短い理由を表示（例：`'chocolatey\\bin' at #42 precedes
  'mise\\shims' at #49`）。
- **MVP では非変更。** 読み取りのみ。`--apply` / `sort` は後回し。

## 4. 非ゴール（MVP）

- **PATH の書き換え／永続化はしない。** sort/fix は後回し。
- **`.bashrc`、`$PROFILE`、レジストリの編集はしない。** lint 出力は
  「何を直すべきか」を示す。「どう直すか」はユーザー判断。
- **シェル補完のインストールはしない。** `pathlint completions <shell>`
  は将来追加するかも。
- **パッケージ管理はしない。** ルールを満たすために不足ツールを入れる
  ような副作用は持たない。
- **launchd / PAM / `/etc/environment` の深いパースはしない。**
  プロセスが実際に見ている PATH（`getenv("PATH")`）と、Windows なら
  レジストリ 2 ヶ所までを読む。それ以外の階層はスコープ外。

## 5. ターゲットユーザー

- 自分の `doctor` ステップで PATH ドリフトを検出したい dotfiles 利用者
  — 自宅 Windows、業務 macOS、WSL、Termux スマホ等を全部カバーしたい。
- 「なぜこの Python が動いて、あちらの Python が動かないのか」を
  再起動後にも残る形で追いかけたい開発者。
- 開発環境を bootstrap する CI で、PATH の順序リグレッションを
  はっきり失敗させたい運用者。

## 6. ユーザーストーリー

- 自分が気にするルールを `pathlint.toml` に書く — 一部に
  `os = ["windows"]`、一部に `os = ["macos", "linux", "termux"]`、
  一部はタグ無し — dotfiles に commit、`pathlint check` が各マシン
  で正しい部分集合だけを評価する。
- linter run は全ルールとそのステータスを表示。失敗したらどのエン
  トリがどのエントリに勝っているかが見える。
- Windows で `setx PATH ...` する前に
  `pathlint check --target user` で User PATH だけ検証できる。
- Termux で `pathlint check` を走らせると、`$PREFIX/bin` が
  `/usr/bin` の代替であることを理解した上で評価される。
- （MVP 後）`pathlint sort --target user --dry-run` で何が変わるか
  diff として見る。

## 7. 機能要件（MVP）

### 7.1 `pathlint [OPTIONS]`（= `pathlint check`）

`check` がデフォルトサブコマンド。`pathlint` 単体で `check` 動作。

```
pathlint                              # = pathlint check
pathlint --target user                # 明示的なターゲット
pathlint --rules ./other.toml
pathlint --verbose                    # 展開後 PATH エントリも dump
pathlint --quiet                      # 失敗のみ
```

- `--target` のデフォルトは `process`。`user` / `machine` はどの OS
  でも受け付けるが Windows でのみ意味を持つ。Unix では 1 行警告を出
  して `process` にフォールバック。
- `--rules` のデフォルト解決順：
  1. `--rules <path>` が指定されればそれ。
  2. `./pathlint.toml` があればそれ。
  3. `$XDG_CONFIG_HOME/pathlint/pathlint.toml`（または
     `$HOME/.config/pathlint/pathlint.toml`）。
- ロードした各ルールについて、解決した PATH に対し評価。`os` フィル
  タが現在 OS を除外しているルールは静かにスキップ（`--verbose` で
  `n/a` として表示）。
- ルール 1 つにつき 1 行のステータス（`OK` / `NG` / `skip`）。失敗時
  はインデントされた詳細行が続く。
- exit code: 失敗ルールが 0 なら `0`、それ以外 `1`。

### 7.2 `pathlint which <command>`（MVP）

- OS のルール（Windows なら `PATHEXT`、Unix なら実行可能ビット）に
  従って PATH からコマンドを resolve。
- 勝者のパスを最初に表示し、続いて PATH の後方にある shadow されている
  同名 exe を `[shadowed]` 注記つきで表示。「最初に勝つ、後ろは到達可能
  だが使われない」関係を見える化するのが目的。
- exit code: 1 つ以上マッチで `0`、それ以外 `1`。

### 7.3 `pathlint init`（planned、MVP 外）

- 現ディレクトリに starter `pathlint.toml` を出力。現 OS 向けの少数
  の OS タグ付きルール例 + 他 OS のコメント例で埋める。MVP ではスキッ
  プ。`pathlint init --os <list>` で他 OS のデフォルトをシードできる
  ようにする案あり。

### 7.4 `pathlint sort`（post-MVP）

- 全ルールを満たす PATH エントリのトポロジカル順序を計算し、表示
  （`--dry-run` がデフォルト）または OS に応じた API で適用
  （`--apply`、Windows レジストリ／shell-rc 挿入）。0.0.x ではスコー
  プ外。

## 8. `pathlint.toml` スキーマ

```toml
# 各 [[rule]] が宣言する：`before` を含むエントリの少なくとも 1 つが、
# `after` のいずれかを含む全エントリより PATH 上で前にあること。

# タグ無し — 全 OS で適用。
[[rule]]
name   = "PowerShell 7 precedes legacy WindowsPowerShell"
before = "PowerShell\\7"
after  = ["WindowsPowerShell\\v1.0"]

# Windows 専用ルール。
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

### 8.1 マッチセマンティクス

- 部分一致、大文字小文字無視、各 PATH エントリに対し **環境変数展開後**
  に評価（§8.2 参照）。
- 比較前に slash と backslash を一方に正規化（`\` → `/`）。よって
  `mise\shims` は `mise/shims` ともマッチし、その逆も成立。これにより
  1 つのルールファイルが OS 横断で動く。
- ルールが **OK**: `after` のすべてのマッチが、最初の `before` マッ
  チより後ろにある。
- ルールが **fail**: いずれかの `after` マッチが、すべての `before`
  マッチより前に来る。または `after` にマッチがあり `before` には無い
  場合。
- ルールが **skip**: PATH に双方とも存在しない場合。
- ルールが **n/a**: `os` フィルタが現在 OS を除外している場合。
  `--verbose` でない限り静かに無視。

### 8.2 環境変数展開

PATH エントリはマッチ前に統一的に展開される：

- `%VAR%`（Windows 形式）を展開。
- `$VAR` および `${VAR}`（POSIX 形式）を展開。
- 先頭の `~` をホームディレクトリに展開。
- 展開できない `%VAR%` / `$VAR` はそのまま残す（エラーにしない）。

両形式をどの OS でも受け付ける。これにより、ルールに
`before = "$HOME/bin"` と書いておけば、Windows pwsh の下で実エントリ
が `%USERPROFILE%\bin` だったとしても、両方とも同じ絶対パスに展開さ
れて一致する。

### 8.3 OS 識別子

`os` フィールドは以下の文字列を受け付ける：

| 値 | 該当条件 |
|---|---|
| `"windows"` | Windows で実行中（`cfg!(windows)`） |
| `"macos"` | macOS で実行中（`cfg!(target_os = "macos")`） |
| `"linux"` | Linux で実行中 **かつ** Termux ではない |
| `"termux"` | Termux で実行中（`PREFIX` 環境変数が `/data/data/com.termux/files` 以下を指していることで検出） |
| `"unix"` | macOS / Linux / Termux のいずれか（便利エイリアス） |

Termux を独立扱いするのは、ファイルシステムレイアウトが汎用 Linux と
本質的に違うため（`/usr/bin` が存在しない；すべてが `$PREFIX` 以下に
あるため）。`/usr/bin` を語るルールは Termux で発火させたくない。

## 9. PATH ソース

| `--target` | Windows | macOS / Linux / Termux |
|---|---|---|
| `process` | `GetEnvironmentVariable("PATH")` | `getenv("PATH")` |
| `user` | `HKCU\Environment\Path`（レジストリ） | 警告 + `process` にフォールバック |
| `machine` | `HKLM\System\CurrentControlSet\Control\Session Manager\Environment\Path` | 警告 + `process` にフォールバック |

`process` は Windows では Machine と User の和。Unix には「Machine vs
User」のレジストリ的区別が無い — `pathlint` は MVP では `~/.bashrc`、
`~/.zshrc`、`/etc/environment`、launchd plist、PAM をパースしない。
（要望があれば `shellrc` ソースを後で追加するかも。）

## 10. CLI 表面

```
pathlint [OPTIONS] [COMMAND]

Commands:
  check    PATH をルールに照らして lint（デフォルト）
  which    コマンドを PATH から resolve し、shadow されたコピーを列挙
  help     ヘルプ表示

Options（global）:
      --target <process|user|machine>  デフォルト: process
      --rules <path>                   デフォルト: ./ → $XDG_CONFIG_HOME/pathlint/
  -v, --verbose                        全ルール（n/a 含む）と展開後 PATH を表示
  -q, --quiet                          失敗のみ表示
      --color <auto|always|never>      デフォルト: auto
      --no-glyphs                      ASCII のみ（デフォルトも ASCII。グリフは将来 opt-in）
  -h, --help
  -V, --version
```

## 11. 非機能要件

- **単一の Rust バイナリ。** OS 以外の runtime 依存無し。
- **クロスプラットフォーム第一級。** Windows、macOS、Linux すべてを
  CI で確認。Termux は端末上の `cargo install` 経由のみ — `dotfm`
  と同じ方針でビルド済み配布はしない。
- **起動時間。** PATH 約 100 件、ルール約 20 件で `pathlint check`
  が warm cache 50 ms 以内。
- **安定した exit code。** `0` クリーン、`1` ルール失敗、`2` config
  / I/O エラー。
- **エンコーディング。** どの OS でも path は UTF-8 文字列として扱う。
  稀に存在する非 UTF-8 PATH エントリは警告を出してスキップ。

## 12. 配布

- 0.0.1 ship 後に crates.io publish。
- GitHub Releases workflow で `x86_64-{linux,windows,darwin}` と
  `aarch64-darwin` のアーカイブを配布。`dotfm` と同じ流れ。Termux ユー
  ザーはソースからビルド。
- (post-MVP) Homebrew formula、scoop manifest、AUR PKGBUILD。

## 13. スコープ外

- PATH の編集／永続化（後の `sort` モードに先送り）。
- 関数／エイリアス resolve は対象外。PATH 上のファイル探索のみ。
- シェル設定パッチ（`.bashrc`、`$PROFILE` の書き換え）。
- ルール評価の副作用以上の「ツールが見つからない」検出。
- `/etc/environment`、PAM、launchd plist、systemd unit `Environment=`
  などのパース。

## 14. 成功指標

- リファレンス dotfiles（`ShortArrow/dotfiles`）が
  `windows/Test-PathOrder.ps1` を `pathlint check` 呼び出しに置き換
  え、ルールファイルが同じリポに置かれる（自分の所有する全 OS で動く
  状態）。
- README をコピペ・編集する形で 1 分以内に 5 ルールの `pathlint.toml`
  を書ける（最低 1 つは OS タグ付き）。
- 失敗実行が、追加のデバッグツール無しで直せる程度に明確にすべての
  衝突ペアを示す。

## 15. 未解決事項

- **`before_not`（負マッチ）。** "ユーザー `go/bin`" が "システム
  `Go/bin`" より先 — 両方とも `go/bin` を含むため必要、というケース。
  MVP 外。実際に 2 つ目の衝突が起きたら考える。
- **`shellrc` ソース。** `--target shellrc` で `.bashrc` / `.zshrc`
  の `export PATH=...` 行をパースすべきか。「shellrc に PATH 変更
  commit したけど fresh shell がまだ取り込んでない」を検出するのに
  便利。MVP 外。
- **Termux PATH 慣習。** `os = ["termux"]` ルールで `/usr/bin` を
  `$PREFIX/bin` に自動書き換えするか、それとも書かれた通りに評価する
  か。現状方針: 書き換えず、ルールはそのままの意味。
- **macOS launchd。** `launchctl getenv PATH` がアプリによっては
  process PATH と違う場合あり。MVP 外。
- **`pathlint sort` のセマンティクス。** ルールが衝突したらどちらが
  勝つか。トポロジカルソート＋循環依存の処理は実装前に設計が要る。
- **シェル補完** を `clap_complete` で。安いが post-MVP。

## 16. 他ツールとの関係

- **`which` / `where.exe`**: 同じドメイン（コマンドの解決位置）だが
  「べき」の概念は無い。`pathlint which` は補完であって置き換えでは
  ない。
- **`dotfm doctor`**: `pathlint check` は `dotfm.toml` の
  `[tools.<name>.doctor]` スクリプトから呼ばれる想定。`dotfm` を
  置き換えるものではない。推奨レイアウト：1 つの `pathlint.toml`
  を dotfiles リポに置き、Windows と Unix の doctor スクリプトの
  両方から参照する。
- **`PATH.txt` / `DiffPath.ps1`（`ShortArrow/dotfiles` 内）**: これ
  らは「期待エントリが存在するか」を見る。`pathlint` は「順序が正し
  いか」を見る。両者は補完関係。
- **パッケージマネージャ（mise、brew、choco、pkg）**: `pathlint` は
  インストールを管理しない。彼らが作る順序が望むものかを教える。
