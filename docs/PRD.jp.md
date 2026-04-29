# pathlint — プロダクト要件定義書（PRD）

**ステータス:** 0.0.x 進行中。
**対象リリース:** 0.0.2 が初の動作版。スキーマと CLI 表面は引き続き
動きうる（0.1.0 で安定化予定）。

---

## 1. 概要

`pathlint` は **「各コマンドが、自分が期待しているインストーラから
解決されているか？」** を TOML マニフェストに照らして検証する CLI。

ユーザーは次のように宣言する：

> 「`runex` は `cargo` から来てほしい。`winget` ではなく。」

`pathlint` は実 `PATH` から `runex` を解決し、勝者バイナリの場所を
見て、それが定義された **source ラベル**（"cargo" / "winget" など）
のどれにマッチするかを判定する。

出力は expectation 1 つにつき 1 行：**OK / NG / skip / n/a**。
失敗時は実際の解決パスとマッチした source（あるいは無マッチ）を
表示する。

1 つの `pathlint.toml` が **Windows、macOS、Linux、Termux** すべて
で動く — source は OS 別の場所を宣言でき、各 expectation には
`os = [...]` フィルタを付けられる。

`pathlint` は well-known な source の **組み込みカタログ** を持つ
（`cargo`、`mise`、`volta`、`winget`、`choco`、`scoop`、`brew_arm`、
`brew_intel`、`apt`、`pacman`、`pkg`、`flatpak`、`WindowsApps` …）。
ユーザーは **expectation を書くだけ** でよく、source は名前で参照
されて自動解決される。

## 2. 課題定義

同じコマンド名が異なるインストーラから来ることはよくあって、どれが
勝つかが大事：

- このマシンで `cargo install runex` したのに、実際に走るのは
  `WinGet/Links` にある古いほう。同名・別ファイル。
- `python` は `mise` 由来であってほしい、Microsoft Store の
  `WindowsApps` スタブからではなく。
- `node` は `volta` 由来がいい、システムの `apt` インストールでは
  なく。
- macOS の `gcc` は Homebrew 由来であってほしい、`/usr/bin/gcc`
  （かつて clang シムだった）からではなく。

`which` は何が勝つかを教えてくれるが、何が勝つべきかを dotfiles リポ
にコミットして全マシンでチェックできる形では教えてくれない。
`pathlint` がこの「あるべき姿」を明示し検証する。

## 3. ゴール

- **宣言的な expectation。** `pathlint.toml` に `[[expect]]` で
  「コマンド X は source S から解決されるべき」を書ける。
- **パスではなく source ラベル。** ユーザーはインストーラ名
  （`cargo`、`mise`、`winget`、`brew_arm`、`apt`）で書く。生のパスは
  カタログから引かれる。
- **組み込みカタログ + 上書き。** pathlint がよく使われるインストーラ
  のデフォルトを内蔵。ユーザーは上書きしたいとき、または新規追加した
  いときだけ `[source.X]` を書く。
- **1 ファイル、全 OS。** 各 `[[expect]]` に `os = [...]` フィルタ、
  各 `[source.X]` に OS 別パス（`windows = ...`、`unix = ...` など）。
  同じ `pathlint.toml` が Windows / macOS / Linux / Termux を回す。
- **部分一致 + 大文字小文字無視。** 環境変数展開と slash 正規化の
  あとで、source パスを解決済みパスに対し substring 比較。
- **正直な exit code。** `0` = クリーン、`1` = 1 つ以上失敗、`2` =
  config / I/O エラー。
- **役に立つ失敗出力。** 失敗 expectation はコマンド名、実解決パス、
  マッチした source（または `prefer` / `avoid` 違反）を示す。
- **MVP では非変更。** 読み取りのみ。`--apply` / `sort` は後回し。

## 4. 非ゴール（MVP）

- **PATH の書き換え／永続化はしない。** sort/fix は後回し。
- **`.bashrc`、`$PROFILE`、レジストリの編集はしない。** 出力は何が
  間違っているかを示す。どう直すかはユーザー判断。
- **`which` クローンではない。** `pathlint` 内部に resolve ロジックは
  あるが、`where` / `type -a` / `Get-Command -All` を置き換える意図は
  ない。pathlint が答える面白い問いは「正しいやつが勝っているか？」で
  あって、「これはどこから resolve されるか？」ではない。
- **パッケージ管理はしない。** expectation を満たすために不足ツール
  を入れない。
- **launchd / PAM / `/etc/environment` の深いパースはしない。**
  プロセスが実際に見ている PATH（`getenv("PATH")`）と、Windows なら
  レジストリ 2 ヶ所までを読む。それ以外の階層はスコープ外。

## 5. ターゲットユーザー

- 自分の `doctor` ステップで source ドリフトを検出したい dotfiles
  利用者 — 自宅 Windows、業務 macOS、WSL、Termux スマホなど全部
  カバーしたい。
- 自分で `cargo install` しているツールを反復開発していて、リリース版
  の winget / brew コピーではなく、自分のビルドが走っていることを
  確認したい開発者。
- 開発環境を bootstrap する CI で、間違ったインストーラが勝った
  ときにはっきり失敗させたい運用者。

## 6. ユーザーストーリー

- 自分が気にするコマンドだけ `pathlint.toml` に 5 行の `[[expect]]`
  で書く — source 定義は不要、組み込みでカバーされる。
  `pathlint check` が各 OS で正しい部分集合だけを評価する。
- linter run は全 expectation とそのステータスを表示。失敗時は実
  解決パスと違反した `prefer` / `avoid` を見せる。
- mise を独自パスに置いているマシンでは、`pathlint.toml` で
  `[source.mise]` を上書きする。
- （MVP 後）`pathlint sort --target user --dry-run` で全 expectation
  を満たすように PATH を並べ替える diff を見る。

## 7. 機能要件（MVP）

### 7.1 `pathlint [OPTIONS]`（= `pathlint check`）

`check` がデフォルトサブコマンド。`pathlint` 単体で `check` 動作。

```
pathlint                              # = pathlint check
pathlint --target user                # 明示的なターゲット
pathlint --rules ./other.toml
pathlint --verbose                    # n/a 含む全 expectation と解決後 PATH を表示
pathlint --quiet                      # 失敗のみ
```

- `--target` のデフォルトは `process`。`user` / `machine` はどの OS
  でも受け付けるが Windows でのみ意味を持つ。Unix では 1 行警告を
  出して `process` にフォールバック。
- `--rules` のデフォルト解決順：
  1. `--rules <path>` が指定されればそれ。
  2. `./pathlint.toml` があればそれ。
  3. `$XDG_CONFIG_HOME/pathlint/pathlint.toml`（または
     `$HOME/.config/pathlint/pathlint.toml`）。
- 各 `[[expect]]` について：
  1. `os` フィルタが現在 OS を除外していたら → ステータス `n/a`。
  2. `command` を選択した PATH に対し resolve（Windows なら
     `PATHEXT`、Unix なら実行ビット）。
  3. resolve 不能なら → ステータス `not_found`（`optional = true`
     でない限り failure 扱い）。
  4. 解決した実パスを定義済み `[source.X]` のすべてと照合。マッチ
     した source 名（複数可）を記録。
  5. **OK**: マッチした source の少なくとも 1 つが `prefer` に含
     まれ、かつ `avoid` のものを 1 つもマッチしていない。
  6. **NG**: それ以外。実解決パスと不一致理由を表示。
- expectation 1 つにつき 1 行のステータス。失敗時はインデントされた
  詳細行が続く。
- exit code: `NG` も `not_found` もなければ `0`、それ以外 `1`
  （`optional` は除く）。

### 7.2 source カタログのマージ

- pathlint は組み込みの source カタログを持つ（§9 参照）。
- ユーザー `pathlint.toml` は `[source.<name>]` を任意個書ける：
  - 組み込みと同じ `<name>` → ユーザーが OS 別パスをフィールド単位で
    上書き。
  - 新しい `<name>` → カタログに追加。
- expectation は merged カタログ中の任意の source 名を参照可。未定義
  の source 名を参照したら config エラー。

### 7.3 `pathlint init`（実装ずみ）

- 現ディレクトリに starter `pathlint.toml` を出力。現 OS 向けの少数
  の `[[expect]]` 例で埋める。
- `pathlint init --emit-defaults` で組み込み source カタログ全体を
  ファイルに書き出すこともできる（編集・削除しやすくするため）。
  デフォルトはオフ（ファイルを短く保つため）。
- 既存ファイルがあれば exit 1 で書き換えを拒否。`--force` で許可。

### 7.4 `pathlint catalog list`（実装ずみ）

- 組み込み + ユーザー定義 source 一覧を表示。
- デフォルトは現 OS のパスのみ。`--all` で全 OS のフィールドを縦
  展開。`--names-only` で名前だけ（シェル連携用）。

### 7.5 `pathlint doctor`（実装ずみ）

- `[[expect]]` とは独立に PATH 自体を lint。
- **Error**（exit 1）: 形式破損（NUL 埋め込み、Windows の NTFS 非合
  法文字）。OS が directory として扱えないので escalate。
- **Warn**（exit 0）:
  - 重複エントリ（環境変数展開と slash 正規化のあとで同一）。
  - ディレクトリ不在。
  - 末尾スラッシュ。
  - Windows 8.3 短縮名（`PROGRA~1`）。
  - ケース／slash 違い重複（同じ正規化形だが verbatim が違う）。
  - 環境変数で短縮できる候補（`%LocalAppData%` / `%UserProfile%` /
    `$HOME` 系）。提案文字列は元のケースと slash 向きを保つ。
- `--quiet` で warn 抑制、error は常に表示。

### 7.6 `pathlint sort`（post-MVP）

- 全 expectation を満たす PATH 順序を計算し、表示
  （`--dry-run` がデフォルト）または OS に応じた API で適用
  （`--apply`、Windows レジストリ／shell-rc 挿入）。0.0.x ではスコー
  プ外。

## 8. `pathlint.toml` スキーマ

```toml
# ---- [[expect]]: コマンドごとの期待 ----

# タグ無し: 名前付き source が定義されている全 OS で適用。
[[expect]]
command = "runex"
prefer  = ["cargo"]            # マッチした source の 1 つは必ずここに含まれる
avoid   = ["winget"]           # マッチした source は 1 つもここに含まれない
os      = ["windows", "macos", "linux", "termux"]   # 任意。デフォルトは全 OS

[[expect]]
command = "python"
prefer  = ["mise"]
avoid   = ["WindowsApps", "choco"]
os      = ["windows"]

[[expect]]
command = "python"
prefer  = ["mise", "pkg"]
os      = ["termux"]

[[expect]]
command = "gcc"
prefer  = ["mingw", "msys"]
avoid   = ["strawberry"]
os      = ["windows"]

[[expect]]
command = "git"
optional = true                # PATH に無くても黙ってスキップ
prefer  = ["winget", "apt", "brew_arm", "brew_intel"]


# ---- [source.<name>]: ディスク上の source の見分け方 ----

# 組み込みの上書き（mise を D:\tools\mise に置いてるマシン）：
[source.mise]
windows = "D:/tools/mise"

# 組み込みカタログに無い source の新規定義：
[source.my_dotfiles_bin]
unix = "$HOME/dotfiles/bin"
```

### 8.1 マッチセマンティクス

各 `[source.X]` について、OS 別パス文字列（環境変数展開と slash 正
規化のあと）を解決済みバイナリパスと照合。**部分一致 + 大文字小文字
無視**。

- コマンドが *source にマッチする* とは、解決後バイナリのフルパス
  が source の OS 別パスを substring として含むこと。
- コマンドは **0、1、または複数** の source にマッチしうる。複数で
  も問題ない（例：`mise/installs/python/3.x/bin/python.exe` は
  `[source.mise]` と `[source.python_install]` の両方にマッチする）。
- ステータス判定はマッチした source 名の **集合** に対して：
  - **OK**: 少なくとも 1 つは `prefer` に含まれ、かつ `avoid` のもの
    を 1 つも含まない。
  - **NG (wrong source)**: 1 つ以上 source にマッチしたが、`prefer`
    に含まれないか `avoid` に含まれる。
  - **NG (unknown source)**: 解決パスがどの source にもマッチせず、
    かつ `prefer` が空でない。（「source は何でも良い、存在さえす
    れば OK」にしたいなら `prefer` を空にして `avoid` だけ書く。）
  - **NG (not found)**: コマンドが PATH 上に無く、`optional = false`
    （デフォルト）。
  - **n/a**: `os` フィルタが現在 OS を除外している。

### 8.2 環境変数展開

source パスと PATH エントリは、マッチ前に統一的に展開：

- `%VAR%`（Windows 形式）を展開。
- `$VAR` および `${VAR}`（POSIX 形式）を展開。
- 先頭の `~` をホームディレクトリに展開。
- 展開できない `%VAR%` / `$VAR` はそのまま残す（エラーにしない）。

両形式をどの OS でも受け付けるので、同じ `pathlint.toml` が Windows
pwsh、macOS bash、Termux fish いずれでも動く。

slash 正規化：`\` と `/` は単一表現（`/`）に統一されてから substring
比較される。よって TOML リテラルでの `mise\\shims` と `mise/shims`
は等価。

### 8.3 OS 識別子

`[[expect]]` の `os` フィールド、および `[source.X]` の OS 別キーは
以下の文字列を受け付ける：

| 値 | 該当条件 |
|---|---|
| `"windows"` | Windows で実行中（`cfg!(windows)`） |
| `"macos"` | macOS で実行中（`cfg!(target_os = "macos")`） |
| `"linux"` | Linux で実行中 **かつ** Termux ではない |
| `"termux"` | Termux で実行中（`PREFIX` 環境変数が `/data/data/com.termux/files` 以下を指していることで検出） |
| `"unix"` | macOS / Linux / Termux のいずれか（便利エイリアス） |

Termux を独立扱いするのは、ファイルシステムレイアウトが汎用 Linux と
本質的に違うため（`/usr/bin` が存在しない；すべてが `$PREFIX` 以下に
あるため）。`apt`（= `/usr/bin`）のような source は Termux で発火さ
せたくない。

## 9. 組み込み source カタログ

pathlint は次の TOML 相当のデフォルトカタログを内蔵する。各エントリ
はユーザー `pathlint.toml` でフィールド単位に上書き可能。

```toml
# ---- OS横断のユーザーインストール系 ----

[source.cargo]
description = "binaries from `cargo install`"
windows = "$UserProfile/.cargo/bin"
unix    = "$HOME/.cargo/bin"

[source.go]
description = "binaries from `go install`"
windows = "$UserProfile/go/bin"
unix    = "$HOME/go/bin"

[source.npm_global]
windows = "$AppData/npm"
unix    = "$HOME/.npm-global/bin"

[source.pip_user]
windows = "$AppData/Python"
unix    = "$HOME/.local/bin"

[source.user_bin]
windows = "$UserProfile/bin"
unix    = "$HOME/bin"

[source.user_local_bin]
unix    = "$HOME/.local/bin"

# ---- 多言語バージョンマネージャ ----

[source.mise]
windows = "$LocalAppData/mise"
unix    = "$HOME/.local/share/mise"

[source.volta]
windows = "$LocalAppData/Volta"
unix    = "$HOME/.volta/bin"

[source.aqua]
windows = "$LocalAppData/aquaproj-aqua"
unix    = "$HOME/.local/share/aquaproj-aqua"

[source.asdf]
unix    = "$HOME/.asdf/shims"

# ---- Windows 専用パッケージマネージャ ----

[source.winget]
windows = "$LocalAppData/Microsoft/WinGet"

[source.choco]
windows = "$ProgramData/chocolatey"

[source.scoop]
windows = "$UserProfile/scoop"

[source.WindowsApps]
description = "Microsoft Store stub layer"
windows = "Microsoft/WindowsApps"

[source.strawberry]
windows = "Strawberry"

[source.mingw]
windows = "mingw"

[source.msys]
windows = "msys"

# ---- macOS 専用パッケージマネージャ ----

[source.brew_arm]
description = "Homebrew on Apple Silicon"
macos = "/opt/homebrew"

[source.brew_intel]
description = "Homebrew on Intel macOS"
macos = "/usr/local"

[source.macports]
macos = "/opt/local"

# ---- Linux 専用パッケージマネージャ ----

[source.apt]
linux = "/usr/bin"

[source.pacman]
linux = "/usr/bin"

[source.dnf]
linux = "/usr/bin"

[source.flatpak]
linux = "/var/lib/flatpak/exports/bin"

[source.snap]
linux = "/snap/bin"

# ---- Termux ----

[source.pkg]
description = "Termux pkg installs"
termux = "$PREFIX/bin"

[source.termux_user_bin]
termux = "$PREFIX/../home/bin"

# ---- OS ベースライン（包括的「システム PATH」source） ----

[source.system_windows]
windows = "$SystemRoot/System32"

[source.system_macos]
macos = "/usr/bin"

[source.system_linux]
linux = "/usr/bin"
```

注：

- `apt` / `pacman` / `dnf` はすべて `/usr/bin` を指す。インストール
  バイナリの着地先が同じだから。pathlint からは「Linux distro」と
  ほぼ同義のエイリアス。ユーザーが `pathlint.toml` で読みやすい
  ものを選ぶ。
- `brew_arm` と `brew_intel` を分けたのは、Mac 1 台での
  `/opt/homebrew/bin` vs `/usr/local/bin` 順序自体がよくあるバグ源
  だから。
- `WindowsApps` と `strawberry` は主に `avoid = [...]` リスト用に
  用意。

## 10. PATH ソース（`--target`）

| `--target` | Windows | macOS / Linux / Termux |
|---|---|---|
| `process` | `GetEnvironmentVariable("PATH")` | `getenv("PATH")` |
| `user` | `HKCU\Environment\Path`（レジストリ） | 警告 + `process` にフォールバック |
| `machine` | `HKLM\System\CurrentControlSet\Control\Session Manager\Environment\Path` | 警告 + `process` にフォールバック |

`process` は Windows では Machine と User の和。Unix には「Machine
vs User」のレジストリ的区別が無い — `pathlint` は MVP では
`~/.bashrc`、`~/.zshrc`、`/etc/environment`、launchd plist、PAM を
パースしない。

## 11. CLI 表面

```
pathlint [OPTIONS] [COMMAND]

Commands:
  check    expectation に照らして PATH を lint（デフォルト）
  init     starter pathlint.toml を生成
  catalog  source カタログを inspect
  doctor   PATH 自体を lint
  help     ヘルプ表示

Options（global）:
      --target <process|user|machine>  デフォルト: process
      --rules <path>                   デフォルト: ./ → $XDG_CONFIG_HOME/pathlint/
  -v, --verbose                        n/a 含む全 expectation と解決後 PATH を表示
  -q, --quiet                          失敗のみ
      --color <auto|always|never>      デフォルト: auto
      --no-glyphs                      ASCII のみ
  -h, --help
  -V, --version
```

`pathlint sort` は post-MVP に予約。

## 12. 非機能要件

- **単一の Rust バイナリ。** OS 以外の runtime 依存無し。
- **クロスプラットフォーム第一級。** Windows、macOS、Linux すべてを
  CI で確認。Termux は端末上の `cargo install` 経由のみ — `dotfm`
  と同じ方針でビルド済み配布はしない。
- **起動時間。** PATH 約 100 件、expectation 約 20 件で
  `pathlint check` が warm cache 50 ms 以内。
- **安定した exit code。** `0` クリーン、`1` expectation 失敗、`2`
  config / I/O エラー。
- **エンコーディング。** どの OS でも path は UTF-8 文字列として扱う。
  稀に存在する非 UTF-8 PATH エントリは警告を出してスキップ。
- **組み込みカタログのバージョニング。** カタログはコンパイル時埋め
  込み。バンプ時はチェンジログに記載してデフォルト変更を周知。

## 13. 配布

- 0.0.2 以降に crates.io publish 予定。
- GitHub Releases workflow で `x86_64-{linux,windows,darwin}` と
  `aarch64-darwin` のアーカイブを配布。`dotfm` と同じ流れ。Termux
  ユーザーはソースからビルド。
- (post-MVP) Homebrew formula、scoop manifest、AUR PKGBUILD。

## 14. スコープ外

- PATH の編集／永続化（後の `sort` モードに先送り）。
- 関数／エイリアス resolve は対象外。PATH 上のファイル探索のみ。
- シェル設定パッチ（`.bashrc`、`$PROFILE` の書き換え）。
- バイナリがどの **パッケージ** に属するかの厳密判定。pathlint は
  パスプレフィックスしか見ない（`dpkg -S` / `rpm -qf` /
  `brew which-formula` / `pacman -Qo` / `paru -Qo` のようなことは
  しない）。これは正しさのもっとも大きなトレードオフ：AUR /
  `make install` / 任意 prefix は、ユーザーが該当 prefix を
  `[source.<name>]` で書くまで pathlint からは透明。0.2 で再考予定
  （§16 参照）。
- `/etc/environment`、PAM、launchd plist、systemd unit
  `Environment=` のパース。

## 15. 成功指標

- リファレンス dotfiles（`ShortArrow/dotfiles`）が
  `windows/Test-PathOrder.ps1` を `pathlint check` 呼び出しに置き換
  え、ルールファイルが同じリポに置かれる（5 行の `[[expect]]` のみ、
  `[source.*]` 上書きは無し）。
- README をコピペ・編集する形で 1 分以内に有用な `pathlint.toml`
  を書ける（最低 1 つは OS タグ付き）。
- 失敗実行が、コマンド名、実解決パス、不一致 source を、追加デバッ
  グツール無しで直せる程度に明確に示す。

## 16. 未解決事項

- **同じ source の複数インストール先。** `mise` はバイナリを
  `mise/shims/` と `mise/installs/<lang>/<ver>/bin/` の両方に置く。
  現状は両方とも「mise 由来」と扱う。これで十分か、
  `mise_shims` / `mise_installs` に分けるべきか。
- **カタログの可視化。** 組み込みカタログを `pathlint catalog list`
  で参照できるようにすべきか。実装は trivial だがサブコマンドが増
  える。*(0.0.x で解決済み — `pathlint catalog list` を提供。)*
- **パッケージマネージャ問い合わせ（0.2 候補）。** path ベースの
  マッチでは AUR / Homebrew tap / `make install` / `[source.<name>]`
  に書かれていない prefix のすべてが取りこぼされる。将来のノブと
  して、`[source.X] owner_query = ["pacman", "-Qo"]` のような source
  単位、または `[[expect]] via = "command"` の opt-in 形式が考え
  られる。トレードオフ: 1 回 50–100 ms のオーバーヘッド、OS 別の
  パーサ実装、信頼の循環依存（問い合わせ先のバイナリそのものが信
  頼できる必要）。0.1.x では不採用。path-based がどれだけ取りこぼ
  すかのフィールドデータが集まってから再検討。
- **シンボリックリンクされたシステムディレクトリ。** Arch / Solus /
  openSUSE TW などで `/usr/sbin → /usr/bin`。`which` は
  `/usr/sbin/<cmd>` を返すので、組み込みの `apt` / `pacman` /
  `dnf` / `system_linux`（`linux = "/usr/bin"` のみ）に substring
  マッチしない → ユーザー側で `[source.usr_sbin] linux =
  "/usr/sbin"` を追加するか、カタログに合成エントリを足すか。path
  canonicalize は採用しない方針：レポート上に出る source ラベルを
  silent に変える上、mise / volta / asdf の shim ベースマッチを
  壊す。
- **`prefer` の順序。** 現状 `prefer = ["mise", "volta"]` は集合
  扱い（「どれか満たせば OK」）。`sort` のとき優先順位として使う
  か。MVP 外。
- **カタログのバージョニング。** pathlint 側で組み込み source パス
  を更新（例：winget レイアウト変更）したとき、古いバイナリを使う
  ユーザーは黙って間違ったマッチをする可能性。`catalog_version = N`
  と `--require-catalog >= N` を入れるか検討。
- **macOS launchd / `eval $(brew shellenv)`。** これらが設定する
  PATH は `process` と違う場合あり。MVP 外。

## 17. 他ツールとの関係

- **`which` / `where.exe` / `type -a` / `Get-Command -All`**: 何が
  勝つかを教える。`pathlint` は **正しいやつが勝っているか** を
  教える。
- **`dotfm doctor`**: `pathlint check` は `dotfm.toml` の
  `[tools.<name>.doctor]` スクリプトから呼ばれる想定。
- **`PATH.txt` / `DiffPath.ps1`（`ShortArrow/dotfiles` 内）**:
  これらは「期待エントリが PATH 上に存在するか」を見る。`pathlint`
  は「解決バイナリがどのインストーラ由来か」を見る。両者は補完関係。
- **パッケージマネージャ（mise、brew、choco、pkg、…）**: `pathlint`
  はインストールを管理しない。彼らが作る順序が望むものかを教える。
