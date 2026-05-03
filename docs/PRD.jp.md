# pathlint — プロダクト要件定義書（PRD）

**ステータス:** 0.0.x 進行中。スキーマと CLI 表面は引き続き
動きうる（0.1.0 で安定化予定）。現在のクレートバージョンは
`Cargo.toml` および README の crates.io バッジを参照。

---

## 1. 概要

`pathlint` は、いま手元にある PATH について 4 つの問いに答える CLI。
理想の PATH ではなく、現実の PATH について。

**R1 — 解決順。** あるコマンドについて、どのインストーラ由来のコピー
が勝つか。`[[expect]] command = "x" prefer = ["cargo"]` と書けば
pathlint がチェックする。元来の用途であり、ツールの背骨。

**R2 — 存在と形状。** pathlint が解決したファイルは本当に
実行可能か、それとも誰かが同名のディレクトリで `runex` を覆い隠した
のか。symlink は壊れていないか。`[[expect]]` に
`kind = "executable"` を付ければ、source チェックに加えて解決パスが
実在の実行可能ファイルかも検証する。

**R3 — PATH 衛生。** expectation を 1 つも評価する前に、PATH 自体が
散らかっている — 重複、不在ディレクトリ、8.3 短縮名、より簡潔に
書ける エントリ。`pathlint doctor` が PATH 単体で lint する。

**R4 — 出自。** `pathlint where <command>` が解決済みバイナリの
フルパス、マッチしたカタログ source、最も妥当な uninstall コマンド
（`mise uninstall cargo:lazygit`、`cargo uninstall lazygit` など）
を出力する。mise のプラグイン層から提供されたバイナリには上流の
インストーラも推定して表示する。

1 つの `pathlint.toml` が 4 役割すべてを **Windows、macOS、Linux、
Termux** 横断でカバーする。source は OS 別の場所を宣言、各
`[[expect]]` は `os = [...]` フィルタを持てる。

`pathlint` は well-known な source の **組み込みカタログ** を持つ
（`cargo`、`mise`、`mise_shims`、`mise_installs`、`volta`、`winget`、
`choco`、`scoop`、`brew_arm`、`brew_intel`、`apt`、`pacman`、`pkg`、
`flatpak`、`WindowsApps` …）。ユーザーは **expectation を書くだけ** で
よく、source は名前で参照されて自動解決される。

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

4 役割すべて（R1〜R4）に共通：

- **宣言的。** pathlint が気にすることはすべて、dotfiles リポに置ける
  `pathlint.toml` で表現できる。実行時フラグだけに隠れる挙動はない。
- **パスではなく source ラベル。** ユーザーはインストーラ名
  （`cargo`、`mise_shims`、`winget`、`brew_arm`、`apt`）で書く。
  パスパターンはカタログから引かれるので同じ TOML が全マシンで動く。
- **組み込みカタログ + 上書き。** pathlint がよく使われるインストーラ
  のデフォルトを内蔵。ユーザーは上書きしたい / 新規追加したいときだけ
  `[source.X]` を書く。
- **1 ファイル、全 OS。** 各 `[[expect]]` に `os = [...]` フィルタ、
  各 `[source.X]` に OS 別パス。同じ `pathlint.toml` が Windows /
  macOS / Linux / Termux を回す。
- **部分一致 + 大文字小文字無視。** 環境変数展開と slash 正規化の
  あとで、source パスを解決済みパスに対し substring 比較。
- **正直な exit code。** `0` = クリーン、`1` = 1 つ以上失敗、`2` =
  config / I/O エラー。R3（doctor）と R4（where）も同じスケール。
- **読み取り専用。** PATH、レジストリ、dotfiles、インストール済み
  パッケージ、いずれも書き換えない。何があるかを伝えるのみ、行動は
  ユーザーが取る。

役割別：

- **R1（解決順）。** 失敗 expectation はコマンド名、実解決パス、
  マッチした source、`prefer` / `avoid` の違反内容を示す。他の
  デバッグツール無しで直せる程度に。`pathlint check --explain`
  （0.0.7+）は NG ごとに 6 行（resolved / matched / prefer / avoid
  / diagnosis / hint）の詳細表示に切り替え、`avoid` ヒット時には
  違反 source 名を、`prefer` 不一致時には候補一覧を出し、
  `pathlint where <command>` への follow-up を案内する。
- **R2（存在と形状）。** コマンドが path に解決されるとき、その path
  は本当に実行可能ファイルを指している必要がある。symlink は生き
  ていて、「実行可能」が嘘でないこと。今は `not_found` しか報告
  しないが、それ以外は 0.0.4 以降。
- **R3（PATH 衛生）。** `[[expect]]` を書いていなくても、`pathlint
  doctor` が重複、不在ディレクトリ、8.3 短縮名、env-var で短縮できる
  エントリ、形式破損エントリ（resolve できないもの）を検出する。
- **R4（出自）。** 解決済みバイナリについて、最も妥当なインストーラ
  名と対応する uninstall コマンドを答える。半年前に
  `cargo install` したのか `mise use cargo:tool` したのか思い出せ
  ないときに役立つ。

## 4. 非ゴール

役割を絞ったぶん、明示的な非役割も決まる：

- **PATH の書き換え／永続化はしない。** プロセス PATH、Windows
  レジストリ、`.bashrc`、`$PROFILE`、その他のシェル設定、いずれも
  pathlint は変更しない。何が間違っているかを伝える、どう直すかは
  ユーザー判断。`pathlint sort --dry-run` は推奨順序を表示するだけで、
  適用は決してしない。
- **`which` クローンではない（R1 境界）。** pathlint 内部に resolve
  ロジックはあるが、`where` / `type -a` / `Get-Command -All` を
  置き換える意図はない。R1 が答える問いは「正しいインストーラが
  勝っているか？」であって、「これはどこから resolve されるか？」
  ではない。R4（`pathlint where`）は解決パスを前面に出すが、
  generic な which クローンとしてではなく、出自情報付きで。
- **将来のインストールのシミュレーションはしない。** pathlint は
  *いま*ある PATH とバイナリについて答える。次の `cargo install` が
  どこに着地するか、次の mise activate がどんな順序を作るか、計画
  しているインストールが「安全」か、こうしたことは予測しない。
  予測するためには各インストーラをモデル化する必要があり、信頼面が
  膨れ上がる。
- **パッケージ管理はしない。** expectation を満たすために不足ツール
  を入れない。R4 が uninstall コマンドを*提案*する（ユーザーが実行
  する文字列として）ことはあっても、実行はしない。
- **環境の深いパースはしない。** プロセスが実際に見る PATH
  （`getenv("PATH")`）と、Windows ならレジストリ 2 ヶ所までを読む。
  `/etc/environment`、PAM、launchd plist、systemd unit
  `Environment=`、`eval "$(brew shellenv)"`、いずれもスコープ外。
- **パッケージマネージャ問い合わせはしない（0.1.x）。** pathlint は
  `dpkg -S` / `rpm -qf` / `pacman -Qo` / `brew which-formula` を
  呼ばない。パスプレフィックスマッチは速くオフラインで動くが、
  AUR / `make install` / 任意 prefix は不可視のまま（ユーザーが
  `[source.<name>]` を足すまで）。0.2 で再考（§16 参照）。

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

サブコマンドと役割の対応表（§1 参照）：

| 役割 | サブコマンド | 状態 |
|---|---|---|
| R1 — 解決順 | `pathlint check`（デフォルト） | 実装ずみ（0.0.2） |
| R2 — 存在と形状 | `[[expect]] kind = "..."` を `check` に拡張 | 実装ずみ（0.0.4） |
| R3 — PATH 衛生 | `pathlint doctor` | 実装ずみ（0.0.3） |
| R4 — 出自 | `pathlint where <command>` | 実装ずみ（0.0.4） |

`pathlint init` と `pathlint catalog list` はインフラ系（設定の
雛形、カタログの inspect）でどの役割にも属さない。

### 7.1 `pathlint [OPTIONS]`（= `pathlint check`）

`check` がデフォルトサブコマンド。`pathlint` 単体で `check` 動作。

```
pathlint                              # = pathlint check
pathlint --target user                # 明示的なターゲット
pathlint --rules ./other.toml
pathlint --verbose                    # n/a 含む全 expectation と解決後 PATH を表示
pathlint --quiet                      # 失敗のみ
pathlint check --explain              # NG ごとに多行詳細を表示（0.0.7+）
pathlint check --json                 # 全 outcome の JSON 配列（0.0.7+）
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
  （`optional` および `severity = "warn"` は除く）。
- **ルールごとの severity（0.0.7+）。** `[[expect]]` は optional な
  `severity` を取る（`"error"` デフォルト、`"warn"`）。`error` は
  0.0.x 通りで NG → exit 1。`warn` は同じ診断を `[warn]` タグで
  表示し exit 0 を保つ。CI で「1 件の逸脱でビルドを止めたくないが
  気付きは欲しい」ケース用。`error` ルールと `warn` ルールは同じ
  `pathlint.toml` に混在可能。`check --json` でも severity を出力。

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
  - (0.0.5+) `MiseActivateBoth` — PATH 上に `mise/shims/` と
    `mise/installs/` が同時に存在。`mise activate` が shim と
    PATH-rewrite モード両方で設定されているか、過去の設定の残骸が
    まだ PATH に残っているか。shim entries と install entries
    すべてを列挙して、どちらを残すかユーザーが判断できるようにする。
- `--quiet` で warn 抑制、error は常に表示。
- (0.0.6+) `--include <kind>[,<kind>...]` で表示対象を絞る、
  `--exclude <kind>[,<kind>...]` で抑制。両方同時指定はエラー。
  値は snake_case の kind 名（`duplicate` / `missing` /
  `shortenable` / `trailing_slash` / `case_variant` /
  `short_name` / `malformed` / `mise_activate_both`）。未知の
  名前は config エラー (exit 2)。exit code は **絞られたあとの**
  集合に対して計算されるので、`--exclude malformed` で
  Error も含めて抑制すると本当に exit 0 で通る。
- (0.0.7+) `--json` で human view を JSON 配列に切り替え。各要素
  は `index` / `entry` / `severity`（`"warn"` / `"error"`）/
  `kind` 判別子 + kind ごとの payload フィールド（shortenable の
  `suggestion`、case_variant の `canonical`、duplicate の
  `first_index`、malformed の `reason`、mise_activate_both の
  `shim_indices` / `install_indices`）を持つ。schema は 0.0.x で
  stable、`check --json` / `where --json` と並ぶ 3-way の機械可読
  サーフェス。include / exclude は JSON でも有効。`--quiet` は
  JSON mode では無視（情報を欠落させない設計）。

### 7.6 `[[expect]] kind = "executable"`（R2、0.0.4 で実装）

現状の `[[expect]]` は「`command` が resolve すること」と「マッチ
した source が prefer / avoid 的に妥当か」までしか見ない。解決
パスの実体は次のいずれかでも検出されない：

- ディレクトリ（誰かが同名フォルダで bin を覆い隠した）
- 切れた symlink
- 実行権限のない通常ファイル
- 中途半端なインストールでサイズ 0 のファイル

`kind = "executable"` を expectation に書けるようにすれば、resolve
パスが実際に実行可能ファイルかを pathlint が検証する（symlink は
追跡、Unix のモードビット / NTFS リパースを尊重）。失敗時は
`NG (not_executable)` という新ステータスで形状不一致を名指しする。

語彙は 0.0.4 では最小：`executable` のみ。"native binary" と
"script" の区別は OS 別の事情が多く（Windows `.cmd` vs `.exe`、
Unix の shebang）見合うリターンが薄い。

### 7.7 `pathlint where <command>`（R4、0.0.4 で実装、plugin provenance は 0.0.5 で実装）

`check` が内部で計算している情報を表に出す：指定コマンドについて

- 解決済みフルパス（R1 が評価しているもの）
- マッチした全 source、最も具体的なものから順に
- (0.0.5+) `provenance:` 行。パスが `mise/installs/<segment>/...`
  の下にあり、`<segment>` が `cargo-` / `npm-` / `pipx-` / `go-`
  / `aqua-` のいずれかで始まるとき。インストーラ名と raw plugin
  segment を併記するので `mise plugins ls` で確認できる。
- 最も妥当な uninstall コマンド 1 つ。provenance がある場合
  （0.0.5+）は `mise uninstall <installer>:<rest>` 形式で
  「best-guess; verify」注釈付き（segment → ID 変換は lossy のため）。
  そうでなければマッチした source の `uninstall_command` テンプレ
  から組み立て。

uninstall ヒントはユーザーが自分で実行する文字列、pathlint は
実行しない。provenance もカタログもコマンドを示せないときは、
推測ではなく明示的に「不明」と出す。

plugin provenance は path-segment の heuristic で、R4 専用の
ラベル。**source match ではない**。`[[expect]] prefer = ["cargo"]`
は `mise/installs/cargo-foo/...` のバイナリに **マッチしない**。
そう動かしたければユーザーが明示的に `[source.X]` で
`mise/installs/cargo-` 部分一致を書く必要がある。

命名: `where` は Windows の `where.exe` と被るが、pathlint の出力は
出自情報中心でスタイルが明らかに違う。実用上の混乱が大きすぎたら
0.1.0 までに改名を再検討する。

(0.0.6+) `--json` で出力を機械可読の単一オブジェクトに切り替え。
スキーマは `0.0.x` 中安定：

```json
{
  "found": true,
  "command": "lazygit",
  "resolved": "/home/u/.local/share/mise/installs/cargo-lazygit/0.61/bin/lazygit",
  "matched_sources": ["mise_installs", "mise"],
  "uninstall": {
    "kind": "command",
    "command": "mise uninstall cargo:lazygit  (best-guess; verify with `mise plugins ls`)"
  },
  "provenance": {
    "kind": "mise_installer_plugin",
    "installer": "cargo",
    "plugin_segment": "cargo-lazygit"
  }
}
```

`uninstall.kind` は `"command"` / `"no_template"` (`source` を持つ)
/ `"no_source"`。 `provenance` は heuristic が発火しないとき `null`。
NotFound は `{ "command": "...", "found": false }` を出して exit 1。

### 7.8 `pathlint sort`（R5 — 修正、0.0.8 で読み取り専用版を実装）

- 適用可能な全 expectation を満たす PATH 順序を計算する。読み取り
  専用：before / after 差分（デフォルト）または `SortPlan` JSON
  （`--json`）を出力。pathlint は PATH を書き換えない — 出力を
  shell snippet、レジストリ編集、dotfiles diff と組み合わせて適用
  するのはユーザー側。
- アルゴリズム：`os` フィルタが当てはまる各 `[[expect]]` について、
  PATH の各エントリを **preferred**（`prefer` にマッチ）、
  **avoided**（`avoid` にマッチ）、ニュートラル の 3 つに分類する。
  両方にマッチするエントリは `avoid` が優先（`lint::decide` と
  同じ規則）。`SortPlan` は preferred → ニュートラル → avoided の
  順に 3 バケットを連結。各バケットは元の相対順序を保つので、
  差分には「考える価値のある変化」だけが残る。`prefer` も `avoid`
  も空のルールは寄与しない。どの定義済 source にもマッチしない
  エントリはバケット内位置そのまま。
- `prefer` が並べ替えで満たせない場合（PATH エントリの誰一人として
  該当 source にマッチしない）、`SortNote::UnsatisfiablePrefer` を
  出して command と prefer set を提示する。修正方法は「該当 source
  からインストールする」か「ルールを緩める」のいずれか。
- 常に exit 0。`sort` は **提案** コマンドであって go / no-go チェック
  ではない。go / no-go には `pathlint check` を使う。
- `--apply` は 0.0.8 には入らない。PRD §4 の「PATH を書き換えない」
  方針を維持する。`--apply` を入れる検討は post-1.0 議題で、
  明示的なフラグを必要とする形で再検討する。

## 8. `pathlint.toml` スキーマ

```toml
# ---- [[expect]]: コマンドごとの期待 ----

# タグ無し: 全 OS で適用される。OS を絞りたいなら `os = [...]` を書く。
# （pathlint は「prefer に挙げた source が現在 OS に per-OS path を
# 持たない」ようなケースで自動スキップはしない。ルールは走る。）
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

デフォルトカタログは `plugins/` 配下にパッケージマネージャごと
の TOML として配置されている。`build.rs` がコンパイル時にそれら
を 1 つの埋め込み文字列に concat する。新しいパッケージマネー
ジャを足すには、`plugins/<name>.toml` を作って
`plugins/_index.toml` の plugins 配列に名前を加える。

現在の構成（グループ別）：

| グループ | プラグイン / ソース |
|---|---|
| 汎用ユーザーディレクトリ | `user_bin`, `user_local_bin` |
| 言語ツールチェーン | `cargo`, `go`, `npm_global`, `pip_user` |
| 多言語バージョンマネージャ | `mise` / `mise_shims` / `mise_installs`, `volta`, `aqua`, `asdf` |
| Windows パッケージマネージャ | `winget`, `choco`, `scoop` |
| Windows 固有 | `WindowsApps`, `strawberry`, `mingw`, `msys` |
| macOS パッケージマネージャ | `brew_arm`, `brew_intel`, `macports` |
| Linux パッケージマネージャ | `apt`, `pacman`, `dnf`, `flatpak`, `snap` |
| Termux | `pkg`, `termux_user_bin` |
| OS ベースライン | `system_windows`, `system_macos`, `system_linux` |

`pathlint catalog list` を実行すれば、各 source の OS 別パスを
含めた解決済みカタログをダンプできる（ユーザー上書き分も含む）。各プラグインの TOML は
`plugins/<name>.toml` にある。

設計上のメモ：

- `apt` / `pacman` / `dnf` はすべて `/usr/bin` を指す。
  インストールバイナリの着地先が同じだから。pathlint からは
  「Linux distro」とほぼ同義のエイリアス。読みやすい名前を選ぶ。
- `brew_arm` と `brew_intel` を分けたのは、Mac 1 台での
  `/opt/homebrew/bin` vs `/usr/local/bin` 順序自体が頻出バグ源
  だから。
- `WindowsApps` と `strawberry` は主に `avoid = [...]` リスト用
  に用意。

### 9.1 source 間の関係性（0.0.9+）

プラグインは `[[relation]]` ブロックで source 間の構造的関係を
宣言できる。ユーザーも `pathlint.toml` で同じ語彙を使ってカスタム
source 間の関係を表現できる。`pathlint catalog relations` で
マージ済み一覧を見られる（`--json` で機械可読）。

対応する `kind` は 4 つ：

- **`alias_of`** — 親 source が、より具体的な複数の子の
  キャッチオール。親にマッチすることが子のマッチを排除しない。
  `mise` が `mise_shims` / `mise_installs` の親、として使う。
- **`conflicts_when_both_in_path`** — PATH に同時に存在すると
  問題になる source 群。複数該当があれば `pathlint doctor` が
  `diagnostic` (snake_case な `Kind` 名) を出す。
- **`served_by_via`** — `host` が `guest_provider` 由来の
  バイナリを `guest_pattern` にマッチする path で提供している。
  `pathlint where` が wrapper 経由の provenance に使う。
- **`depends_on`** — `target` が `source` の硬い前提。
  「`source` は `target` に依存する」と読む。例: `paru` は
  `pacman` に依存している（`paru` を uninstall しても pacman 管理
  バイナリは残る）。`pathlint where` で表示。

例（`plugins/mise.toml` に組み込み済）：

```toml
[[relation]]
kind = "alias_of"
parent = "mise"
children = ["mise_shims", "mise_installs"]

[[relation]]
kind = "conflicts_when_both_in_path"
sources = ["mise_shims", "mise_installs"]
diagnostic = "mise_activate_both"

[[relation]]
kind = "served_by_via"
host = "mise_installs"
guest_pattern = "cargo-*"
guest_provider = "cargo"
```

`served_by_via` と `depends_on` は有向辺。pathlint は `pathlint
catalog relations` を実行したときにマージ済みグラフが acyclic で
あるかを検証する。循環は config エラー（exit 2）。`alias_of` と `conflicts_when_both_in_path` は
対称な関係なので DAG 検査には参加しない。

0.0.9 では relation は **記述目的** のみ。`pathlint catalog
relations` で見られるが、doctor / where_cmd / sort のコード経路
は 0.0.9 以前のハードコード heuristic のまま動く。これらを
relation 駆動に書き換えるのは 0.0.10 の課題。

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
    list       全 source を列挙（組み込み + ユーザー定義）
    relations  source 間の宣言された [[relation]] を列挙
  doctor   PATH 自体を lint（重複、不在ディレクトリ等）
  where    コマンドがどこから来るかと uninstall ヒント
  sort     全 [[expect]] を満たす PATH 順序を提案
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

`pathlint sort` は読み取り専用の提案（§7.8 参照）。`--apply` モード
は PRD §4 の「PATH を書き換えない」方針により未実装。検討は
post-1.0 議題。

`pathlint catalog relations` は組み込みプラグインとユーザー
`[[relation]]` ブロックが宣言した source 間の関係を表示する
（§9.1 参照）。

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
  `PATH` 全体が UTF-8 として読めない場合は空として扱う。エントリ
  単位の警告 + スキップは将来の改善項目。
- **組み込みカタログのバージョニング。** カタログはコンパイル時埋め
  込み。バンプ時は GitHub Release のリリースノートに記載してデフォ
  ルト変更を周知。

## 13. 配布

- crates.io に `pathlint` として公開ずみ。
- GitHub Releases で `x86_64-{linux,windows,darwin}` と
  `aarch64-darwin` のアーカイブを配布。Termux ユーザーは
  `cargo install pathlint` でソースからビルド。
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

各項目に該当する役割を [R1] / [R2] / [R3] / [R4] でタグ付け。

### R1 — 解決順

- **[R1] シンボリックリンクされたシステムディレクトリ。** Arch /
  Solus / openSUSE TW などで `/usr/sbin → /usr/bin`。`which` は
  `/usr/sbin/<cmd>` を返すので、組み込みの `apt` / `pacman` /
  `dnf` / `system_linux`（`linux = "/usr/bin"` のみ）に substring
  マッチしない → ユーザー側で `[source.usr_sbin] linux =
  "/usr/sbin"` を追加するか、カタログに合成エントリを足すか。path
  canonicalize は採用しない方針：レポート上に出る source ラベルを
  silent に変える上、mise / volta / asdf の shim ベースマッチを
  壊す。
- **[R1] `prefer` の順序。** 現状 `prefer = ["mise", "volta"]` は
  集合扱い（「どれか満たせば OK」）。`sort` のとき優先順位として
  使うか。post-MVP の `pathlint sort` 設計と一体。

### R1 / R4 — インストーラ識別

- **[R1, R4] パッケージマネージャ問い合わせ（0.2 候補）。** path
  ベースのマッチでは AUR / Homebrew tap / `make install` /
  `[source.<name>]` に書かれていない prefix のすべてが取りこぼされる。
  将来のノブとして `[source.X] owner_query = ["pacman", "-Qo"]` か
  `[[expect]] via = "command"` opt-in が考えられる。トレードオフ:
  1 回 50–100 ms、OS 別パーサ、信頼の循環依存（問い合わせ先の
  バイナリ自体が信頼できる必要）。0.1.x では不採用。path-based が
  どれだけ取りこぼすかのフィールドデータ次第。R4 は特にここから
  恩恵を受ける（パッケージマネージャが所有者を確認すれば
  uninstall ヒントが鋭くなる）。
- **[R1, R4] mise プラグイン経由のバイナリの帰属。** mise の
  プラグイン経由のバイナリは `mise/installs/<plugin>/<ver>/bin/<bin>`
  に置かれ、`<plugin>` が上流インストーラ名を含む。
  *(0.0.5 で解決 — R4 が segment が `cargo-` / `npm-` / `pipx-` /
  `go-` / `aqua-` で始まるときに `provenance:` 行と
  `mise uninstall <installer>:<rest>` ヒントを出す。R1 のカタログ
  には触らず、これは純粋な provenance heuristic — source label
  ではない。なので `prefer = ["cargo"]` は
  `mise/installs/cargo-foo/...` のバイナリに**マッチしない**。
  マッチさせたいユーザーは `mise/installs/cargo-` 部分一致の
  `[source.X]` を自分で書く。)*

### R3 — PATH 衛生

- **[R3] mise activate vs shims モード。** `mise activate` は PATH
  先頭に `mise/shims/` を前置する形と、`installs/<lang>/<ver>/bin/`
  を直接 PATH 書き換えする形の 2 通り。*(0.0.5 で「両層が同時存在
  したら警告」の半分を解決 — `pathlint doctor` が `MiseActivateBoth`
  diagnostic を出して shim / install 両方のエントリを列挙する。
  expect ルール側でどちらを選ぶかはユーザーが決める、pathlint は
  自動判別しない。)*
- **[R3] macOS launchd / `eval $(brew shellenv)`。** これらが設定
  する PATH は `process` と違うことがある。MVP 外。R3 では R1 と
  違う形で出すかも：login services が見る PATH と、ユーザーが見る
  PATH を比較して doctor が差分を提示する、など。

### 横断 / インフラ

- **`MISE_DATA_DIR` / `XDG_DATA_HOME`.** mise はこれらの env var で
  ツリーの場所を変えられる。組み込みカタログはデフォルトの
  `$LocalAppData/mise` (Windows) / `$HOME/.local/share/mise` (Unix)
  を埋め込んでいる。カスタム配置のユーザーは `pathlint.toml` 側
  で `[source.mise]`（および兄弟 2 つ）を上書きする。これが繰り返し
  papercut になるなら 0.0.5 以降で自動検出に格上げ。

### 解決済み

- **[R1] 同じ source の複数インストール先。** *(0.0.3 で解決 —
  `mise` / `mise_shims` / `mise_installs` の 3 層に分割。)*
- **カタログの可視化。** *(0.0.x で解決 — `pathlint catalog list`
  を提供。)*
- **カタログのバージョニング。** *(0.0.3 で解決 — `catalog_version`
  / `require_catalog`。)*

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
