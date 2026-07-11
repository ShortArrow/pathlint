# pathlint — プロダクト要件定義書（PRD）

🌐 [English](PRD.md) | **日本語**

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

**R3 — PATH 衛生 + selfcheck。** expectation を 1 つも評価する前
に、PATH 自体が 散らかっている — 重複、不在ディレクトリ、8.3 短縮
名、より簡潔に 書ける エントリ。`pathlint lint` が PATH 単体で lint
する（0.0.34 で `pathlint doctor` から改名）。
`pathlint doctor` は別の問いに答える — pathlint 自身がこの環境で
動くか?（バイナリが PATH 上にあるか、`pathlint.toml` が parse でき
るか、env 変数が読めるか）。

**R4 — 出自。** `pathlint trace <command>` が解決済みバイナリの
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
`flatpak`、`windows_apps` …）。ユーザーは **expectation を書くだけ** で
よく、source は名前で参照されて自動解決される。

## 2. 課題定義

同じコマンド名が異なるインストーラから来ることはよくあって、どれが
勝つかが大事：

- このマシンで `cargo install runex` したのに、実際に走るのは
  `WinGet/Links` にある古いほう。同名・別ファイル。
- `python` は `mise` 由来であってほしい、Microsoft Store の
  `windows_apps` スタブからではなく。
- `node` は `volta` 由来がいい、システムの `apt` インストールでは
  なく。
- macOS の `gcc` は Homebrew 由来であってほしい、`/usr/bin/gcc`
  （かつて clang シムだった）からではなく。

`which` は何が勝つかを教えてくれるが、何が勝つべきかを dotfiles リポ
にコミットして全マシンでチェックできる形では教えてくれない。
`pathlint` がこの「あるべき姿」を明示し検証する。

## 3. ゴール

以下 7 つの横断原則は、PRD §3 番号に依存せずに引用できるように
[docs/PRINCIPLES.jp.md](PRINCIPLES.jp.md)（EN: [PRINCIPLES.md](PRINCIPLES.md)）
にも独立ドキュメントとして公開している。両者の本文は同期維持。

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
  `pathlint trace <command>` への follow-up を案内する。
- **R2（存在と形状）。** コマンドが path に解決されるとき、その path
  は本当に実行可能ファイルを指している必要がある。symlink は生き
  ていて、「実行可能」が嘘でないこと。今は `not_found` しか報告
  しないが、それ以外は 0.0.4 以降。
- **R3（PATH 衛生 + selfcheck）。** 0.0.34 以降は 2 つの兄弟
  コマンド：
  - `pathlint lint` — `[[expect]]` を書いていなくても、重複、不在
    ディレクトリ、8.3 短縮名、env-var で短縮できるエントリ、PATH
    ディレクトリ間で shadow される同名コマンド、相対パスエントリ、
    world-writable ディレクトリ、resolve できない形式破損エントリを
    検出する。`pathlint doctor`（0.0.13〜0.0.33）が出していた 12
    detector kind を引き継ぐ。
  - `pathlint doctor` — pathlint 自身がこの環境で機能するかを検査
    する：バイナリの PATH 上 self-locate、`pathlint.toml` の発見 +
    parse、`env_lookup` の動作（`PATH`、Windows では `PATHEXT`、
    config 探索用の `HOME` / `USERPROFILE`）。PATH の異常検査は
    しない。
- **R4（出自）。** 解決済みバイナリについて、最も妥当なインストーラ
  名と対応する uninstall コマンドを答える。半年前に
  `cargo install` したのか `mise use cargo:tool` したのか思い出せ
  ないときに役立つ。

### 3.1 0.1.0 graduation 基準

0.0.x → 0.1.0 への昇格は以下 7 項目を満たした時点で行う。「十分
できた」 ではなく、 reviewer が機械的に / 文書的に検証できる
具体的 pin として定義する。 pre-1.0 で BREAKING を許す方針は
decision log に記録されており、 この gate がその方針を retire する。

1. **lib 公開 API の凍結。** `tests/public_api.rs` で pin された
   10 module の surface が、 2 release 連続で `CHANGELOG.md` に
   `### Breaking` を出していない。
2. **CLI 表面の凍結。** `pathlint <subcommand>` と global flag が
   §11 の表と一致した状態で 2 release 連続を経過。
3. **Schemars 1.0 migration を評価済み。** 移行するか、
   移行を見送る ADR を書く (理由付き)。
4. **Trust model が文書化されている。**
   [`docs/SECURITY.md`](SECURITY.md) が全 boundary を網羅し、
   sanitisation pointer がコードを指す形で実装と同期。
5. **ADR completeness。** `CHANGELOG.md` の各 release の
   `### Breaking` で公開型 / 関数を名指ししているエントリには、
   `docs/decisions/NNNN-*.md` 上の対応 ADR が必ず link される。
6. **EN ↔ JP PRD の parity。** semantic な差分が < 50 行
   (目次のみ / link のみの差分は除外)。
7. **codex audit の H severity 未解決が無い。** 解決済か、
   H 評価が当てはまらなくなった理由を ADR に書いて downgrade
   済み。

1, 2, 5, 6, 7 は機械的 (countable) gate、 3 と 4 は narrative
gate。 graduation criteria の audit を通った瞬間の verification は
ADR (planned) として記録する。 番号はその時点の decision log の
末尾になる。

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
  ではない。R4（`pathlint trace`）は解決パスを前面に出すが、
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

§3 / §4 の境界 — OS レイアウト知識を first-class に持ち、ツールメタは
宣言的なカタログエントリのみで、ツールの runtime 挙動はモデル化しない
— は、それ自体が恒久的なコミットメント。「mise / asdf / volta が今
アクティブにしているものを pathlint が知るべきでは？」型の将来
request は、個別に検討するのではなくこの境界に照らして reject する。
ツール状態の問い合わせは side effect であり、拡張点はカタログの
宣言的 vocabulary の側にある。

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
| R3 — PATH 衛生 | `pathlint lint`（旧 `pathlint doctor`） | 実装ずみ（0.0.3）、 0.0.34 で 改名 |
| R3' — selfcheck | `pathlint doctor` | 実装ずみ（0.0.34） |
| R4 — 出自 | `pathlint trace <command>` | 実装ずみ（0.0.4） |

`pathlint init` と `pathlint catalog list` はインフラ系（設定の
雛形、カタログの inspect）でどの役割にも属さない。

### 7.1 `pathlint [OPTIONS]`（= `pathlint check`）

`check` がデフォルトサブコマンド。`pathlint` 単体で `check` 動作。

```
pathlint                              # = pathlint check
pathlint --target user                # 明示的なターゲット
pathlint --config ./other.toml
pathlint --verbose                    # n/a 含む全 expectation と解決後 PATH を表示
pathlint --quiet                      # 失敗のみ
pathlint check --explain              # NG ごとに多行詳細を表示（0.0.7+）
pathlint check --json                 # 全 outcome の JSON 配列（0.0.7+）
```

- `--target` のデフォルトは `process`。`user` / `machine` はどの OS
  でも受け付けるが Windows でのみ意味を持つ。Unix では 1 行警告を
  出して `process` にフォールバック。
- `--config` のデフォルト解決順：
  1. `--config <path>` が指定されればそれ。
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

### 7.5 `pathlint lint` と `pathlint doctor`（0.0.34+）

R3 は 0.0.34 で 2 つの兄弟コマンドに分離した。背景: Round 1 で
dotfiles dogfooding を 行った結果、0.0.33 の `doctor` が PATH 衛生
と pathlint selfcheck の 2 つの 無関係な仕事を 1 つの 名前で 抱え
ている のが 問題と 判明。

#### `pathlint lint`（PATH 衛生）

0.0.33 までの `doctor` が 出していた 12 detector kind を継承。
`Diagnostic` の JSON shape、kind enum、`--include` / `--exclude`
filter UX、`--json` 出力配列、schema（`schemas/doctor.schema.json`
を新 doctor と共有）、exit code 規約 — すべて そのまま、CLI 動詞だ
け が `doctor` → `lint` に変わる。

#### `pathlint doctor`（selfcheck）

3 つの確認 だけ:
1. binary 自己発見 — 動作中の pathlint binary が PATH 上にあるか。
2. `pathlint.toml` 発見 + parse — `locate_rules` と同じ探索
   （cwd → `$XDG_CONFIG_HOME` → `$HOME/.config` → `$USERPROFILE/.config`）
   で見つかった config が parse できるか。意味検証は別件（lint
   側、将来）。
3. `env_lookup` 動作 — `PATH`、Windows なら `PATHEXT`、config
   検索用に `HOME` / `USERPROFILE`。

selfcheck kind（共有 schema に additive）:
`binary_not_in_path`、`config_parse_error`、`config_not_found`（info
severity — config なしで pathlint を 走らせる のは 正当）、
`env_lookup_failed`。`Severity` enum に `info` discriminant を
`warn` / `error` と並列に additively 追加。

#### 0.0.33 doctor と共有の振る舞い（現在は `lint` 配下）:

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
  - `Conflict` — PATH 上に共存すべきでない 2 つ以上の source が
    すべて検出された。diagnostic 名は
    `Relation::ConflictsWhenBothInPath` 宣言（ビルトインまたは
    ユーザー `pathlint.toml`）から来る。各 source にマッチした
    PATH エントリは `group #N:` ブロックに分けて表示。0.0.11
    時点のビルトイン: `mise_activate_both`（mise の shim 層と
    install 層が同時にアクティブ）。ユーザーは pathlint 本体を
    いじらず `[[relation]] kind = "conflicts_when_both_in_path"`
    を書き加えるだけで新しい conflict 検出を増やせる。
    0.0.11 より前は path-substring ベースだったため、ユーザーが
    `[source.mise_shims]` を別の場所に上書きしても発火していた。
    relation 駆動になり、relation の declared sources が
    実際に PATH にマッチしたときだけ発火するように変わった
    （通常はこれが正しい挙動）。
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
  `first_index`、malformed の `reason`、conflict の `diagnostic`
  + `groups`）を持つ。0.0.11 より前は conflict が
  `kind = "mise_activate_both"` + `shim_indices` / `install_indices`
  という形だったが、汎用化に伴い退役した。schema は
  `check --json` / `where --json` と並ぶ 3-way の機械可読
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

### 7.7 `pathlint trace <command>`（R4、0.0.4 で実装、plugin provenance は 0.0.5 で実装）

`check` が内部で計算している情報を表に出す：指定コマンドについて

- 解決済みフルパス（R1 が評価しているもの）
- マッチした全 source、最も具体的なものから順に
- `provenance:` 行。`[[relation]] kind = "served_by_via"` 宣言が
  マッチしたとき、つまり 解決済みパスが relation の `host` source
  下にあり、続く path セグメントが relation の `guest_pattern` に
  マッチしたとき。relation の `installer_token`（無ければ
  `guest_provider`）が installer ラベルになり、raw segment は
  そのまま表示するので installer 自身のツールで確認できる。

  0.0.10 より前はハードコードの `MISE_PLUGIN_PREFIXES` テーブル
  だった。0.0.10 から `plugins/<name>.toml` を読むので、ユーザー
  も `pathlint.toml` に relation を書き足すだけで wrapper の
  attribution を拡張できる。
- 最も妥当な uninstall コマンド 1 つ。provenance がある場合は
  `<installer> uninstall '<rest>'` 形式（mise plugin の場合は
  `mise uninstall <installer>:'<rest>'`）で「best-guess; verify」
  注釈付き。そうでなければマッチした source の `uninstall_command`
  テンプレから組み立て。

`{bin}` 置換と mise plugin segment は 0.0.10 から
`format::quote_for(os, _)` を経由する。`/.../installs/cargo-$(rm -rf ~)/bin`
のような hostile PATH エントリでも、出力をコピペしてもシェル
インジェクションにならない。エスケープは Unix 系で POSIX 単一
クォート、Windows で PowerShell 単一クォート。

uninstall ヒントはユーザーが自分で実行する文字列、pathlint は
実行しない。provenance もカタログもコマンドを示せないときは、
推測ではなく明示的に「不明」と出す。

plugin provenance は relation 駆動のラベルで、R4 専用。
**source match ではない**。`[[expect]] prefer = ["cargo"]`
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
    "command": "mise uninstall cargo:'lazygit'  (best-guess; verify with `mise plugins ls`)"
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
  順に 3 バケットを連結。各バケットは元の相対順序を保つが、
  `[[relation]] kind = "prefer_order_over"`（0.0.10+）が当てはまる
  場合は **同一バケット内** で並べ替える（バケット境界は跨がない）。
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
- **Relation の消費範囲（0.0.12+）。** `pathlint sort` が読むのは
  `prefer_order_over` のみ。残り 4 種（`alias_of` /
  `conflicts_when_both_in_path` / `served_by_via` / `depends_on`）は
  source 間のグラフ構造を表現するが sort には影響しない。将来
  「mise_installs を mise_shims より前に出す」のような新しい順序
  ルールが必要になった場合は、新しい relation kind を追加する。
  既存 kind の解釈を拡張しない。これにより「新しい
  `served_by_via` を追加したら sort の挙動が変わった」という
  事故を防ぐ。

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
avoid   = ["windows_apps", "choco"]
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

### 8.4 エディタ向け JSON Schema（0.0.11 で出荷）

TOML 自体には schema 機構がないが、Taplo（TOML LSP のデファクト、
VS Code の "Even Better TOML" にも同梱）は JSON Schema を読める。
0.0.11 で出荷した内容：

1. `schemars` を runtime dep として追加し、`Config` /
   `Expectation` / `SourceDef` / `Relation` / `Severity` /
   `Kind` 型に `JsonSchema` derive を付けた。schema はパーサと
   同じ Rust 型から生成されるのでドリフトしない。
2. `src/bin/gen_schema` で schema を出力。`tests/schema.rs` が
   CI でドリフトを検出する（`schemas/pathlint.schema.json` が
   現在の生成器の出力と一致しなければ fail）。
3. `release` ワークフローが tag commit から再生成し、
   `pathlint.schema.json` を GitHub Release asset として
   バイナリと SHA256SUMS に並べて publish する。

ユーザーが pin できる安定 URL は 2 種類：

- **main の最新**（merge ごとに更新）:
  `https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json`
- **特定リリース**（tag で凍結 — `<TAG>` は固定したいバージョン
  に置き換える、例: `v0.0.40`）:
  `https://github.com/ShortArrow/pathlint/releases/download/<TAG>/pathlint.schema.json`

`pathlint.toml` の先頭 1 行で opt-in：

```toml
#:schema https://raw.githubusercontent.com/ShortArrow/pathlint/main/schemas/pathlint.schema.json
```

https://www.schemastore.org/ に `pathlint.toml` をファイル名で
マッチさせる PR を別途出す。Schema Store 反映後は Taplo /
Even Better TOML がユーザー設定なしで自動解決する。Schema Store
登録は pathlint のリリースサイクルから独立。

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
| Windows 固有 | `windows_apps`, `strawberry`, `mingw`, `msys` |
| macOS パッケージマネージャ | `brew_arm`, `brew_intel`, `macports` |
| Linux パッケージマネージャ | `apt`, `pacman`, `dnf`, `flatpak`, `snap` |
| Termux | `pkg`, `termux_user_bin` |
| OS ベースライン | `os_baseline_windows`, `os_baseline_macos`, `os_baseline_linux` |

`pathlint catalog list` を実行すれば、各 source の OS 別パスを
含めた解決済みカタログをダンプできる（ユーザー上書き分も含む）。各プラグインの TOML は
`plugins/<name>.toml` にある。

**source path の制約（0.0.10+）：** `check` / `doctor` / `where` /
`sort` は起動時に各 `[source.<name>]` の per-OS path を validate
する。expand 済 needle が `/`、`\`、または 3 バイト未満なら
exit 2 で reject。`Microsoft/WindowsApps` のような相対 needle
（fragment 系のビルトインで使う）は受け入れる — `find` の
boundary 検査がパス segment 境界を跨ぐ過剰マッチを防ぐ。

設計上のメモ：

- `apt` / `pacman` / `dnf` はすべて `/usr/bin` を指す。
  インストールバイナリの着地先が同じだから。pathlint からは
  「Linux distro」とほぼ同義のエイリアス。読みやすい名前を選ぶ。
- `brew_arm` と `brew_intel` を分けたのは、Mac 1 台での
  `/opt/homebrew/bin` vs `/usr/local/bin` 順序自体が頻出バグ源
  だから。
- `windows_apps` と `strawberry` は主に `avoid = [...]` リスト用
  に用意。

### 9.1 source 間の関係性（0.0.9+）

プラグインは `[[relation]]` ブロックで source 間の構造的関係を
宣言できる。ユーザーも `pathlint.toml` で同じ語彙を使ってカスタム
source 間の関係を表現できる。`pathlint catalog relations` で
マージ済み一覧を見られる（`--json` で機械可読）。

対応する `kind` は 5 つ：

- **`alias_of`** — 親 source が、より具体的な複数の子の
  キャッチオール。親にマッチすることが子のマッチを排除しない。
  0.0.10 以降 `pathlint trace` は、子のいずれかが matched に含まれて
  いるとき親をリストの末尾に押し下げる。`mise` が `mise_shims` /
  `mise_installs` の親、として使う。
- **`conflicts_when_both_in_path`** — PATH に同時に存在すると
  問題になる source 群。0.0.11 から `pathlint doctor` がすべての
  relation を walk し、`Kind::Conflict` を発火する。`diagnostic`
  ラベルは relation 由来。各 source にマッチした PATH エントリは
  group ごとに列挙される。ビルトイン: `mise_activate_both`。
  ユーザーは relation を書き足すだけで新 conflict 検出を増やせる。
- **`served_by_via`** — `host` が `guest_provider` 由来の
  バイナリを `guest_pattern` にマッチする path で提供している。
  オプションの `installer_token` フィールド（0.0.10+）は人間向け
  出力に使う installer 名で、source 名と異なってよい。例:
  `guest_provider = "pip_user"` だが `installer_token = "pipx"`
  （ユーザーが実行するのは `mise uninstall pipx:black`）。
  0.0.10 から `pathlint trace` がこれを直接読む。
- **`depends_on`** — `target` が `source` の硬い前提。
  「`source` は `target` に依存する」と読む。例: `paru` は
  `pacman` に依存している（`paru` を uninstall しても pacman 管理
  バイナリは残る）。**descriptive only** — `pathlint catalog
  relations` の出力には現れ、cycle check にも参加するが、他の
  subcommand はこの kind を消費しない。`pathlint trace` から
  「X も uninstall する必要がある」hint を出すかは post-1.0 検討
  事項で、それまでは relation はユーザーが grep できるデータで
  あって runtime signal ではない。
- **`prefer_order_over`**（0.0.10+） — `earlier` は PATH 内で
  `later` より前にあるべき。`pathlint sort` が preferred / neutral /
  avoided バケット内のタイブレークに使う。バケット境界は跨がない
  （`prefer_order_over` で avoid 扱いの source を救出することは
  できない）。サイクル検査の対象となる有向辺。

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
installer_token = "cargo"
```

`served_by_via` / `depends_on` / `prefer_order_over` は有向辺。
pathlint は `pathlint catalog relations` を実行したときにマージ
済みグラフが acyclic であるかを検証する。循環は config エラー
（exit 2）。`alias_of` と `conflicts_when_both_in_path` は対称な
関係なので DAG 検査には参加しない。

0.0.9 では relation は記述目的のみだったが、0.0.10 で
`pathlint trace` が `served_by_via` + `alias_of` を直接読むように
なり（`MISE_PLUGIN_PREFIXES` テーブルは削除）、`pathlint sort` が
`prefer_order_over` を読むようになった。0.0.11 で
`pathlint doctor` も `conflicts_when_both_in_path` を読む側に
回り、relation グラフ全体が runtime で消費される設計が完成した。
新たな conflict / order / provenance 挙動は TOML 編集だけで
追加できる。

各 consumer が読む kind は明確に分かれている：`where` は
`served_by_via` + `alias_of`、`sort` は `prefer_order_over` のみ
（§7.8 参照）、`doctor` は `conflicts_when_both_in_path`。
`depends_on` は現状記述専用 — `catalog relations` 出力には
現れるが、他の subcommand では消費されない。この明示的な
対応関係により「relation を 1 つ増やしたら、宣言してもいない
コマンドの挙動が変わる」事故を防ぐ。

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

### 10.1 PATH entry の raw/expanded 二重性 (0.0.23+)

PATH entry には、 検出器と resolver が別々の理由で必要とする 2 つの
意味的な形がある:

- **raw** — source に保存されたままの文字列。 Windows では
  `REG_EXPAND_SZ` の `%LocalAppData%\WindowsApps`、 Unix では
  shell が `PATH` を export する際に展開しなかった
  `~/.local/bin` や `$HOME/bin`。
- **expanded** — `expand::expand_env(raw)`。 OS が実際に
  ディスク上で参照するディレクトリ文字列。

`pathlint` はこの 2 形式を **境界点 1 箇所**
(`pathlint::path_source::read_path`) で確定し、 全 entry を
`pathlint::path_entry::PathEntry { raw, expanded }` に lift する。
*ユーザが書いた文字列* を見る検出器 — Shortenable (既に短縮済の
entry をさらに短縮しろと誤誘導してはならない)、 RelativePathEntry
(未解決 `$VAR/bin` は config bug で surface すべき) — は
`entry.raw` を読む。 *ファイルシステム上のディレクトリ* を見る
検出器 — Missing、 WriteablePathDir、 DuplicateButShadowed、
PerSourceMissingRequired — と `resolve` walker は `entry.expanded`
を読む。

**Windows registry の取り扱い方針。** `winreg` の
`RegKey::get_value::<String, _>` は REG_EXPAND_SZ に対して内部で
`ExpandEnvironmentStringsW` を黙って呼んでしまい、
`%LocalAppData%` の形が `PathEntry::from_raw` に届く前に剥がれて
しまう。 そのため `pathlint` は `RegKey::get_raw_value` で raw
bytes を取得し、 `path_source::decode_reg_string` で UTF-16 LE
として decode する。 invalid surrogate は U+FFFD に置換 (lossy
decode)。 `REG_SZ` / `REG_EXPAND_SZ` 以外の registry type
(`REG_MULTI_SZ`、 `REG_BINARY`、 `REG_DWORD` …) に対しては
明示的な warning を出して空 PATH に fallback — 悪意ある payload
で panic しない。 `expand_env` の呼び出しは `PathEntry::from_raw`
1 箇所のみ、 これにより Windows と Unix が完全に同じ
「source は raw、 boundary で expand」ルールに乗る。

**ユーザから見える効果。** `pathlint doctor --target user` /
`--target machine` の Windows 出力で、 `%LocalAppData%`-style
entry が registry に書いたまま raw 形式で表示されるようになった
(human / JSON 共通)。 0.0.22 までは展開後の形式が出ていたため、
ユーザが registry に入力した文字列と一致せず混乱の元だった。

**Decoder 失敗時の方針。** `path_source::decode_reg_string` は
invalid UTF-16 surrogate pair に対して **lossy** に
(該当 code unit を `U+FFFD` で置換、 panic はしない)、 `REG_SZ`
/ `REG_EXPAND_SZ` 以外の registry value type (`REG_MULTI_SZ`、
`REG_BINARY`、 `REG_DWORD`、 …) に対しては **`Err`** を返す。
どちらの場合も `read_path` は `warning` と空の `entries` を返す
ので、 pathlint は悪意ある registry payload で panic せず、
壊れた bytes から黙って diagnostic を組み立てもせず、
理解できない type の値を見過ごしもしない。

**env-lookup injection。** `PathEntry::from_raw(raw, env_lookup)`
は `Fn(&str) -> Option<String>` を取り、 constructor は
caller の closure のみ env oracle として参照する — pathlint は
`from_raw` の中から `std::env::var` を呼ばない。 infrastructure
境界 (`path_source::read_path` と `resolve::split_path`) のみ
`|v| std::env::var(v).ok()` を inject する。 lib embedder と
test は決定論的 closure を inject することで、 host 環境に
依存しない動作を得る。 同じ closure が `expand::expand_env_with`
にも流れる (これが従来の `expand::expand_env` の公開 form、
`expand_env` 自体は process env を読む薄い wrapper として残る)。

0.0.26+ で公開 matching surface にも同 pattern を展開。
`expand` 層は `expand_and_normalize_with(input, env_lookup)` を、
`source_match` 層は `find_with(...)` / `validate_sources_with(...)` /
`names_only_with(...)` を公開し、 既存 4 関数 (`expand_and_normalize`,
`find`, `validate_sources`, `names_only`) は wrapper として残す。
embedder が `_with` 系のみを呼べば、 catalog source path の
展開でも `std::env::var` を一切経由せずに pathlint を動かせる
— lib 公開境界で injection は完成。

0.0.27+ で内部 call-graph の threading も完了。
4 つの公開 entry point — `doctor::analyze` / `lint::evaluate` /
`trace::locate` / `sort::sort_path` — はそれぞれ typed `*Deps<'_>`
carrier (`AnalyzeDeps` / `EvaluateDeps` / `LocateDeps` /
`SortDeps`) を受ける形になり、 carrier は共通 `CommonDeps` を
embed して env oracle を 1 箇所に集約する。 各 entry point 内の
matcher は `deps.common.env_lookup` を `source_match::*_with` に
流すので、 deterministic な carrier を組む embedder は
`std::env` を 1 度も触らずに pathlint を動かせる。 repo 内で
wrapper を残す唯一の call site は
`bin/pathlint/run::enforce_source_validation` — これは binary
側で常に live env を読む役割。

**observed と provenance (0.0.24+、 Windows process target; 型分離は 0.0.28)。**
`PathEntry { raw, expanded }` は **同一 source** から見た entry の 2
形式を表す。 Windows には 2 つの source が食い違う 1 ケースがある:
`--target process` は `getenv("PATH")` を読むが、 OS が
`REG_EXPAND_SZ` の registry 値を `ExpandEnvironmentStringsW` で
**子プロセスに渡す前に展開してしまう**。 そのため process
entry の `raw` は常に literal — HKCU に
`%LocalAppData%\Microsoft\WindowsApps` と書いてあっても、
process target からは展開後の literal しか見えない。 0.0.23 の
raw 保持 fix は `--target user` / `--target machine` (registry
直読) には効くが、 default の `--target process` には効かない。

0.0.24 で cross-source overlay を導入、 0.0.28 で `PathEntry`
から専用 carrier `pathlint::Attribution` に切り出した:

```rust
pub struct Attribution {
    pub observed: PathEntry,
    pub provenance_raw: Option<String>,
}
```

Windows process target では `path_source::read_process` が起動時に
HKCU と HKLM の raw を併読し、 純粋な
`reconcile_process_with_registry` overlay が、 process 側の
`Attribution.observed.expanded` と registry 側の
`Attribution.observed.expanded` が一致した entry に対して
`provenance_raw` を付与する。 ユーザ意図を見る検出器
(`Shortenable`、 `Malformed`、 `TrailingSlash`、 `ShortName`、
および `Diagnostic.entry` の表示) は
`Attribution::effective_raw_for_user_intent()` 経由で
`provenance_raw` を優先参照する。 ファイルシステム側の検出器
(`Missing`、 `WriteablePathDir`、 resolver) は引き続き
`attrib.observed.expanded` を読む — file system はユーザが書いた
文字列に関心がないので。

overlay の契約:

- match 条件: 両側の `expanded` を `expand::normalize` した
  上での equality (大文字小文字無視 + slash 統一)。
  `C:\Users\Me\X` と `c:/users/me/x` は同 entry。
- tie-break: HKCU が HKLM に優先、 同一 source 内では先に
  現れたものを採用。 実行ごとに決定的。
- 一致しない entry は overlay 不採用。 codex の安全側方針:
  false-suppress (本来出すべき diagnostic を消す) より
  false-negative (literal を literal のまま扱い Shortenable は
  発火する) を選ぶ。 子 shell の `set PATH=...` や Python
  supervisor の `os.environ['PATH']` 改変など runtime injection
  された PATH は registry に対応物が無いので、 そのまま観測通り
  に扱う。
- registry の raw が process の raw と verbatim 一致する場合
  も overlay 不採用 (REG_SZ なので展開が起きておらず、
  overlay する必要が無い)。
- `provenance_raw` はそれ以外の経路で常に `None`:
  `--target user` / `--target machine` (raw が source で
  authoritative)、 Unix / macOS (registry 自体が無い)、
  registry に対応 entry が無い process entry。

`--target` の意味自体は変わらない — 3 値はあくまで *pathlint が
どこから読むか* を表す。 overlay は cross-source の補助情報で、
`process` が「raw を復元できるならする」ためのもの。 overlay の
結果として process が user+machine の merge view に化けるわけ
ではない。

## 11. CLI 表面

```
pathlint [OPTIONS] [COMMAND]

Commands:
  check    expectation に照らして PATH を lint（デフォルト）
  init     starter pathlint.toml を生成
  catalog  source カタログを inspect
    list       全 source を列挙（組み込み + ユーザー定義）
    relations  source 間の宣言された [[relation]] を列挙
  lint     PATH 自体を lint（重複、不在ディレクトリ等）
  doctor   selfcheck: pathlint 自身がこの環境で機能するか
  trace    コマンドがどこから来るかと uninstall ヒント
  sort     全 [[expect]] を満たす PATH 順序を提案
  help     ヘルプ表示

Options（global）:
      --target <process|user|machine>  デフォルト: process
      --config <path>                   デフォルト: ./ → $XDG_CONFIG_HOME/pathlint/
  -v, --verbose                        n/a 含む全 expectation と解決後 PATH を表示
  -q, --quiet                          失敗のみ
      --color <auto|always|never>      デフォルト: auto
      --no-glyphs                      ASCII のみ
  -h, --help
  -V, --version
```

`pathlint sort` は読み取り専用の提案（§7.8 参照）で、`--dry-run`
が必須。`--dry-run` なしで `sort` を実行すると説明メッセージと
共に exit 2 になる（意図的な speed bump）。`--apply` モード
は PRD §4 の「PATH を書き換えない」方針により未実装。検討は
post-1.0 議題。

`pathlint catalog relations` は組み込みプラグインとユーザー
`[[relation]]` ブロックが宣言した source 間の関係を表示する
（§9.1 参照）。

**alias の退役 (0.0.22 で削除済)。** `pathlint where` (`pathlint
trace` の alias) と `--rules` (`--config` の alias) は 0.0.14 から
0.0.21 まで clap の `visible_alias` として維持されていた (0.0.20
で stderr deprecation warning も追加)。 0.0.22 BREAKING リリースで
両方とも削除。 `pathlint trace` と `--config` に rename して
ください。 旧 alias を渡すと clap が unknown argument として reject
する。

## 12. 非機能要件

- **単一の Rust バイナリ。** OS 以外の runtime 依存無し。
- **クロスプラットフォーム第一級。** Windows、macOS、Linux すべてを
  CI で確認。Termux は端末上の `cargo install` 経由のみ — `dotfm`
  と同じ方針でビルド済み配布はしない。
- **起動時間。** PATH 約 100 件、expectation 約 20 件で
  `pathlint check` が warm cache 50 ms 以内。 0.0.24 の Windows
  process target では provenance overlay のために起動時に HKCU と
  HKLM を併読する (`RegQueryValueEx` × 2 hive + `O(n*m)` の
  expanded-equality reconcile、 通常 `m ≈ 30`)。 実測で数 ms 単位
  のオーバーヘッドに収まり、 50 ms 予算は維持される。
- **安定した exit code。** `0` クリーン、`1` expectation 失敗、`2`
  config / I/O エラー。
- **エンコーディング。** どの OS でも path は UTF-8 文字列として扱う。
  `PATH` 全体が UTF-8 として読めない場合は空として扱う。エントリ
  単位の警告 + スキップは将来の改善項目。0.0.11 から
  `format::strip_control_chars`（ASCII 制御バイト 0x00–0x08、
  0x0B–0x1F、0x7F は `?` に置換、`\t` と `\n` は維持）を
  すべての human モードレンダラに適用する：`where` / `doctor` /
  `catalog list` / `catalog relations` / `check` のレポート。
  JSON 出力は `serde_json` が制御バイトを正しく escape するので
  変更不要。
- **シェル文字列の信頼境界（0.0.10+）。** `pathlint trace` の出力は
  ユーザーがコピペするかもしれないコマンド文字列。`{bin}` 置換と
  mise plugin segment は `format::quote_for(os, _)` を経由する
  （Unix 系で POSIX 単一クォート、Windows で PowerShell 単一
  クォート）。カタログのテンプレ本体（`uninstall_command = "..."`
  の中身）は再 quote しない — カタログ作者かユーザー設定由来で、
  pathlint はそこを信頼する。
- **rules ファイルの DoS 対策（0.0.11+）。** `Config::from_path` は
  `--config` / `pathlint.toml` の最終 hop が regular file でない
  （ブロックデバイス、複数段の symlink）場合に reject、サイズも
  16 MiB を上限にする。1 段の symlink → regular file は許可
  （dotfiles 管理を壊さないため）。違反は exit 2。
- **組み込みカタログのバージョニング。** カタログはコンパイル時埋め
  込み。バンプ時は GitHub Release のリリースノートに記載してデフォ
  ルト変更を周知。bump 履歴：
  - `0.0.10` → `catalog_version = 3`（TOML 本文は変わらないが
    `trace` / `sort` が relation を読むように解釈が変わったため）。
  - `0.0.11` は `catalog_version = 3` を維持：doctor も relation
    を読むようになったが、relation TOML 本文とビルトイン source
    path に変更はない。
  - `0.0.14` → `catalog_version = 4`：source 名 rename のため
    （`WindowsApps` → `windows_apps`、`system_*` →
    `os_baseline_*`、加えて `os_baseline_linux_sbin` を新設）。
    旧名で `[source.<name>]` を参照していた user TOML は移行が
    必要（§17 参照）。
  - `0.0.15` は `catalog_version = 4` を維持：embedded TOML 本文に
    変更はないが、user TOML が `catalog_version` を宣言した場合
    の reject が post-parse から structural（deny_unknown_fields）
    に格上げされた。

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
### R3 — PATH 衛生

- **[R3] mise activate モードの自動判別。** `mise activate` は PATH
  先頭に `mise/shims/` を前置する形と、`installs/<lang>/<ver>/bin/`
  を直接 PATH 書き換えする形の 2 通り。両層が同時に存在するときの
  警告は実装済み（§16 解決済み参照）が、ユーザーがどちらのモードを
  意図しているかの自動判別はしない — `[[expect]]` ルールは
  `mise_shims` / `mise_installs` を明示で書く。ツールのモード推定を
  持ち込んでまで自動判別する価値があるかは未解決。
- **[R3] DuplicateButShadowed。** 同じ command basename が PATH の
  異なる dir に実体として 2 つ以上存在し、後ろの dir が shadow
  される状況。常に報告する — 重複は事実であって noise ではない。
  host 個別に抑制したい場合は `--exclude duplicate_but_shadowed`。

  既存の relation 駆動 `mise_activate_both` Conflict detector と
  役割分担: あちらは *named* source (`mise_shims` と
  `mise_installs`) が両方 PATH に出ているときに発火 (同 command が
  両方にあるかは問わない)。 こちらは *同 command* が 2 dir 以上に
  存在するときに発火 (dir が named source か否かは問わない)。
  named-source-pair の角度と unnamed command-name の角度を
  両方カバーする 2 段構え。

  常に報告する理由: mise activate の標準的な使い方では shims か
  installs のどちらか片方しか PATH に出ない (`mise activate` は
  shims、 `mise hook-env` は installs を露出する)。 両方が PATH に
  あるのはそれ自体が設定ミスで、 既存の `mise_activate_both`
  Conflict detector が別角度からカバーする。 同じ状況を別 detector
  で隠すと同じミスを見逃すことになる。 詳細な設計議論は §17.2
  0.0.19 entry。 *(0.0.19+。)*
- **[R3] RelativePathEntry。** PATH entry が env 展開後も相対
  path のまま (`.`、 `./bin`、 bare `bin`、 …)。 shell は呼出時の
  cwd 基準で resolve するため、 走る binary が user の居場所に
  依存する — security と可搬性の両面で footgun。 env 変数は先に
  展開するので HOME 設定済の `$HOME/bin` は発火しない。 未解決の
  `$VAR/bin` は verbatim 残るので発火 (それ自体が設定 bug)。
  「absolute かどうか」は host ではなく target Os で判定する
  (Linux の `/usr/bin` は absolute、 Windows では drive letter が
  ないので相対扱い)。 抑制は `--exclude relative_path_entry`。
  *(0.0.20+。)*
- **[R3] WriteablePathDir。** PATH entry が指す dir が owner 以外
  のユーザに書き込み可能なときに発火。 shell 権限を持つ攻撃者が
  malicious binary を置いて、 user が common command を typing した
  ときに走らせるリスク。 Unix では others-write bit (`mode & 0o002`)
  を見る。 Windows では DACL を取得し、 3 つの well-known SID
  — **Everyone** (`S-1-1-0`)、 **Authenticated Users**
  (`S-1-5-11`)、 **BUILTIN\\Users** (`S-1-5-32-545`) — のいずれかに
  effective `FILE_GENERIC_WRITE` / `FILE_APPEND_DATA` が立っていれば
  発火。 0.0.21 は Everyone のみ check していたが、 0.0.22 で残り
  2 つを追加した (典型的な Windows host では Everyone より group
  経由 write の方が多い)。 まだ approximation: この 3 group の外で
  per-user に explicit grant されたケースや DENY ACE の伝搬は
  modelling していない。 Missing / 読み取り不能 dir は skip
  (Missing detector がカバー)。 抑制は `--exclude writeable_path_dir`。
  *(0.0.21+。 0.0.22 で SID を 3 つに拡張。)*
- **[R3] macOS launchd / `eval $(brew shellenv)`。** これらが設定
  する PATH は `process` と違うことがある。MVP 外、0.0.x 線でも
  扱わず 0.1.x 候補として整理。実装方針は 3 案：

  1. **`--target launchd` フラグを新設。** `Target` enum に第 4
     variant を追加し、`pathlint check --target launchd` で
     launchd-visible な PATH を同じ rule set で lint。
     **長所:** check / doctor / where が同一構造で扱える。
     **短所:** launchctl spawn コストが毎実行発生、macOS 専用、
     `launchctl getenv PATH` は global env のみで plist-bootstrap
     系の daemon env はカバーしない。
  2. **doctor 専用の diff 診断。** ユーザシェルの PATH と
     `launchctl getenv PATH` が違うときに発火する `Kind` variant
     を新設。
     **長所:** 「iTerm では動くのに launchd 経由で起動した
     daemon では PATH が違う」を target モデル拡張なしに発見。
     **短所:** doctor の責務が「PATH 自体の lint」から「環境
     差分の検出」に拡張される。診断 payload に 2 本の PATH を
     抱え込む必要があり、現行の per-entry 診断より太る。
  3. **段階的：まずオプション 2、需要を見てオプション 1 に拡張。**
     0.1.x で read-only な diff 診断のみ出荷し、ユーザが
     launchd-visible PATH に対して `[[expect]]` ルールを書きたい
     という需要が立ったら Target を拡張。憶測で target 表面を
     固定しない。

  着手前に詰めるべき前提：
  - `launchctl` の出力フォーマットが macOS バージョン間で
    安定しているか（Sequoia でいくつかの subcommand が変わった）。
  - `launchctl getenv` が正しい情報源なのか、それとも user /
    system の Launch Daemons / Agents plist も読むべきか。
  - Linux の systemd user units / `EnvironmentFile=`、Windows の
    HKLM\SYSTEM\CurrentControlSet\Services `Environment` REG_MULTI_SZ
    — 同じ問題で形だけ違う。需要が一番大きい macOS から手を
    付ける。

  Schema Store 登録（PRD §8.4 のフォローアップ）と SHA pin 済み
  actions の Renovate / Dependabot 化（PRD §13）は別軸で進める
  項目で、本件をブロックしない。

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
- **`pathlint where` と `which` / `where.exe` の混同。**
  *(0.0.14 で解決 — `pathlint where` を `pathlint trace` に rename。
  `where` alias 自体は deprecation warning の移行期間を経て 0.0.22
  で削除。)*
- **Arch / openSUSE TW における `/usr/sbin` 先行レイアウト
  （シンボリックリンクされたシステムディレクトリ）。**
  *(0.0.14 で解決 — built-in `os_baseline_linux_sbin` source を追加。
  Arch / Solus / openSUSE TW などで `/usr/sbin → /usr/bin` の
  symlink 構成のとき `which` は `/usr/sbin/<cmd>` を返すので、
  `os_baseline_linux_sbin` を package manager と並べて `prefer` に
  入れる。path canonicalize は不採用 — レポート上の source ラベルを
  silent に変える上、mise / volta / asdf の shim ベースマッチを
  壊すため。)*
- **[R1, R4] mise プラグイン経由のバイナリの帰属。** *(0.0.5 で
  解決 — `mise/installs/<plugin>/<ver>/bin/<bin>` の `<plugin>`
  segment が `cargo-` / `npm-` / `pipx-` / `go-` / `aqua-` で
  始まるとき、R4 が `provenance:` 行と
  `mise uninstall <installer>:<rest>` ヒントを出す。これは純粋な
  provenance heuristic であって source label ではない —
  `prefer = ["cargo"]` は `mise/installs/cargo-foo/...` のバイナリに
  **マッチしない**。マッチさせたいユーザーは `mise/installs/cargo-`
  部分一致の `[source.X]` を自分で書く。)*
- **[R3] mise の shims と installs の同時存在を警告。** *(0.0.5 で
  解決 — `Kind::Conflict { diagnostic = "mise_activate_both" }`
  diagnostic が shim / install 両 group を列挙する。意図している
  モードの自動判別は未解決のまま、§16 R3 参照。)*

## 17. 0.0.x 変更履歴（累積）

リリース別の変更履歴はリポジトリルートの
[`CHANGELOG.md`](../CHANGELOG.md) に
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) 形式で
集約している。各エントリは `### Breaking` / `### Added` /
`### Changed` 下に逆時系列で並ぶ。

0.0.x 線は各 `0.0.x → 0.0.(x+1)` の bump を MAJOR 相当として
扱う (Cargo の pre-1.0 慣例)。Breaking change は
0.0.x 内で許容され、`CHANGELOG.md` の `### Breaking` で告知する。
0.0.x が 0.1.0 に上がる時期は未定 (PRD §3 の graduation 基準を
満たした時点で切る)。

設計判断 (「なぜそうしたか」) は
[`docs/decisions/`](decisions/) の ADR (Architecture Decision
Record) に蓄積する。`CHANGELOG.md` の `### Breaking` で公開
型 / 関数 を名指しするエントリには、対応する ADR が必ずある
(これは graduation 基準の一つ)。

PRD §17 はかつて累積 changelog を inline で持っていたが、
0.0.22-0.0.23 で `CHANGELOG.md` に移行し、0.0.25 で JP 側も
EN parity のため CHANGELOG 参照のみに縮小した。release 履歴は
プロジェクトルートで読者が期待する場所に置き、PRD は仕様に
専念する分担になっている。

## 18. 他ツールとの関係

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
